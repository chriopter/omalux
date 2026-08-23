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
    develop::{CpuImage, RgbaPixel},
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
    let neutral = RgbaPixel::new(0.0, 0.0, 0.0, 1.0).map_err(|_| DecodeError::CorruptInput)?;
    let mut working = Vec::new();
    working
        .try_reserve_exact(payload.encoded.len())
        .map_err(|_| allocation_error())?;
    working.resize(payload.encoded.len(), neutral);
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
    let image = apply_orientation(image, payload.orientation, cancellation)?;
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

fn apply_orientation(
    image: CpuImage,
    value: u8,
    cancellation: &RasterCancellation,
) -> Result<CpuImage, DecodeError> {
    if cancellation.cancelled() {
        return Err(DecodeError::Cancelled);
    }
    if value == 1 {
        return Ok(image);
    }
    let (width, height) = if value >= 5 {
        (image.height(), image.width())
    } else {
        (image.width(), image.height())
    };
    let count = usize::try_from(u64::from(width) * u64::from(height))
        .map_err(|_| DecodeError::Limit(crate::io::LimitError::ArithmeticOverflow))?;
    let mut pixels = Vec::new();
    pixels
        .try_reserve_exact(count)
        .map_err(|_| allocation_error())?;
    for y in 0..height {
        if cancellation.cancelled() {
            return Err(DecodeError::Cancelled);
        }
        for x in 0..width {
            let (source_x, source_y) = match value {
                2 => (image.width() - 1 - x, y),
                3 => (image.width() - 1 - x, image.height() - 1 - y),
                4 => (x, image.height() - 1 - y),
                5 => (y, x),
                6 => (y, image.height() - 1 - x),
                7 => (image.width() - 1 - y, image.height() - 1 - x),
                8 => (image.width() - 1 - y, x),
                _ => return Err(DecodeError::Metadata),
            };
            let index = usize::try_from(
                u64::from(source_y) * u64::from(image.width()) + u64::from(source_x),
            )
            .map_err(|_| DecodeError::Limit(crate::io::LimitError::ArithmeticOverflow))?;
            pixels.push(image.pixels()[index]);
        }
    }
    CpuImage::new(width, height, pixels).map_err(|_| DecodeError::CorruptInput)
}

fn allocation_error() -> DecodeError {
    DecodeError::Limit(crate::io::LimitError::Allocation)
}

fn try_zeroed(length: usize) -> Result<Vec<u8>, DecodeError> {
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(length)
        .map_err(|_| allocation_error())?;
    bytes.resize(length, 0);
    Ok(bytes)
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
