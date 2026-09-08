use std::io::Cursor;

use image::{ColorType, ImageDecoder, codecs::bmp::BmpDecoder};

use super::{
    RasterCancellation, RasterPayload, allocation_error, resolve_missing, try_zeroed,
    validate_dimensions,
};
use crate::io::{ColorProvenance, DecodeError, DecodeOptions, color::ResolvedInputProfile};

pub(super) fn decode(
    bytes: &[u8],
    options: &DecodeOptions,
    cancellation: &RasterCancellation,
) -> Result<RasterPayload, DecodeError> {
    if cancellation.cancelled() {
        return Err(DecodeError::Cancelled);
    }
    let declared_srgb = validate_color_header(bytes)?;
    let mut decoder = BmpDecoder::new(Cursor::new(bytes)).map_err(|_| DecodeError::CorruptInput)?;
    let (width, height) = decoder.dimensions();
    let color = decoder.color_type();
    if !matches!(color, ColorType::Rgb8 | ColorType::Rgba8) {
        return Err(DecodeError::UnsupportedFormat);
    }
    let pixels = validate_dimensions(width, height, false, &options.limits)?;
    let channels = usize::from(color.bytes_per_pixel());
    let expected = pixels.checked_mul(channels).ok_or(DecodeError::Limit(
        crate::io::LimitError::ArithmeticOverflow,
    ))?;
    if decoder.total_bytes() != expected as u64 {
        return Err(DecodeError::CorruptInput);
    }
    decoder
        .set_limits(super::image_limits(&options.limits))
        .map_err(|_| {
            DecodeError::Limit(crate::io::LimitError::WorkingBytes {
                requested: expected as u64,
                maximum: options.limits.max_working_bytes,
            })
        })?;
    let mut decoded = try_zeroed(expected)?;
    decoder
        .read_image(&mut decoded)
        .map_err(|_| DecodeError::CorruptInput)?;
    if cancellation.cancelled() {
        return Err(DecodeError::Cancelled);
    }
    let mut encoded = Vec::new();
    encoded
        .try_reserve_exact(pixels)
        .map_err(|_| allocation_error())?;
    let row_pixels = usize::try_from(width)
        .map_err(|_| DecodeError::Limit(crate::io::LimitError::ArithmeticOverflow))?;
    for (index, pixel) in decoded.chunks_exact(channels).enumerate() {
        if index % row_pixels == 0 && cancellation.cancelled() {
            return Err(DecodeError::Cancelled);
        }
        encoded.push([
            f32::from(pixel[0]) / 255.0,
            f32::from(pixel[1]) / 255.0,
            f32::from(pixel[2]) / 255.0,
            if channels == 4 {
                f32::from(pixel[3]) / 255.0
            } else {
                1.0
            },
        ]);
    }
    Ok(RasterPayload {
        width,
        height,
        encoded,
        resolved: if declared_srgb {
            declared_srgb_profile(options)?
        } else {
            resolve_missing(options)?
        },
        exif: None,
        orientation: 1,
        diagnostics: Vec::new(),
    })
}

fn declared_srgb_profile(options: &DecodeOptions) -> Result<ResolvedInputProfile, DecodeError> {
    let mut resolved = crate::io::color::assumed_srgb_profile(
        crate::io::AssumedProfileReason::MissingProfile,
        &options.limits,
    )
    .map_err(super::map_color)?;
    resolved.provenance = ColorProvenance::DeclaredSrgb;
    resolved.diagnostics.clear();
    Ok(resolved)
}

fn validate_color_header(bytes: &[u8]) -> Result<bool, DecodeError> {
    let dib_size = read_u32(bytes, 14).ok_or(DecodeError::CorruptInput)?;
    if dib_size < 108 {
        return Ok(false);
    }
    let color_space = read_u32(bytes, 14 + 56).ok_or(DecodeError::CorruptInput)?;
    const CALIBRATED_RGB: u32 = 0;
    const SRGB: u32 = 0x7352_4742;
    const WINDOWS: u32 = 0x5769_6e20;
    const PROFILE_LINKED: u32 = 0x4c49_4e4b;
    const PROFILE_EMBEDDED: u32 = 0x4d42_4544;
    if matches!(
        color_space,
        CALIBRATED_RGB | PROFILE_LINKED | PROFILE_EMBEDDED
    ) {
        return Err(DecodeError::ColorManagement);
    }
    if !matches!(color_space, SRGB | WINDOWS) {
        return Err(DecodeError::ColorManagement);
    }
    if dib_size >= 124 {
        let profile_offset = read_u32(bytes, 14 + 112).ok_or(DecodeError::CorruptInput)?;
        let profile_size = read_u32(bytes, 14 + 116).ok_or(DecodeError::CorruptInput)?;
        if profile_offset != 0 || profile_size != 0 {
            return Err(DecodeError::ColorManagement);
        }
    }
    Ok(true)
}

fn read_u32(bytes: &[u8], offset: usize) -> Option<u32> {
    Some(u32::from_le_bytes(
        bytes.get(offset..offset.checked_add(4)?)?.try_into().ok()?,
    ))
}
