use std::io::{Cursor, Read};

use crate::io::{
    DecodeError, DecodeOptions, MetadataKind, PngChrmFields, PngCicpFields,
    color::{PngChunk, PngColorDeclarations, resolve_png_color_declarations},
};

use super::{
    RasterCancellation, RasterPayload, allocation_error, metadata, resolve_missing, try_zeroed,
    validate_dimensions,
};

pub(super) fn decode(
    bytes: &[u8],
    options: &DecodeOptions,
    cancellation: &RasterCancellation,
) -> Result<RasterPayload, DecodeError> {
    let raw = scan_chunks(bytes, options, cancellation)?;
    let sixteen = raw.bit_depth == 16;
    let pixels = validate_dimensions(raw.width, raw.height, sixteen, &options.limits)?;
    let gray = raw.color_type == 0;
    let iccp_selected = raw.cicp.value.is_none()
        && !raw.cicp.duplicate
        && (raw.iccp.value.is_some() || raw.iccp.duplicate);
    if gray && iccp_selected {
        return Err(DecodeError::ColorManagement);
    }
    let exif_slice = raw.exif.map(|location| location.data(bytes));
    let icc = if iccp_selected && !raw.iccp.duplicate {
        let location = raw.iccp.value.ok_or(DecodeError::CorruptInput)?;
        Some(decompress_selected_iccp(
            location.data(bytes),
            exif_slice.map_or(0, <[u8]>::len),
            options,
            cancellation,
        )?)
    } else {
        None
    };
    let declarations = PngColorDeclarations {
        cicp: raw.cicp,
        iccp: PngChunk {
            value: icc.as_deref(),
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
    let exif = metadata::normalize_exif(exif_slice, &options.limits, cancellation)?;
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
    let mut decoded = try_zeroed(output_size)?;
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
    let mut encoded = Vec::new();
    encoded
        .try_reserve_exact(pixels)
        .map_err(|_| allocation_error())?;
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

#[derive(Clone, Copy, Debug)]
struct ChunkLocation {
    start: usize,
    end: usize,
}

impl ChunkLocation {
    fn data(self, bytes: &[u8]) -> &[u8] {
        &bytes[self.start..self.end]
    }
}

struct PngRaw {
    width: u32,
    height: u32,
    bit_depth: u8,
    color_type: u8,
    cicp: PngChunk<PngCicpFields>,
    iccp: PngChunk<ChunkLocation>,
    srgb: PngChunk<u8>,
    gamma: PngChunk<u32>,
    chrm: PngChunk<PngChrmFields>,
    exif: Option<ChunkLocation>,
}

fn scan_chunks(
    bytes: &[u8],
    options: &DecodeOptions,
    cancellation: &RasterCancellation,
) -> Result<PngRaw, DecodeError> {
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
        if cancellation.cancelled() {
            return Err(DecodeError::Cancelled);
        }
        let kind_start = offset.checked_add(4).ok_or(DecodeError::CorruptInput)?;
        let kind_end = kind_start.checked_add(4).ok_or(DecodeError::CorruptInput)?;
        let length_bytes: [u8; 4] = bytes
            .get(offset..offset.checked_add(4).ok_or(DecodeError::CorruptInput)?)
            .ok_or(DecodeError::CorruptInput)?
            .try_into()
            .map_err(|_| DecodeError::CorruptInput)?;
        let length = usize::try_from(u32::from_be_bytes(length_bytes))
            .map_err(|_| DecodeError::CorruptInput)?;
        let kind: [u8; 4] = bytes
            .get(kind_start..kind_end)
            .ok_or(DecodeError::CorruptInput)?
            .try_into()
            .map_err(|_| DecodeError::CorruptInput)?;
        let data_start = offset.checked_add(8).ok_or(DecodeError::CorruptInput)?;
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
        for chunk in data.chunks(64 * 1024) {
            if cancellation.cancelled() {
                return Err(DecodeError::Cancelled);
            }
            crc.update(chunk);
        }
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
            // First pass records structure only. The selected iCCP is the only
            // declaration inflated during the resolution pass.
            b"iCCP" => insert(
                &mut iccp,
                ChunkLocation {
                    start: data_start,
                    end: data_end,
                },
            ),
            b"eXIf" if exif.is_none() => {
                options
                    .limits
                    .check_metadata_component(MetadataKind::Exif, data.len())
                    .map_err(DecodeError::Limit)?;
                options
                    .limits
                    .check_metadata_total(u64::try_from(data.len()).map_err(|_| {
                        DecodeError::Limit(crate::io::LimitError::ArithmeticOverflow)
                    })?)
                    .map_err(DecodeError::Limit)?;
                exif = Some(ChunkLocation {
                    start: data_start,
                    end: data_end,
                });
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

fn decompress_selected_iccp(
    data: &[u8],
    exif_bytes: usize,
    options: &DecodeOptions,
    cancellation: &RasterCancellation,
) -> Result<Vec<u8>, DecodeError> {
    if cancellation.cancelled() {
        return Err(DecodeError::Cancelled);
    }
    // PNG limits profile names to 1..=79 bytes. Never search an attacker-sized
    // compressed payload for the separator: byte 80 must already be NUL.
    let keyword_window = &data[..data.len().min(80)];
    let nul = keyword_window
        .iter()
        .position(|byte| *byte == 0)
        .ok_or(DecodeError::CorruptInput)?;
    if nul == 0 || nul > 79 || data.get(nul + 1) != Some(&0) {
        return Err(DecodeError::CorruptInput);
    }
    let compressed = data.get(nul + 2..).ok_or(DecodeError::CorruptInput)?;
    options
        .limits
        .check_metadata_component(MetadataKind::Icc, compressed.len())
        .map_err(DecodeError::Limit)?;
    let exif_u64 = u64::try_from(exif_bytes)
        .map_err(|_| DecodeError::Limit(crate::io::LimitError::ArithmeticOverflow))?;
    let compressed_u64 = u64::try_from(compressed.len())
        .map_err(|_| DecodeError::Limit(crate::io::LimitError::ArithmeticOverflow))?;
    options
        .limits
        .check_metadata_total(
            exif_u64
                .checked_add(compressed_u64)
                .ok_or(DecodeError::Limit(
                    crate::io::LimitError::ArithmeticOverflow,
                ))?,
        )
        .map_err(DecodeError::Limit)?;
    let maximum = options.limits.max_icc_bytes.min(
        options
            .limits
            .max_total_metadata_bytes
            .saturating_sub(exif_u64),
    );
    let peak = compressed_u64
        .checked_add(maximum)
        .and_then(|value| value.checked_add(exif_u64))
        .ok_or(DecodeError::Limit(
            crate::io::LimitError::ArithmeticOverflow,
        ))?;
    if peak > options.limits.max_working_bytes {
        return Err(DecodeError::Limit(crate::io::LimitError::WorkingBytes {
            requested: peak,
            maximum: options.limits.max_working_bytes,
        }));
    }
    let mut decoder = flate2::bufread::ZlibDecoder::new(Cursor::new(compressed));
    let mut profile = Vec::new();
    let mut chunk = [0_u8; 32 * 1024];
    loop {
        if cancellation.cancelled() {
            return Err(DecodeError::Cancelled);
        }
        let count = decoder
            .read(&mut chunk)
            .map_err(|_| DecodeError::CorruptInput)?;
        if count == 0 {
            break;
        }
        let next = u64::try_from(profile.len())
            .map_err(|_| DecodeError::Limit(crate::io::LimitError::ArithmeticOverflow))?
            .checked_add(
                u64::try_from(count)
                    .map_err(|_| DecodeError::Limit(crate::io::LimitError::ArithmeticOverflow))?,
            )
            .ok_or(DecodeError::Limit(
                crate::io::LimitError::ArithmeticOverflow,
            ))?;
        if next > maximum {
            return Err(DecodeError::Limit(crate::io::LimitError::MetadataBytes {
                kind: MetadataKind::Icc,
                requested: next,
                maximum,
            }));
        }
        // `next` was checked against the exact logical bound above. Request
        // only the bytes needed for this output chunk; never use geometric
        // spare-capacity growth for hostile compressed metadata.
        profile
            .try_reserve_exact(count)
            .map_err(|_| allocation_error())?;
        profile.extend_from_slice(&chunk[..count]);
    }
    if usize::try_from(decoder.get_ref().position()).ok() != Some(compressed.len()) {
        return Err(DecodeError::CorruptInput);
    }
    options
        .limits
        .check_metadata_component(MetadataKind::Icc, profile.len())
        .map_err(DecodeError::Limit)?;
    options
        .limits
        .check_metadata_total(
            exif_u64
                .checked_add(
                    u64::try_from(profile.len()).map_err(|_| {
                        DecodeError::Limit(crate::io::LimitError::ArithmeticOverflow)
                    })?,
                )
                .ok_or(DecodeError::Limit(
                    crate::io::LimitError::ArithmeticOverflow,
                ))?,
        )
        .map_err(DecodeError::Limit)?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::{Compression, write::ZlibEncoder};
    use std::io::Write;

    #[test]
    fn selected_iccp_inflate_observes_cancellation() {
        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::best());
        encoder.write_all(&vec![7_u8; 128 * 1024]).unwrap();
        let compressed = encoder.finish().unwrap();
        let mut data = b"profile\0\0".to_vec();
        data.extend_from_slice(&compressed);
        let cancellation = RasterCancellation::default();
        cancellation.cancel();
        assert!(matches!(
            decompress_selected_iccp(&data, 0, &DecodeOptions::default(), &cancellation),
            Err(DecodeError::Cancelled)
        ));
    }

    #[test]
    fn iccp_keyword_scan_never_enters_an_attacker_sized_payload() {
        let cancellation = RasterCancellation::default();
        let no_nul = vec![b'x'; 2 * 1024 * 1024];
        assert!(matches!(
            decompress_selected_iccp(&no_nul, 0, &DecodeOptions::default(), &cancellation),
            Err(DecodeError::CorruptInput)
        ));

        let mut late_nul = no_nul;
        late_nul[80] = 0;
        assert!(matches!(
            decompress_selected_iccp(&late_nul, 0, &DecodeOptions::default(), &cancellation),
            Err(DecodeError::CorruptInput)
        ));
    }

    #[test]
    fn selected_iccp_uses_exact_non_power_of_two_peak_and_payload_budget() {
        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::best());
        encoder.write_all(&vec![3_u8; 997]).unwrap();
        let compressed = encoder.finish().unwrap();
        let mut data = b"profile\0\0".to_vec();
        data.extend_from_slice(&compressed);
        let exif_bytes = 7_usize;
        let exact_peak = compressed.len() as u64 + 1_003 + exif_bytes as u64;
        let mut options = DecodeOptions::default();
        options.limits.max_icc_bytes = 1_003;
        options.limits.max_working_bytes = exact_peak;
        assert_eq!(
            decompress_selected_iccp(&data, exif_bytes, &options, &RasterCancellation::default())
                .unwrap()
                .len(),
            997
        );

        options.limits.max_working_bytes = exact_peak - 1;
        assert!(matches!(
            decompress_selected_iccp(&data, exif_bytes, &options, &RasterCancellation::default()),
            Err(DecodeError::Limit(
                crate::io::LimitError::WorkingBytes { .. }
            ))
        ));

        let mut oversized = b"profile\0\0".to_vec();
        oversized.extend(std::iter::repeat_n(0_u8, 1_004));
        options.limits.max_working_bytes = u64::MAX;
        assert!(matches!(
            decompress_selected_iccp(&oversized, 0, &options, &RasterCancellation::default()),
            Err(DecodeError::Limit(
                crate::io::LimitError::MetadataBytes { .. }
            ))
        ));
    }

    #[test]
    fn cross_thread_cancellation_preempts_large_iccp_validation() {
        use std::sync::{Arc, Barrier};

        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::best());
        encoder.write_all(&vec![0x4d; 8 * 1024 * 1024]).unwrap();
        let compressed = encoder.finish().unwrap();
        let mut data = b"profile\0\0".to_vec();
        data.extend_from_slice(&compressed);
        let mut options = DecodeOptions::default();
        options.limits.max_icc_bytes = 8 * 1024 * 1024 + 1;
        let cancellation = RasterCancellation::default();
        let trigger = Arc::new(Barrier::new(2));
        let worker_cancellation = cancellation.clone();
        let worker_trigger = Arc::clone(&trigger);
        let worker = std::thread::spawn(move || {
            worker_trigger.wait();
            decompress_selected_iccp(&data, 0, &options, &worker_cancellation)
        });
        trigger.wait();
        cancellation.cancel();
        let result = worker.join().unwrap();
        assert!(matches!(result, Err(DecodeError::Cancelled)));
    }
}
