use std::io::Cursor;

use image::{ImageDecoder, codecs::jpeg::JpegDecoder};

use super::{RasterCancellation, RasterPayload, metadata, resolve_missing, validate_dimensions};
use crate::io::{
    AssumedProfileReason, DecodeError, DecodeOptions,
    color::{ColorError, assumed_srgb_profile, embedded_rgb_profile},
};

pub(super) fn decode(
    bytes: &[u8],
    options: &DecodeOptions,
    cancellation: &RasterCancellation,
) -> Result<RasterPayload, DecodeError> {
    let markers = scan_markers(bytes, options)?;
    match markers.components {
        Some(1 | 3) => {}
        Some(_) => return Err(DecodeError::UnsupportedFormat),
        None => return Err(DecodeError::CorruptInput),
    }
    if markers.components == Some(1) && markers.icc.is_some() {
        // This path deliberately supports RGB input profiles only. Treating an
        // RGB profile as a grayscale profile would be colorimetrically false.
        return Err(DecodeError::ColorManagement);
    }
    let resolved = match markers.icc {
        Some(icc) => match embedded_rgb_profile(&icc, &options.limits) {
            Ok(profile) => profile,
            Err(ColorError::Limit(error)) => return Err(DecodeError::Limit(error)),
            Err(_) => match options.unprofiled {
                crate::io::UnprofiledPolicy::AssumeSrgbAndWarn => {
                    assumed_srgb_profile(AssumedProfileReason::UnsupportedProfile, &options.limits)
                        .map_err(super::map_color)?
                }
                crate::io::UnprofiledPolicy::Reject => {
                    return Err(DecodeError::ColorManagement);
                }
            },
        },
        None => resolve_missing(options)?,
    };
    let exif = metadata::normalize_exif(markers.exif.as_deref(), &options.limits)?;
    if cancellation.cancelled() {
        return Err(DecodeError::Cancelled);
    }
    let mut decoder =
        JpegDecoder::new(Cursor::new(bytes)).map_err(|_| DecodeError::CorruptInput)?;
    let (width, height) = decoder.dimensions();
    let pixels = validate_dimensions(width, height, false, &options.limits)?;
    let requested = decoder.total_bytes();
    decoder
        .set_limits(super::image_limits(&options.limits))
        .map_err(|_| {
            DecodeError::Limit(crate::io::LimitError::WorkingBytes {
                requested,
                maximum: options.limits.max_working_bytes,
            })
        })?;
    let channels = usize::from(markers.components.unwrap());
    let expected = pixels.checked_mul(channels).ok_or(DecodeError::Limit(
        crate::io::LimitError::ArithmeticOverflow,
    ))?;
    if decoder.total_bytes() != expected as u64 {
        return Err(DecodeError::CorruptInput);
    }
    let mut decoded = vec![0_u8; expected];
    decoder
        .read_image(&mut decoded)
        .map_err(|_| DecodeError::CorruptInput)?;
    if cancellation.cancelled() {
        return Err(DecodeError::Cancelled);
    }
    let encoded = if channels == 3 {
        decoded
            .chunks_exact(3)
            .map(|rgb| {
                [
                    f32::from(rgb[0]) / 255.0,
                    f32::from(rgb[1]) / 255.0,
                    f32::from(rgb[2]) / 255.0,
                    1.0,
                ]
            })
            .collect()
    } else {
        decoded
            .into_iter()
            .map(|gray| {
                let gray = f32::from(gray) / 255.0;
                [gray, gray, gray, 1.0]
            })
            .collect()
    };
    Ok(RasterPayload {
        width,
        height,
        encoded,
        resolved,
        exif: exif.bytes,
        orientation: exif.orientation,
        diagnostics: exif.diagnostics,
    })
}

struct JpegMarkers {
    components: Option<u8>,
    icc: Option<Vec<u8>>,
    exif: Option<Vec<u8>>,
}

fn scan_markers(bytes: &[u8], options: &DecodeOptions) -> Result<JpegMarkers, DecodeError> {
    if !bytes.starts_with(&[0xff, 0xd8]) {
        return Err(DecodeError::CorruptInput);
    }
    let mut offset = 2_usize;
    let mut components = None;
    let mut exif = None;
    let mut icc_parts: Vec<Option<Vec<u8>>> = Vec::new();
    let mut icc_count = None;
    let mut icc_total = 0_usize;
    while offset < bytes.len() {
        if bytes[offset] != 0xff {
            return Err(DecodeError::CorruptInput);
        }
        while bytes.get(offset) == Some(&0xff) {
            offset += 1;
        }
        let marker = *bytes.get(offset).ok_or(DecodeError::CorruptInput)?;
        offset += 1;
        if marker == 0xda || marker == 0xd9 {
            break;
        }
        if marker == 0x01 || (0xd0..=0xd8).contains(&marker) {
            continue;
        }
        let length_bytes: [u8; 2] = bytes
            .get(offset..offset.checked_add(2).ok_or(DecodeError::CorruptInput)?)
            .ok_or(DecodeError::CorruptInput)?
            .try_into()
            .map_err(|_| DecodeError::CorruptInput)?;
        let length = usize::from(u16::from_be_bytes(length_bytes));
        if length < 2 {
            return Err(DecodeError::CorruptInput);
        }
        let start = offset + 2;
        let end = offset
            .checked_add(length)
            .ok_or(DecodeError::CorruptInput)?;
        let data = bytes.get(start..end).ok_or(DecodeError::CorruptInput)?;
        offset = end;
        if is_sof(marker) {
            let found = *data.get(5).ok_or(DecodeError::CorruptInput)?;
            if components.replace(found).is_some() {
                return Err(DecodeError::CorruptInput);
            }
        } else if marker == 0xe1 && data.starts_with(b"Exif\0\0") {
            if exif.is_some() {
                return Err(DecodeError::CorruptInput);
            }
            let raw = &data[6..];
            options
                .limits
                .check_metadata_component(crate::io::MetadataKind::Exif, raw.len())
                .map_err(DecodeError::Limit)?;
            exif = Some(raw.to_vec());
        } else if marker == 0xe2 && data.starts_with(b"ICC_PROFILE\0") {
            let sequence = usize::from(*data.get(12).ok_or(DecodeError::CorruptInput)?);
            let count = usize::from(*data.get(13).ok_or(DecodeError::CorruptInput)?);
            if sequence == 0 || count == 0 || sequence > count {
                return Err(DecodeError::CorruptInput);
            }
            if icc_count.is_some_and(|known| known != count) {
                return Err(DecodeError::CorruptInput);
            }
            icc_count = Some(count);
            if icc_parts.is_empty() {
                icc_parts.resize_with(count, || None);
            }
            let part = &data[14..];
            icc_total = icc_total.checked_add(part.len()).ok_or(DecodeError::Limit(
                crate::io::LimitError::ArithmeticOverflow,
            ))?;
            options
                .limits
                .check_metadata_component(crate::io::MetadataKind::Icc, icc_total)
                .map_err(DecodeError::Limit)?;
            if icc_parts[sequence - 1].replace(part.to_vec()).is_some() {
                return Err(DecodeError::CorruptInput);
            }
        }
    }
    let icc = if let Some(count) = icc_count {
        if icc_parts.len() != count || icc_parts.iter().any(Option::is_none) {
            return Err(DecodeError::CorruptInput);
        }
        let mut assembled = Vec::new();
        assembled
            .try_reserve_exact(icc_total)
            .map_err(|_| DecodeError::Limit(crate::io::LimitError::ArithmeticOverflow))?;
        for part in icc_parts {
            assembled.extend_from_slice(part.as_deref().expect("validated complete ICC"));
        }
        Some(assembled)
    } else {
        None
    };
    Ok(JpegMarkers {
        components,
        icc,
        exif,
    })
}

fn is_sof(marker: u8) -> bool {
    matches!(
        marker,
        0xc0 | 0xc1 | 0xc2 | 0xc3 | 0xc5 | 0xc6 | 0xc7 | 0xc9 | 0xca | 0xcb | 0xcd | 0xce | 0xcf
    )
}
