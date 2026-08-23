use std::io::Cursor;

use image::{ColorType, ImageDecoder, codecs::bmp::BmpDecoder};

use super::{RasterCancellation, RasterPayload, resolve_missing, validate_dimensions};
use crate::io::{DecodeError, DecodeOptions};

pub(super) fn decode(
    bytes: &[u8],
    options: &DecodeOptions,
    cancellation: &RasterCancellation,
) -> Result<RasterPayload, DecodeError> {
    if cancellation.cancelled() {
        return Err(DecodeError::Cancelled);
    }
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
    let mut decoded = vec![0_u8; expected];
    decoder
        .read_image(&mut decoded)
        .map_err(|_| DecodeError::CorruptInput)?;
    if cancellation.cancelled() {
        return Err(DecodeError::Cancelled);
    }
    let encoded = decoded
        .chunks_exact(channels)
        .map(|pixel| {
            [
                f32::from(pixel[0]) / 255.0,
                f32::from(pixel[1]) / 255.0,
                f32::from(pixel[2]) / 255.0,
                if channels == 4 {
                    f32::from(pixel[3]) / 255.0
                } else {
                    1.0
                },
            ]
        })
        .collect();
    Ok(RasterPayload {
        width,
        height,
        encoded,
        resolved: resolve_missing(options)?,
        exif: None,
        orientation: 1,
        diagnostics: Vec::new(),
    })
}
