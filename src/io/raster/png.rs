use std::io::{Cursor, Read};

use crate::io::{
    DecodeError, DecodeOptions, MetadataKind, PngChrmFields, PngCicpFields,
    color::{PngChunk, PngColorDeclarations, resolve_png_color_declarations},
};

use super::{RasterCancellation, RasterPayload, metadata, resolve_missing, validate_dimensions};

pub(super) fn decode(
    bytes: &[u8],
    options: &DecodeOptions,
    cancellation: &RasterCancellation,
) -> Result<RasterPayload, DecodeError> {
    let raw = scan_chunks(bytes, options)?;
    let sixteen = raw.bit_depth == 16;
    let pixels = validate_dimensions(raw.width, raw.height, sixteen, &options.limits)?;
    let gray = raw.color_type == 0;
    if gray && raw.iccp.value.is_some() {
        return Err(DecodeError::ColorManagement);
    }
    let declarations = PngColorDeclarations {
        cicp: raw.cicp,
        iccp: PngChunk {
            value: raw.iccp.value.as_deref(),
            duplicate: raw.iccp.duplicate,
        },
        srgb_rendering_intent: raw.srgb,
        gamma_times_100000: raw.gamma,
        chromaticities_times_100000: raw.chrm,
    };
    let any_declaration = declarations.cicp.value.is_some()
        || declarations.cicp.duplicate
        || declarations.iccp.value.is_some()
        || declarations.iccp.duplicate
        || declarations.srgb_rendering_intent.value.is_some()
        || declarations.srgb_rendering_intent.duplicate
        || declarations.gamma_times_100000.value.is_some()
        || declarations.gamma_times_100000.duplicate
        || declarations.chromaticities_times_100000.value.is_some()
        || declarations.chromaticities_times_100000.duplicate;
    let resolved = if any_declaration {
        let selected = resolve_png_color_declarations(declarations, &options.limits)
            .map_err(super::map_color)?;
        crate::io::color::ResolvedInputProfile {
            profile: selected.profile,
            provenance: selected.provenance,
            diagnostics: selected.diagnostics,
        }
    } else {
        resolve_missing(options)?
    };
    let exif = metadata::normalize_exif(raw.exif.as_deref(), &options.limits)?;
    if cancellation.cancelled() {
        return Err(DecodeError::Cancelled);
    }
    let png_limit = usize::try_from(options.limits.max_working_bytes)
        .unwrap_or(usize::MAX)
        .min(usize::try_from(options.limits.max_decoded_bytes).unwrap_or(usize::MAX));
    let mut decoder =
        png::Decoder::new_with_limits(Cursor::new(bytes), png::Limits { bytes: png_limit });
    decoder.set_ignore_iccp_chunk(true);
    decoder.set_transformations(png::Transformations::IDENTITY);
    let mut reader = decoder.read_info().map_err(|_| DecodeError::CorruptInput)?;
    let info = reader.info();
    if info.width != raw.width
        || info.height != raw.height
        || bit_depth_number(info.bit_depth) != raw.bit_depth
        || color_type_number(info.color_type) != raw.color_type
    {
        return Err(DecodeError::CorruptInput);
    }
    let output_size = reader.output_buffer_size().ok_or(DecodeError::Limit(
        crate::io::LimitError::ArithmeticOverflow,
    ))?;
    let mut decoded = vec![0_u8; output_size];
    let output = reader
        .next_frame(&mut decoded)
        .map_err(|_| DecodeError::CorruptInput)?;
    if output.width != raw.width || output.height != raw.height {
        return Err(DecodeError::CorruptInput);
    }
    let channels = match raw.color_type {
        0 => 1_usize,
        2 => 3,
        6 => 4,
        _ => return Err(DecodeError::UnsupportedFormat),
    };
    let bytes_per_sample = if sixteen { 2 } else { 1 };
    let expected = pixels
        .checked_mul(channels)
        .and_then(|value| value.checked_mul(bytes_per_sample))
        .ok_or(DecodeError::Limit(
            crate::io::LimitError::ArithmeticOverflow,
        ))?;
    if output.buffer_size() != expected || decoded.len() < expected {
        return Err(DecodeError::CorruptInput);
    }
    let row_bytes = usize::try_from(raw.width)
        .ok()
        .and_then(|width| width.checked_mul(channels))
        .and_then(|value| value.checked_mul(bytes_per_sample))
        .ok_or(DecodeError::Limit(
            crate::io::LimitError::ArithmeticOverflow,
        ))?;
    let mut encoded = Vec::with_capacity(pixels);
    for row in decoded[..expected].chunks_exact(row_bytes) {
        if cancellation.cancelled() {
            return Err(DecodeError::Cancelled);
        }
        if sixteen {
            for pixel in row.chunks_exact(channels * 2) {
                let sample = |channel: usize| {
                    f32::from(u16::from_be_bytes([
                        pixel[channel * 2],
                        pixel[channel * 2 + 1],
                    ])) / 65_535.0
                };
                encoded.push(expand(channels, sample));
            }
        } else {
            for pixel in row.chunks_exact(channels) {
                let sample = |channel: usize| f32::from(pixel[channel]) / 255.0;
                encoded.push(expand(channels, sample));
            }
        }
    }
    Ok(RasterPayload {
        width: raw.width,
        height: raw.height,
        encoded,
        resolved,
        exif: exif.bytes,
        orientation: exif.orientation,
        diagnostics: exif.diagnostics,
    })
}

fn expand(channels: usize, sample: impl Fn(usize) -> f32) -> [f32; 4] {
    match channels {
        1 => {
            let gray = sample(0);
            [gray, gray, gray, 1.0]
        }
        3 => [sample(0), sample(1), sample(2), 1.0],
        4 => [sample(0), sample(1), sample(2), sample(3)],
        _ => unreachable!("validated PNG channel count"),
    }
}

struct PngRaw {
    width: u32,
    height: u32,
    bit_depth: u8,
    color_type: u8,
    cicp: PngChunk<PngCicpFields>,
    iccp: PngChunk<Vec<u8>>,
    srgb: PngChunk<u8>,
    gamma: PngChunk<u32>,
    chrm: PngChunk<PngChrmFields>,
    exif: Option<Vec<u8>>,
}

fn scan_chunks(bytes: &[u8], options: &DecodeOptions) -> Result<PngRaw, DecodeError> {
    if !bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Err(DecodeError::CorruptInput);
    }
    let mut offset = 8_usize;
    let mut ihdr = None;
    let mut cicp = PngChunk::absent();
    let mut iccp = PngChunk::absent();
    let mut srgb = PngChunk::absent();
    let mut gamma = PngChunk::absent();
    let mut chrm = PngChunk::absent();
    let mut exif = None;
    let mut saw_idat = false;
    let mut saw_iend = false;
    while offset < bytes.len() {
        let length_bytes: [u8; 4] = bytes
            .get(offset..offset.checked_add(4).ok_or(DecodeError::CorruptInput)?)
            .ok_or(DecodeError::CorruptInput)?
            .try_into()
            .map_err(|_| DecodeError::CorruptInput)?;
        let length = usize::try_from(u32::from_be_bytes(length_bytes))
            .map_err(|_| DecodeError::CorruptInput)?;
        let kind: [u8; 4] = bytes
            .get(offset + 4..offset + 8)
            .ok_or(DecodeError::CorruptInput)?
            .try_into()
            .map_err(|_| DecodeError::CorruptInput)?;
        let data_start = offset + 8;
        let data_end = data_start
            .checked_add(length)
            .ok_or(DecodeError::CorruptInput)?;
        let next = data_end.checked_add(4).ok_or(DecodeError::CorruptInput)?;
        let data = bytes
            .get(data_start..data_end)
            .ok_or(DecodeError::CorruptInput)?;
        let stored_crc: [u8; 4] = bytes
            .get(data_end..next)
            .ok_or(DecodeError::CorruptInput)?
            .try_into()
            .map_err(|_| DecodeError::CorruptInput)?;
        let mut crc = crc32fast::Hasher::new();
        crc.update(&kind);
        crc.update(data);
        if crc.finalize() != u32::from_be_bytes(stored_crc) {
            return Err(DecodeError::CorruptInput);
        }
        offset = next;
        if ihdr.is_none() && &kind != b"IHDR" {
            return Err(DecodeError::CorruptInput);
        }
        if saw_idat
            && matches!(
                &kind,
                b"cICP" | b"sRGB" | b"gAMA" | b"cHRM" | b"iCCP" | b"eXIf"
            )
        {
            return Err(DecodeError::CorruptInput);
        }
        match &kind {
            b"IHDR" => {
                if ihdr.is_some() || offset != 33 || data.len() != 13 {
                    return Err(DecodeError::CorruptInput);
                }
                let width = u32::from_be_bytes(data[0..4].try_into().expect("IHDR width"));
                let height = u32::from_be_bytes(data[4..8].try_into().expect("IHDR height"));
                let depth = data[8];
                let color = data[9];
                if !matches!((color, depth), (0 | 2 | 6, 8 | 16)) || data[10..13] != [0, 0, 0] {
                    return Err(DecodeError::UnsupportedFormat);
                }
                // Reject hostile dimensions before considering any metadata
                // that could itself require bounded decompression/allocation.
                validate_dimensions(width, height, depth == 16, &options.limits)?;
                ihdr = Some((width, height, depth, color));
            }
            b"IDAT" => saw_idat = true,
            b"acTL" | b"tRNS" => return Err(DecodeError::UnsupportedFormat),
            b"cICP" => {
                if data.len() != 4 {
                    return Err(DecodeError::CorruptInput);
                }
                let value = PngCicpFields::try_from_raw(data[0], data[1], data[2], data[3])
                    .map_err(super::map_color)?;
                insert(&mut cicp, value);
            }
            b"sRGB" => {
                if data.len() != 1 {
                    return Err(DecodeError::CorruptInput);
                }
                insert(&mut srgb, data[0]);
            }
            b"gAMA" => {
                if data.len() != 4 {
                    return Err(DecodeError::CorruptInput);
                }
                insert(
                    &mut gamma,
                    u32::from_be_bytes(data.try_into().expect("gAMA")),
                );
            }
            b"cHRM" => {
                if data.len() != 32 {
                    return Err(DecodeError::CorruptInput);
                }
                let field = |index: usize| {
                    u32::from_be_bytes(data[index..index + 4].try_into().expect("cHRM field"))
                };
                insert(
                    &mut chrm,
                    PngChrmFields {
                        white_x: field(0),
                        white_y: field(4),
                        red_x: field(8),
                        red_y: field(12),
                        green_x: field(16),
                        green_y: field(20),
                        blue_x: field(24),
                        blue_y: field(28),
                    },
                );
            }
            b"iCCP" => insert(&mut iccp, decompress_iccp(data, options)?),
            b"eXIf" if exif.is_none() => {
                options
                    .limits
                    .check_metadata_component(MetadataKind::Exif, data.len())
                    .map_err(DecodeError::Limit)?;
                exif = Some(data.to_vec());
            }
            b"eXIf" => return Err(DecodeError::CorruptInput),
            b"IEND" => {
                if !data.is_empty() || next != bytes.len() {
                    return Err(DecodeError::CorruptInput);
                }
                saw_iend = true;
                break;
            }
            _ => {}
        }
    }
    let (width, height, bit_depth, color_type) = ihdr.ok_or(DecodeError::CorruptInput)?;
    if !saw_idat || !saw_iend || width == 0 || height == 0 {
        return Err(DecodeError::CorruptInput);
    }
    Ok(PngRaw {
        width,
        height,
        bit_depth,
        color_type,
        cicp,
        iccp,
        srgb,
        gamma,
        chrm,
        exif,
    })
}

fn insert<T>(chunk: &mut PngChunk<T>, value: T) {
    if chunk.value.is_some() {
        chunk.duplicate = true;
    } else {
        chunk.value = Some(value);
    }
}

fn decompress_iccp(data: &[u8], options: &DecodeOptions) -> Result<Vec<u8>, DecodeError> {
    let nul = data
        .iter()
        .position(|byte| *byte == 0)
        .ok_or(DecodeError::CorruptInput)?;
    if nul == 0 || nul > 79 || data.get(nul + 1) != Some(&0) {
        return Err(DecodeError::CorruptInput);
    }
    let compressed = data.get(nul + 2..).ok_or(DecodeError::CorruptInput)?;
    let maximum = options.limits.max_icc_bytes;
    let mut decoder = flate2::read::ZlibDecoder::new(compressed);
    let mut profile = Vec::new();
    {
        let mut bounded = decoder.by_ref().take(maximum.saturating_add(1));
        bounded
            .read_to_end(&mut profile)
            .map_err(|_| DecodeError::CorruptInput)?;
    }
    options
        .limits
        .check_metadata_component(MetadataKind::Icc, profile.len())
        .map_err(DecodeError::Limit)?;
    let mut trailing = [0_u8; 1];
    if decoder
        .read(&mut trailing)
        .map_err(|_| DecodeError::CorruptInput)?
        != 0
    {
        return Err(DecodeError::Limit(crate::io::LimitError::MetadataBytes {
            kind: MetadataKind::Icc,
            requested: maximum.saturating_add(1),
            maximum,
        }));
    }
    Ok(profile)
}

fn bit_depth_number(depth: png::BitDepth) -> u8 {
    depth as u8
}

fn color_type_number(color: png::ColorType) -> u8 {
    match color {
        png::ColorType::Grayscale => 0,
        png::ColorType::Rgb => 2,
        png::ColorType::Indexed => 3,
        png::ColorType::GrayscaleAlpha => 4,
        png::ColorType::Rgba => 6,
    }
}
