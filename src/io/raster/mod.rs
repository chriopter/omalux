//! Bounded JPEG, PNG and BMP decoding into linear Rec.2020 working pixels.

mod bmp;
mod jpeg;
mod metadata;
mod png;
mod source;

use std::{
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use crate::{
    develop::{
        CpuImage, RgbaPixel,
        orientation::{ExifOrientation, apply_exif_orientation},
    },
    io::{
        DecodeError, DecodeOptions, DecodedPhoto, DecodedPhotoError, Diagnostic, MetadataBundle,
        ResourceLimits, SignalRelation,
        color::{ColorError, RasterToWorkingTransform, ResolvedInputProfile},
    },
};

#[derive(Clone, Default, Debug)]
pub struct RasterCancellation(Arc<AtomicBool>);

impl RasterCancellation {
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Release);
    }

    fn cancelled(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
}

struct RasterPayload {
    width: u32,
    height: u32,
    encoded: Vec<[f32; 4]>,
    resolved: ResolvedInputProfile,
    exif: Option<Vec<u8>>,
    orientation: u8,
    diagnostics: Vec<Diagnostic>,
}

/// Opens the source exactly once, sniffs its signature, and decodes the same
/// bounded byte buffer that produced `source_digest`.
pub fn decode_raster(
    source: impl AsRef<Path>,
    options: &DecodeOptions,
    cancellation: &RasterCancellation,
) -> Result<DecodedPhoto, DecodeError> {
    options.validate()?;
    let buffered = source::read_once(source.as_ref(), &options.limits, || {
        cancellation.cancelled()
    })?;
    let mut payload = match sniff(&buffered.bytes)? {
        RasterFormat::Jpeg => jpeg::decode(&buffered.bytes, options, cancellation)?,
        RasterFormat::Png => png::decode(&buffered.bytes, options, cancellation)?,
        RasterFormat::Bmp => bmp::decode(&buffered.bytes, options, cancellation)?,
    };
    if cancellation.cancelled() {
        return Err(DecodeError::Cancelled);
    }
    let transform = RasterToWorkingTransform::new(&payload.resolved.profile, &options.limits)
        .map_err(map_color)?;
    let mut working =
        vec![RgbaPixel::new(0.0, 0.0, 0.0, 1.0).expect("finite neutral"); payload.encoded.len()];
    let width = usize::try_from(payload.width)
        .map_err(|_| DecodeError::Limit(crate::io::LimitError::ArithmeticOverflow))?;
    for (source_row, destination_row) in
        payload.encoded.chunks(width).zip(working.chunks_mut(width))
    {
        if cancellation.cancelled() {
            return Err(DecodeError::Cancelled);
        }
        transform
            .transform_scanline(source_row, destination_row, &options.limits)
            .map_err(map_color)?;
    }
    // Avoid retaining the encoded staging buffer while orientation may
    // allocate another full-size f32 image.
    drop(std::mem::take(&mut payload.encoded));
    let image = CpuImage::new(payload.width, payload.height, working)
        .map_err(|_| DecodeError::CorruptInput)?;
    let image = apply_orientation(image, payload.orientation)?;
    let metadata = MetadataBundle::try_new(payload.exif.take(), None, None, true, &options.limits)
        .map_err(DecodeError::Limit)?;
    payload
        .diagnostics
        .append(&mut payload.resolved.diagnostics);
    DecodedPhoto::new(
        image,
        metadata,
        buffered.digest,
        payload.resolved.provenance,
        SignalRelation::LinearizedDisplayReferred,
        payload.diagnostics,
        &options.limits,
    )
    .map_err(map_photo)
}

fn apply_orientation(image: CpuImage, value: u8) -> Result<CpuImage, DecodeError> {
    let orientation = match value {
        1 => ExifOrientation::Normal,
        2 => ExifOrientation::MirrorHorizontal,
        3 => ExifOrientation::Rotate180,
        4 => ExifOrientation::MirrorVertical,
        5 => ExifOrientation::Transpose,
        6 => ExifOrientation::Rotate90Clockwise,
        7 => ExifOrientation::Transverse,
        8 => ExifOrientation::Rotate270Clockwise,
        _ => ExifOrientation::Normal,
    };
    apply_exif_orientation(&image, orientation).map_err(|_| DecodeError::CorruptInput)
}

fn resolve_missing(options: &DecodeOptions) -> Result<ResolvedInputProfile, DecodeError> {
    match options.unprofiled {
        crate::io::UnprofiledPolicy::AssumeSrgbAndWarn => crate::io::color::assumed_srgb_profile(
            crate::io::AssumedProfileReason::MissingProfile,
            &options.limits,
        )
        .map_err(map_color),
        crate::io::UnprofiledPolicy::Reject => Err(DecodeError::ColorManagement),
    }
}

fn map_color(error: ColorError) -> DecodeError {
    match error {
        ColorError::Limit(error) => DecodeError::Limit(error),
        _ => DecodeError::ColorManagement,
    }
}

fn map_photo(error: DecodedPhotoError) -> DecodeError {
    match error {
        DecodedPhotoError::Limit(error) => DecodeError::Limit(error),
        DecodedPhotoError::ColorRelationMismatch => DecodeError::ColorManagement,
        DecodedPhotoError::Image(_) => DecodeError::CorruptInput,
    }
}

enum RasterFormat {
    Jpeg,
    Png,
    Bmp,
}

fn sniff(bytes: &[u8]) -> Result<RasterFormat, DecodeError> {
    if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        Ok(RasterFormat::Jpeg)
    } else if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Ok(RasterFormat::Png)
    } else if bytes.starts_with(b"BM") {
        Ok(RasterFormat::Bmp)
    } else {
        Err(DecodeError::UnsupportedFormat)
    }
}

fn validate_dimensions(
    width: u32,
    height: u32,
    sixteen_bit: bool,
    limits: &ResourceLimits,
) -> Result<usize, DecodeError> {
    let profile = if sixteen_bit {
        crate::io::DecodeWorkingSetProfile::RasterRgba16
    } else {
        crate::io::DecodeWorkingSetProfile::RasterRgba8
    };
    limits
        .estimate_working_set(width, height, profile)
        .map_err(DecodeError::Limit)?;
    usize::try_from(u64::from(width) * u64::from(height))
        .map_err(|_| DecodeError::Limit(crate::io::LimitError::ArithmeticOverflow))
}

fn image_limits(limits: &ResourceLimits) -> image::Limits {
    let mut image_limits = image::Limits::default();
    image_limits.max_image_width = Some(u32::try_from(limits.max_pixels).unwrap_or(u32::MAX));
    image_limits.max_image_height = Some(u32::try_from(limits.max_pixels).unwrap_or(u32::MAX));
    image_limits.max_alloc = Some(limits.max_working_bytes.min(limits.max_decoded_bytes));
    image_limits
}
