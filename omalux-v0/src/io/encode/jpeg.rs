use std::{
    fs::File,
    io::{self, Write},
    path::Path,
};

use image::{ExtendedColorType, ImageEncoder, codecs::jpeg::JpegEncoder};

use super::{
    EncodeCancellation, JpegEncodeInput, JpegEncodeReport, metadata::MAX_JPEG_EXIF_TIFF_BYTES,
    prepare_display_rgb8,
};
use crate::io::{
    AtomicOutputError, AtomicOutputOptions, EncodeError, EncodeOptions, ResourceLimits,
    SourceFileIdentity, write_atomic_output_for_source,
};

pub struct JpegEncodeRequest<'a> {
    pub input: JpegEncodeInput<'a>,
    pub destination: &'a Path,
    /// Identity captured from the decoder's already-open source file.
    ///
    /// It is used only for inode-collision protection and is never persisted;
    /// the encoder never reopens or resolves the source path.
    pub source_identity: Option<SourceFileIdentity>,
    pub encode: EncodeOptions,
    pub atomic: AtomicOutputOptions,
    pub limits: &'a ResourceLimits,
    pub cancellation: &'a EncodeCancellation,
}

/// Encodes and atomically publishes one display-referred JPEG.
pub fn encode_jpeg(request: JpegEncodeRequest<'_>) -> Result<JpegEncodeReport, EncodeError> {
    let prepared = prepare_display_rgb8(
        request.input,
        &request.encode,
        request.limits,
        request.cancellation,
    )?;
    let expected = u64::from(prepared.width)
        .checked_mul(u64::from(prepared.height))
        .and_then(|value| value.checked_mul(3))
        .and_then(|value| usize::try_from(value).ok())
        .ok_or(EncodeError::Limit(
            crate::io::LimitError::ArithmeticOverflow,
        ))?;
    if prepared.rgb8.len() != expected
        || prepared
            .exif
            .as_ref()
            .is_some_and(|exif| exif.len() > MAX_JPEG_EXIF_TIFF_BYTES)
    {
        return Err(EncodeError::Encode);
    }

    let mut inner_failure = None;
    let mut output_bytes = 0_u64;
    let publication = write_atomic_output_for_source(
        request.destination,
        request.source_identity,
        request.atomic,
        |file| {
            if request.cancellation.cancelled() {
                inner_failure = Some(InnerFailure::Cancelled);
                return Err(io::Error::new(
                    io::ErrorKind::Interrupted,
                    "encode cancelled",
                ));
            }
            let mut bounded = BoundedWriter::new(file, request.limits.max_output_bytes);
            let codec_result = encode_prepared(&prepared, request.encode.quality, &mut bounded);
            output_bytes = bounded.written;
            if bounded.limit_hit {
                inner_failure = Some(InnerFailure::Limit(crate::io::LimitError::OutputBytes {
                    requested: bounded.attempted,
                    maximum: request.limits.max_output_bytes,
                }));
                return Err(io::Error::new(
                    io::ErrorKind::FileTooLarge,
                    "encoded output limit exceeded",
                ));
            }
            if let Some(failure) = bounded.write_failure.take() {
                // Keep filesystem failures in the AtomicOutput error domain.  A
                // codec error caused by its writer is not a codec failure and
                // callers must retain the transaction/retry semantics.
                return Err(failure.into_io_error());
            }
            if let Err(failure) = codec_result {
                inner_failure = Some(match failure {
                    CodecFailure::Allocation => {
                        InnerFailure::Limit(crate::io::LimitError::Allocation)
                    }
                    CodecFailure::Codec => InnerFailure::Codec,
                });
                return Err(io::Error::other("JPEG codec failed"));
            }
            if request.cancellation.cancelled() {
                inner_failure = Some(InnerFailure::Cancelled);
                return Err(io::Error::new(
                    io::ErrorKind::Interrupted,
                    "encode cancelled",
                ));
            }
            Ok(())
        },
    );
    let outcome = match publication {
        Ok(outcome) => outcome,
        Err(AtomicOutputError::PublishedButNotDurable(_)) if inner_failure.is_none() => {
            crate::io::AtomicOutputOutcome::PublishedButNotDurable
        }
        Err(error) => return Err(map_atomic(error, inner_failure)),
    };

    Ok(JpegEncodeReport {
        outcome,
        width: prepared.width,
        height: prepared.height,
        quality: request.encode.quality,
        output_bytes,
        icc: prepared.icc_provenance,
        clipped_samples: prepared.clipped_samples,
        alpha_flattened_pixels: prepared.alpha_flattened_pixels,
        metadata: prepared.metadata_report,
    })
}

fn encode_prepared(
    prepared: &super::PreparedDisplayRgb,
    quality: u8,
    writer: &mut impl Write,
) -> Result<(), CodecFailure> {
    let mut encoder = JpegEncoder::new_with_quality(writer, quality);
    let icc = try_clone_bytes(&prepared.icc)?;
    encoder
        .set_icc_profile(icc)
        .map_err(|_| CodecFailure::Codec)?;
    if let Some(exif) = &prepared.exif {
        let exif = try_clone_bytes(exif)?;
        encoder
            .set_exif_metadata(exif)
            .map_err(|_| CodecFailure::Codec)?;
    }
    encoder
        .encode(
            &prepared.rgb8,
            prepared.width,
            prepared.height,
            ExtendedColorType::Rgb8,
        )
        .map_err(|_| CodecFailure::Codec)
}

fn try_clone_bytes(source: &[u8]) -> Result<Vec<u8>, CodecFailure> {
    let mut output = Vec::new();
    output
        .try_reserve_exact(source.len())
        .map_err(|_| CodecFailure::Allocation)?;
    output.extend_from_slice(source);
    Ok(output)
}

enum CodecFailure {
    Allocation,
    Codec,
}

enum InnerFailure {
    Cancelled,
    Limit(crate::io::LimitError),
    Codec,
}

fn map_atomic(error: AtomicOutputError, inner: Option<InnerFailure>) -> EncodeError {
    match (error, inner) {
        (AtomicOutputError::Write(_), Some(InnerFailure::Cancelled)) => EncodeError::Cancelled,
        (AtomicOutputError::Write(_), Some(InnerFailure::Limit(error))) => {
            EncodeError::Limit(error)
        }
        (AtomicOutputError::Write(_), Some(InnerFailure::Codec)) => EncodeError::Encode,
        (error, _) => EncodeError::Output(error),
    }
}

struct BoundedWriter<'a> {
    inner: &'a mut File,
    maximum: u64,
    written: u64,
    attempted: u64,
    limit_hit: bool,
    write_failure: Option<WriteFailure>,
}

#[derive(Clone, Copy)]
struct WriteFailure {
    kind: io::ErrorKind,
    raw_os_error: Option<i32>,
}

impl WriteFailure {
    fn from_io_error(error: &io::Error) -> Self {
        Self {
            kind: error.kind(),
            raw_os_error: error.raw_os_error(),
        }
    }

    fn into_io_error(self) -> io::Error {
        self.raw_os_error
            .map(io::Error::from_raw_os_error)
            .unwrap_or_else(|| io::Error::new(self.kind, "JPEG output write failed"))
    }
}

impl<'a> BoundedWriter<'a> {
    fn new(inner: &'a mut File, maximum: u64) -> Self {
        Self {
            inner,
            maximum,
            written: 0,
            attempted: 0,
            limit_hit: false,
            write_failure: None,
        }
    }
}

impl Write for BoundedWriter<'_> {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        let Some(requested) = self.written.checked_add(buffer.len() as u64) else {
            self.attempted = u64::MAX;
            self.limit_hit = true;
            return Err(io::Error::new(
                io::ErrorKind::FileTooLarge,
                "output overflow",
            ));
        };
        self.attempted = requested;
        if requested > self.maximum {
            self.limit_hit = true;
            return Err(io::Error::new(
                io::ErrorKind::FileTooLarge,
                "encoded output limit exceeded",
            ));
        }
        let count = self.inner.write(buffer).inspect_err(|error| {
            self.write_failure = Some(WriteFailure::from_io_error(error));
        })?;
        self.written = self
            .written
            .checked_add(count as u64)
            .ok_or_else(|| io::Error::new(io::ErrorKind::FileTooLarge, "output overflow"))?;
        Ok(count)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn post_publish_durability_failure_is_never_reclassified() {
        let mapped = map_atomic(
            AtomicOutputError::PublishedButNotDurable(io::Error::other("fault")),
            None,
        );
        assert!(matches!(
            mapped,
            EncodeError::Output(AtomicOutputError::PublishedButNotDurable(_))
        ));
    }
}
