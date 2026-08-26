use std::{io::Cursor, mem::size_of};

use image::{ImageDecoder, codecs::jpeg::JpegDecoder};

use super::{
    RasterCancellation, RasterPayload, allocation_error, metadata, resolve_missing, try_zeroed,
    validate_dimensions,
};
use crate::io::{
    AssumedProfileReason, DecodeError, DecodeOptions,
    color::{ColorError, assumed_srgb_profile, embedded_rgb_profile},
};

pub(super) fn decode(
    bytes: &[u8],
    options: &DecodeOptions,
    cancellation: &RasterCancellation,
) -> Result<RasterPayload, DecodeError> {
    let markers = scan_markers(bytes, options, cancellation)?;
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
    let exif = metadata::normalize_exif(markers.exif.as_deref(), &options.limits, cancellation)?;
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
    let row_samples = usize::try_from(width)
        .ok()
        .and_then(|width| width.checked_mul(channels))
        .ok_or(DecodeError::Limit(
            crate::io::LimitError::ArithmeticOverflow,
        ))?;
    if channels == 3 {
        for (index, rgb) in decoded.as_chunks::<3>().0.iter().enumerate() {
            if index
                .checked_mul(3)
                .is_some_and(|sample| sample % row_samples == 0)
                && cancellation.cancelled()
            {
                return Err(DecodeError::Cancelled);
            }
            encoded.push([
                f32::from(rgb[0]) / 255.0,
                f32::from(rgb[1]) / 255.0,
                f32::from(rgb[2]) / 255.0,
                1.0,
            ]);
        }
    } else {
        for (index, gray) in decoded.into_iter().enumerate() {
            if index % row_samples == 0 && cancellation.cancelled() {
                return Err(DecodeError::Cancelled);
            }
            let gray = f32::from(gray) / 255.0;
            encoded.push([gray, gray, gray, 1.0]);
        }
    }
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

#[derive(Clone, Copy)]
struct IccPart {
    start: usize,
    end: usize,
}

fn scan_markers(
    bytes: &[u8],
    options: &DecodeOptions,
    cancellation: &RasterCancellation,
) -> Result<JpegMarkers, DecodeError> {
    if !bytes.starts_with(&[0xff, 0xd8]) {
        return Err(DecodeError::CorruptInput);
    }
    let mut offset = 2_usize;
    let mut components = None;
    let mut exif = None;
    // ICC APP2 payloads remain ranges into the immutable source buffer until
    // validation is complete. Only the final, exact-size profile is copied.
    let mut icc_parts: Vec<Option<IccPart>> = Vec::new();
    let mut icc_count = None;
    let mut icc_total = 0_usize;
    while offset < bytes.len() {
        if cancellation.cancelled() {
            return Err(DecodeError::Cancelled);
        }
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
            let total = u64::try_from(raw.len())
                .ok()
                .and_then(|value| value.checked_add(u64::try_from(icc_total).ok()?))
                .ok_or(DecodeError::Limit(
                    crate::io::LimitError::ArithmeticOverflow,
                ))?;
            options
                .limits
                .check_metadata_total(total)
                .map_err(DecodeError::Limit)?;
            check_metadata_working_peak(icc_total, raw.len(), icc_parts.len(), options)?;
            let mut copy = Vec::new();
            copy.try_reserve_exact(raw.len())
                .map_err(|_| allocation_error())?;
            for chunk in raw.chunks(64 * 1024) {
                if cancellation.cancelled() {
                    return Err(DecodeError::Cancelled);
                }
                copy.extend_from_slice(chunk);
            }
            exif = Some(copy);
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
                check_metadata_working_peak(icc_total, exif_len(&exif), count, options)?;
                icc_parts
                    .try_reserve_exact(count)
                    .map_err(|_| allocation_error())?;
                icc_parts.resize_with(count, || None);
            }
            let part = &data[14..];
            if icc_parts[sequence - 1].is_some() {
                return Err(DecodeError::CorruptInput);
            }
            icc_total = icc_total.checked_add(part.len()).ok_or(DecodeError::Limit(
                crate::io::LimitError::ArithmeticOverflow,
            ))?;
            options
                .limits
                .check_metadata_component(crate::io::MetadataKind::Icc, icc_total)
                .map_err(DecodeError::Limit)?;
            let metadata_total = u64::try_from(icc_total)
                .ok()
                .and_then(|value| {
                    value.checked_add(u64::try_from(exif.as_ref().map_or(0, Vec::len)).ok()?)
                })
                .ok_or(DecodeError::Limit(
                    crate::io::LimitError::ArithmeticOverflow,
                ))?;
            options
                .limits
                .check_metadata_total(metadata_total)
                .map_err(DecodeError::Limit)?;
            check_metadata_working_peak(icc_total, exif_len(&exif), icc_parts.len(), options)?;
            icc_parts[sequence - 1] = Some(IccPart {
                start: start.checked_add(14).ok_or(DecodeError::CorruptInput)?,
                end,
            });
        }
    }
    let icc = if let Some(count) = icc_count {
        if icc_parts.len() != count || icc_parts.iter().any(Option::is_none) {
            return Err(DecodeError::CorruptInput);
        }
        Some(assemble_icc(bytes, &icc_parts, icc_total, cancellation)?)
    } else {
        None
    };
    let metadata_total = icc
        .as_ref()
        .map_or(0_u64, |value| value.len() as u64)
        .checked_add(exif.as_ref().map_or(0_u64, |value| value.len() as u64))
        .ok_or(DecodeError::Limit(
            crate::io::LimitError::ArithmeticOverflow,
        ))?;
    options
        .limits
        .check_metadata_total(metadata_total)
        .map_err(DecodeError::Limit)?;
    Ok(JpegMarkers {
        components,
        icc,
        exif,
    })
}

fn exif_len(exif: &Option<Vec<u8>>) -> usize {
    exif.as_ref().map_or(0, Vec::len)
}

fn check_metadata_working_peak(
    icc_bytes: usize,
    exif_bytes: usize,
    part_count: usize,
    options: &DecodeOptions,
) -> Result<(), DecodeError> {
    let descriptors =
        part_count
            .checked_mul(size_of::<Option<IccPart>>())
            .ok_or(DecodeError::Limit(
                crate::io::LimitError::ArithmeticOverflow,
            ))?;
    let requested = u64::try_from(icc_bytes)
        .ok()
        .and_then(|value| value.checked_add(u64::try_from(exif_bytes).ok()?))
        .and_then(|value| value.checked_add(u64::try_from(descriptors).ok()?))
        .ok_or(DecodeError::Limit(
            crate::io::LimitError::ArithmeticOverflow,
        ))?;
    if requested > options.limits.max_working_bytes {
        return Err(DecodeError::Limit(crate::io::LimitError::WorkingBytes {
            requested,
            maximum: options.limits.max_working_bytes,
        }));
    }
    Ok(())
}

fn assemble_icc(
    bytes: &[u8],
    parts: &[Option<IccPart>],
    total: usize,
    cancellation: &RasterCancellation,
) -> Result<Vec<u8>, DecodeError> {
    let mut assembled = Vec::new();
    assembled
        .try_reserve_exact(total)
        .map_err(|_| allocation_error())?;
    for part in parts {
        let part = part.ok_or(DecodeError::CorruptInput)?;
        let source = bytes
            .get(part.start..part.end)
            .ok_or(DecodeError::CorruptInput)?;
        for chunk in source.chunks(16 * 1024) {
            if cancellation.cancelled() {
                return Err(DecodeError::Cancelled);
            }
            assembled.extend_from_slice(chunk);
        }
    }
    if assembled.len() != total {
        return Err(DecodeError::CorruptInput);
    }
    Ok(assembled)
}

fn is_sof(marker: u8) -> bool {
    matches!(
        marker,
        0xc0 | 0xc1 | 0xc2 | 0xc3 | 0xc5 | 0xc6 | 0xc7 | 0xc9 | 0xca | 0xcb | 0xcd | 0xce | 0xcf
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn marker(bytes: &mut Vec<u8>, marker: u8, data: &[u8]) {
        bytes.extend_from_slice(&[0xff, marker]);
        bytes.extend_from_slice(&u16::try_from(data.len() + 2).unwrap().to_be_bytes());
        bytes.extend_from_slice(data);
    }

    fn jpeg_with_icc_parts(parts: &[&[u8]]) -> Vec<u8> {
        let mut bytes = vec![0xff, 0xd8];
        for (index, part) in parts.iter().enumerate() {
            let mut data = b"ICC_PROFILE\0".to_vec();
            data.push(u8::try_from(index + 1).unwrap());
            data.push(u8::try_from(parts.len()).unwrap());
            data.extend_from_slice(part);
            marker(&mut bytes, 0xe2, &data);
        }
        marker(&mut bytes, 0xc0, &[8, 0, 1, 0, 1, 3]);
        bytes.extend_from_slice(&[0xff, 0xda]);
        bytes
    }

    #[test]
    fn multipart_icc_has_one_exact_final_copy_and_checked_peak() {
        let first = vec![0x31; 101];
        let second = vec![0x72; 102];
        let bytes = jpeg_with_icc_parts(&[&first, &second]);
        let descriptor_bytes = 2 * size_of::<Option<IccPart>>();
        let exact_peak = u64::try_from(first.len() + second.len() + descriptor_bytes).unwrap();
        let mut options = DecodeOptions::default();
        options.limits.max_icc_bytes = 211;
        options.limits.max_working_bytes = exact_peak;
        let markers = scan_markers(&bytes, &options, &RasterCancellation::default()).unwrap();
        let mut expected = first;
        expected.extend_from_slice(&second);
        assert_eq!(markers.icc.unwrap(), expected);

        options.limits.max_working_bytes = exact_peak - 1;
        assert!(matches!(
            scan_markers(&bytes, &options, &RasterCancellation::default()),
            Err(DecodeError::Limit(
                crate::io::LimitError::WorkingBytes { .. }
            ))
        ));
    }

    #[test]
    fn icc_part_assembly_polls_cancellation_before_copying() {
        let bytes = vec![0x5a; 32 * 1024];
        let parts = [Some(IccPart {
            start: 0,
            end: bytes.len(),
        })];
        let cancellation = RasterCancellation::default();
        cancellation.cancel();
        assert!(matches!(
            assemble_icc(&bytes, &parts, bytes.len(), &cancellation),
            Err(DecodeError::Cancelled)
        ));
    }
}
