//! Production Qt-free decoder and JPEG publication services.

use std::{fs::File, io, os::unix::fs::FileExt, path::Path};

use rustix::fs::{self, FileType, Mode, OFlags};

use crate::{
    io::{
        AtomicOutputOptions, AtomicOutputOutcome, DecodeError, DecodeOptions, EncodeCancellation,
        EncodeError, EncodeOptions, JpegEncodeInput, JpegEncodeRequest, OutputFormat,
        ResourceLimits,
        raster::{RasterCancellation, decode_raster_file},
        raw::{RawCancellation, RawExecutionOptions, decode_raw_file, trusted_dcraw_execution},
    },
    job::{
        CancellationToken, DecodedSource, DisplayReferred, EncodeReceipt, PhotoDecoder,
        PhotoEncoder, PublicationRequest, PublicationStatus, WorkingArtifact,
    },
};

#[derive(Clone, Debug)]
pub struct ProductionPhotoDecoder {
    raw: Option<RawExecutionOptions>,
}

impl ProductionPhotoDecoder {
    /// Creates a decoder with an optional, fixed system RAW backend. Raster
    /// decoding remains available when the backend is not installed or fails
    /// the trusted-file policy.
    pub fn new() -> Self {
        Self {
            raw: trusted_dcraw_execution().ok(),
        }
    }

    #[cfg(test)]
    fn with_raw(raw: RawExecutionOptions) -> Self {
        Self { raw: Some(raw) }
    }
}

impl Default for ProductionPhotoDecoder {
    fn default() -> Self {
        Self::new()
    }
}

impl PhotoDecoder for ProductionPhotoDecoder {
    type Error = DecodeError;

    fn decode_path_once(
        &self,
        input: &Path,
        options: &DecodeOptions,
        cancellation: &CancellationToken,
    ) -> Result<DecodedSource, Self::Error> {
        options.validate()?;
        if cancellation.is_cancelled() {
            return Err(DecodeError::Cancelled);
        }
        let fd = fs::open(
            input,
            OFlags::RDONLY | OFlags::NONBLOCK | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(map_source_open)?;
        self.decode_opened(File::from(fd), input, options, cancellation)
    }
}

impl ProductionPhotoDecoder {
    fn decode_opened(
        &self,
        file: File,
        source_name: &Path,
        options: &DecodeOptions,
        cancellation: &CancellationToken,
    ) -> Result<DecodedSource, DecodeError> {
        let stat = fs::fstat(&file).map_err(|error| {
            DecodeError::Input(io::Error::from_raw_os_error(error.raw_os_error()))
        })?;
        if FileType::from_raw_mode(stat.st_mode) != FileType::RegularFile {
            return Err(DecodeError::UnsupportedFormat);
        }
        let advertised = u64::try_from(stat.st_size).map_err(|_| DecodeError::UnsupportedFormat)?;
        options
            .limits
            .check_source_bytes(advertised)
            .map_err(DecodeError::Limit)?;
        let mut signature = [0_u8; 8];
        let read = file
            .read_at(&mut signature, 0)
            .map_err(DecodeError::Input)?;

        let raster = RasterCancellation::from_flag(cancellation.shared_flag());
        let raw = RawCancellation::from_flag(cancellation.shared_flag());
        let result = if raster_signature(&signature[..read]) {
            decode_raster_file(file, options, &raster)
        } else {
            let execution = self
                .raw
                .as_ref()
                .ok_or(DecodeError::RawBackendUnavailable)?;
            decode_raw_file(file, source_name, options, execution, &raw)
        }?;
        Ok(DecodedSource {
            photo: result.0,
            source_identity: result.1,
        })
    }
}

fn raster_signature(bytes: &[u8]) -> bool {
    bytes.starts_with(&[0xff, 0xd8, 0xff])
        || bytes.starts_with(b"\x89PNG\r\n\x1a\n")
        || bytes.starts_with(b"BM")
}

fn map_source_open(error: rustix::io::Errno) -> DecodeError {
    if error == rustix::io::Errno::LOOP {
        DecodeError::UnsupportedFormat
    } else {
        DecodeError::Input(io::Error::from_raw_os_error(error.raw_os_error()))
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ProductionJpegEncoder {
    limits: ResourceLimits,
}

impl ProductionJpegEncoder {
    pub const fn new(limits: ResourceLimits) -> Self {
        Self { limits }
    }
}

impl PhotoEncoder for ProductionJpegEncoder {
    type Error = EncodeError;

    fn encode_display(
        &self,
        publication: PublicationRequest<'_>,
        artifact: &WorkingArtifact<DisplayReferred>,
        options: &EncodeOptions,
        cancellation: &CancellationToken,
    ) -> Result<EncodeReceipt, Self::Error> {
        if options.format != OutputFormat::Jpeg {
            return Err(EncodeError::UnsupportedFormat);
        }
        let encode_cancellation = EncodeCancellation::from_flag(cancellation.shared_flag());
        let report = crate::io::encode_jpeg(JpegEncodeRequest {
            input: JpegEncodeInput {
                image: artifact.image(),
                signal_relation: artifact.signal_relation(),
                metadata: artifact.metadata(),
            },
            destination: publication.destination,
            source_identity: Some(publication.source_identity),
            encode: *options,
            atomic: AtomicOutputOptions::default().with_overwrite(publication.overwrite),
            limits: &self.limits,
            cancellation: &encode_cancellation,
        })?;
        let publication = match report.outcome {
            AtomicOutputOutcome::PublishedAndDurable => PublicationStatus::PublishedAndDurable,
            AtomicOutputOutcome::PublishedButNotDurable => {
                PublicationStatus::PublishedButNotDurable
            }
        };
        Ok(EncodeReceipt {
            bytes_written: report.output_bytes,
            publication,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs as stdfs, os::unix::fs::PermissionsExt, time::Duration};
    use tempfile::tempdir;

    fn jpeg(path: &Path, rgb: [u8; 3]) {
        image::save_buffer_with_format(
            path,
            &rgb,
            1,
            1,
            image::ColorType::Rgb8,
            image::ImageFormat::Jpeg,
        )
        .unwrap();
    }

    fn fake_raw_backend(directory: &Path) -> RawExecutionOptions {
        let executable = directory.join("dcraw_emu");
        stdfs::write(
            &executable,
            "#!/bin/sh\nout=''\nwhile [ \"$#\" -gt 0 ]; do\n  if [ \"$1\" = '-Z' ]; then shift; out=$1; fi\n  shift\ndone\nprintf 'P6\\n1 1\\n65535\\n\\377\\377\\100\\000\\000\\000' > \"$out\"\n",
        )
        .unwrap();
        stdfs::set_permissions(&executable, stdfs::Permissions::from_mode(0o700)).unwrap();
        let mut execution = RawExecutionOptions::new(&executable).unwrap();
        execution.staging_directory = directory.to_owned();
        execution.timeout = Duration::from_secs(2);
        execution
    }

    #[test]
    fn opened_raster_survives_path_replacement_and_keeps_original_identity() {
        let directory = tempdir().unwrap();
        let original = directory.path().join("photo.raw");
        let moved = directory.path().join("moved.jpg");
        jpeg(&original, [120, 80, 40]);
        let file = File::open(&original).unwrap();
        let identity = crate::io::SourceFileIdentity::from_file(&file).unwrap();
        stdfs::rename(&original, &moved).unwrap();
        stdfs::write(&original, b"replacement raw bytes").unwrap();

        let result = ProductionPhotoDecoder::new()
            .decode_opened(
                file,
                &original,
                &DecodeOptions::default(),
                &CancellationToken::new(),
            )
            .unwrap();
        assert_eq!(result.source_identity, identity);
        assert_eq!(
            result.photo.source_digest(),
            crate::io::SourceDigestV1::from_bytes(&stdfs::read(moved).unwrap())
        );
    }

    #[test]
    fn unknown_signature_routes_the_same_open_file_to_raw() {
        let directory = tempdir().unwrap();
        let original = directory.path().join("source.jpg");
        stdfs::write(&original, b"synthetic raw source").unwrap();
        let file = File::open(&original).unwrap();
        let identity = crate::io::SourceFileIdentity::from_file(&file).unwrap();
        let decoder = ProductionPhotoDecoder::with_raw(fake_raw_backend(directory.path()));
        let result = decoder
            .decode_opened(
                file,
                &original,
                &DecodeOptions::default(),
                &CancellationToken::new(),
            )
            .unwrap();
        assert_eq!(result.source_identity, identity);
        assert_eq!(
            result.photo.source_digest(),
            crate::io::SourceDigestV1::from_bytes(b"synthetic raw source")
        );
        assert_eq!(
            result.photo.signal_relation(),
            crate::io::SignalRelation::SceneRelatedRaw
        );
    }

    #[test]
    fn fake_raw_runs_once_through_scene_render_and_publishes_jpeg() {
        use crate::{
            develop::PresetCatalog,
            io::{AlphaPolicy, MetadataPolicy, OutputProfile, OverwritePolicy, SdrRangePolicy},
            job::{
                DevelopJob, DevelopJobOutcome, DevelopJobRunner, DevelopOutput, NoProgress,
                PresetSelection, ReportSignalRelation,
            },
        };

        let directory = tempdir().unwrap();
        let input = directory.path().join("source.nef");
        let output = directory.path().join("developed.jpg");
        stdfs::write(&input, b"synthetic raw source").unwrap();
        let limits = ResourceLimits::default();
        let job = DevelopJob {
            input,
            output: output.clone(),
            decode: DecodeOptions::default(),
            output_options: DevelopOutput::new(
                OutputFormat::Jpeg,
                90,
                OutputProfile::Srgb,
                MetadataPolicy::StripLocation,
                AlphaPolicy::Reject,
                SdrRangePolicy::ClipAndReport,
            ),
            overwrite: OverwritePolicy::Forbid,
            preset: PresetSelection::CatalogId("neutral".to_owned()),
            overrides: Vec::new(),
        };
        let decoder = ProductionPhotoDecoder::with_raw(fake_raw_backend(directory.path()));
        let report = DevelopJobRunner::new(PresetCatalog::built_in().unwrap())
            .run(
                &job,
                &decoder,
                &ProductionJpegEncoder::new(limits),
                &CancellationToken::new(),
                &mut NoProgress,
            )
            .unwrap();

        assert_eq!(
            report.input_signal_relation,
            Some(ReportSignalRelation::SceneRelatedRaw)
        );
        assert_eq!(
            report.output_signal_relation,
            Some(ReportSignalRelation::LinearizedDisplayReferred)
        );
        assert!(report.scene_render.is_some());
        assert!(matches!(
            report.outcome,
            DevelopJobOutcome::PublishedAndDurable { bytes_written } if bytes_written > 0
        ));
        let decoded = image::open(&output).unwrap().to_rgb8();
        assert_eq!(decoded.dimensions(), (1, 1));
    }

    #[test]
    fn corrupt_raster_magic_never_falls_back_to_raw() {
        let directory = tempdir().unwrap();
        let source = directory.path().join("corrupt.nef");
        stdfs::write(&source, b"\xff\xd8\xffbroken").unwrap();
        let decoder = ProductionPhotoDecoder::with_raw(fake_raw_backend(directory.path()));
        assert!(matches!(
            decoder.decode_path_once(
                &source,
                &DecodeOptions::default(),
                &CancellationToken::new()
            ),
            Err(DecodeError::CorruptInput)
                | Err(DecodeError::ColorManagement)
                | Err(DecodeError::Metadata)
        ));
    }

    #[test]
    fn pre_cancelled_job_never_opens_input_or_creates_target() {
        use crate::{
            develop::PresetCatalog,
            io::{AlphaPolicy, MetadataPolicy, OutputProfile, OverwritePolicy, SdrRangePolicy},
            job::{
                DevelopJob, DevelopJobRunner, DevelopOutput, JobErrorCode, NoProgress,
                PresetSelection,
            },
        };
        let directory = tempdir().unwrap();
        let output = directory.path().join("never-created.jpg");
        let limits = ResourceLimits::default();
        let job = DevelopJob {
            input: directory.path().join("does-not-exist.jpg"),
            output: output.clone(),
            decode: DecodeOptions::default(),
            output_options: DevelopOutput::new(
                OutputFormat::Jpeg,
                90,
                OutputProfile::Srgb,
                MetadataPolicy::StripLocation,
                AlphaPolicy::Reject,
                SdrRangePolicy::ClipAndReport,
            ),
            overwrite: OverwritePolicy::Forbid,
            preset: PresetSelection::CatalogId("neutral".to_owned()),
            overrides: Vec::new(),
        };
        let cancellation = CancellationToken::new();
        cancellation.cancel();
        let failure = DevelopJobRunner::new(PresetCatalog::built_in().unwrap())
            .run(
                &job,
                &ProductionPhotoDecoder::new(),
                &ProductionJpegEncoder::new(limits),
                &cancellation,
                &mut NoProgress,
            )
            .unwrap_err();
        assert_eq!(failure.error.code, JobErrorCode::Cancelled);
        assert!(!output.exists());
    }
}
