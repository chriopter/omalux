use std::{fs, io::Write, path::Path};

use flate2::{Compression, write::ZlibEncoder};
use grainroom::io::{
    ColorProvenance, DecodeError, DecodeOptions, DiagnosticCode, PngSelectedColorSource,
    ResourceLimits, SignalRelation, SourceDigestV1, UnprofiledPolicy,
    color::srgb_profile,
    raster::{RasterCancellation, decode_raster},
};
use image::{
    ExtendedColorType, ImageEncoder,
    codecs::{bmp::BmpEncoder, jpeg::JpegEncoder},
};
use tempfile::TempDir;

fn write_source(directory: &TempDir, name: &str, bytes: &[u8]) -> std::path::PathBuf {
    let path = directory.path().join(name);
    fs::write(&path, bytes).unwrap();
    path
}

fn png_bytes(
    width: u32,
    height: u32,
    color: png::ColorType,
    depth: png::BitDepth,
    pixels: &[u8],
) -> Vec<u8> {
    let mut bytes = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut bytes, width, height);
        encoder.set_color(color);
        encoder.set_depth(depth);
        encoder.set_source_srgb(png::SrgbRenderingIntent::Perceptual);
        let mut writer = encoder.write_header().unwrap();
        writer.write_image_data(pixels).unwrap();
    }
    bytes
}

fn gamma_chrm_png() -> Vec<u8> {
    let mut bytes = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut bytes, 1, 1);
        encoder.set_color(png::ColorType::Rgb);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.set_source_gamma(png::ScaledFloat::from_scaled(45_455));
        encoder.set_source_chromaticities(png::SourceChromaticities::new(
            (0.3127, 0.3290),
            (0.6400, 0.3300),
            (0.3000, 0.6000),
            (0.1500, 0.0600),
        ));
        let mut writer = encoder.write_header().unwrap();
        writer.write_image_data(&[128, 64, 32]).unwrap();
    }
    bytes
}

fn jpeg_bytes(width: u32, height: u32, pixels: &[u8], exif: Option<Vec<u8>>) -> Vec<u8> {
    let mut bytes = Vec::new();
    let mut encoder = JpegEncoder::new_with_quality(&mut bytes, 100);
    if let Some(exif) = exif {
        encoder.set_exif_metadata(exif).unwrap();
    }
    encoder
        .write_image(pixels, width, height, ExtendedColorType::Rgb8)
        .unwrap();
    bytes
}

fn jpeg_with_icc(width: u32, height: u32, pixels: &[u8]) -> Vec<u8> {
    let limits = ResourceLimits::default();
    let icc = srgb_profile(&limits).unwrap().to_icc(&limits).unwrap();
    let mut bytes = Vec::new();
    let mut encoder = JpegEncoder::new_with_quality(&mut bytes, 100);
    encoder.set_icc_profile(icc).unwrap();
    encoder
        .write_image(pixels, width, height, ExtendedColorType::Rgb8)
        .unwrap();
    bytes
}

fn exif(orientation: u16, gps: bool) -> Vec<u8> {
    let count = if gps { 2_u16 } else { 1 };
    let mut bytes = b"II*\0\x08\0\0\0".to_vec();
    bytes.extend_from_slice(&count.to_le_bytes());
    bytes.extend_from_slice(&0x0112_u16.to_le_bytes());
    bytes.extend_from_slice(&3_u16.to_le_bytes());
    bytes.extend_from_slice(&1_u32.to_le_bytes());
    bytes.extend_from_slice(&orientation.to_le_bytes());
    bytes.extend_from_slice(&[0, 0]);
    if gps {
        bytes.extend_from_slice(&0x8825_u16.to_le_bytes());
        bytes.extend_from_slice(&4_u16.to_le_bytes());
        bytes.extend_from_slice(&1_u32.to_le_bytes());
        bytes.extend_from_slice(&8_u32.to_le_bytes());
    }
    bytes.extend_from_slice(&0_u32.to_le_bytes());
    bytes
}

fn duplicate_orientation_exif(first: u16, second: u16) -> Vec<u8> {
    let mut bytes = exif(first, false);
    bytes[8..10].copy_from_slice(&2_u16.to_le_bytes());
    let mut entry = bytes[10..22].to_vec();
    entry[8..10].copy_from_slice(&second.to_le_bytes());
    bytes.splice(22..22, entry);
    bytes
}

fn insert_before_idat(png: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    let mut offset = 8_usize;
    while &png[offset + 4..offset + 8] != b"IDAT" {
        let length = u32::from_be_bytes(png[offset..offset + 4].try_into().unwrap()) as usize;
        offset += 12 + length;
    }
    let mut chunk = Vec::with_capacity(data.len() + 12);
    chunk.extend_from_slice(&(data.len() as u32).to_be_bytes());
    chunk.extend_from_slice(kind);
    chunk.extend_from_slice(data);
    chunk.extend_from_slice(&png_crc(kind, data).to_be_bytes());
    png.splice(offset..offset, chunk);
}

fn png_crc(kind: &[u8; 4], data: &[u8]) -> u32 {
    let mut crc = 0xffff_ffff_u32;
    for byte in kind.iter().chain(data) {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0xedb8_8320_u32 & 0_u32.wrapping_sub(crc & 1));
        }
    }
    !crc
}

fn replace_ihdr_width(png: &mut [u8], width: u32) {
    png[16..20].copy_from_slice(&width.to_be_bytes());
    let crc = png_crc(b"IHDR", &png[16..29]);
    png[29..33].copy_from_slice(&crc.to_be_bytes());
}

fn set_jpeg_sof_components(jpeg: &mut [u8], components: u8) {
    let mut offset = 2_usize;
    loop {
        while jpeg[offset] == 0xff {
            offset += 1;
        }
        let marker = jpeg[offset];
        offset += 1;
        if matches!(
            marker,
            0xc0 | 0xc1
                | 0xc2
                | 0xc3
                | 0xc5
                | 0xc6
                | 0xc7
                | 0xc9
                | 0xca
                | 0xcb
                | 0xcd
                | 0xce
                | 0xcf
        ) {
            jpeg[offset + 7] = components;
            return;
        }
        let length = u16::from_be_bytes(jpeg[offset..offset + 2].try_into().unwrap()) as usize;
        offset += length;
    }
}

fn decode(path: &Path) -> grainroom::io::DecodedPhoto {
    decode_raster(
        path,
        &DecodeOptions::default(),
        &RasterCancellation::default(),
    )
    .unwrap()
}

#[test]
fn png_8bit_srgb_and_16bit_straight_alpha_decode_to_working_space() {
    let directory = TempDir::new().unwrap();
    let rgb = png_bytes(
        2,
        1,
        png::ColorType::Rgb,
        png::BitDepth::Eight,
        &[255, 0, 0, 0, 255, 0],
    );
    let photo = decode(&write_source(&directory, "misleading.raw", &rgb));
    assert_eq!((photo.image().width(), photo.image().height()), (2, 1));
    assert_eq!(
        photo.signal_relation(),
        SignalRelation::LinearizedDisplayReferred
    );
    assert!(matches!(
        photo.color(),
        ColorProvenance::PngDeclared {
            selected: PngSelectedColorSource::Srgb,
            ..
        }
    ));
    assert_eq!(photo.image().pixels()[0].alpha(), 1.0);
    assert!(photo.image().pixels()[0].red() > photo.image().pixels()[0].green());

    let rgba16 = png_bytes(
        1,
        1,
        png::ColorType::Rgba,
        png::BitDepth::Sixteen,
        &[0xff, 0xff, 0, 0, 0x80, 0, 0x80, 0],
    );
    let photo = decode(&write_source(&directory, "alpha.png", &rgba16));
    assert!((photo.image().pixels()[0].alpha() - 32768.0 / 65535.0).abs() < 1.0e-7);
}

#[test]
fn png_icc_has_priority_and_grayscale_has_explicit_policy() {
    let directory = TempDir::new().unwrap();
    let mut rgb = png_bytes(
        1,
        1,
        png::ColorType::Rgb,
        png::BitDepth::Eight,
        &[128, 64, 32],
    );
    let icc = srgb_profile(&ResourceLimits::default())
        .unwrap()
        .to_icc(&ResourceLimits::default())
        .unwrap();
    let mut zlib = ZlibEncoder::new(Vec::new(), Compression::default());
    zlib.write_all(&icc).unwrap();
    let compressed = zlib.finish().unwrap();
    let mut chunk = b"sRGB\0\0".to_vec();
    chunk.extend_from_slice(&compressed);
    insert_before_idat(&mut rgb, b"iCCP", &chunk);
    let photo = decode(&write_source(&directory, "icc.png", &rgb));
    assert!(matches!(
        photo.color(),
        ColorProvenance::PngDeclared {
            selected: PngSelectedColorSource::EmbeddedIcc,
            ..
        }
    ));

    let exif_bytes = exif(1, false);
    let mut combined_metadata = rgb.clone();
    insert_before_idat(&mut combined_metadata, b"eXIf", &exif_bytes);
    let mut total_limited = DecodeOptions::default();
    total_limited.limits.max_icc_bytes = icc.len() as u64;
    total_limited.limits.max_metadata_component_bytes = icc.len() as u64;
    total_limited.limits.max_total_metadata_bytes = icc.len() as u64;
    assert!(matches!(
        decode_raster(
            write_source(&directory, "metadata-total.png", &combined_metadata),
            &total_limited,
            &RasterCancellation::default()
        ),
        Err(DecodeError::Limit(_))
    ));

    let gray = png_bytes(
        1,
        1,
        png::ColorType::Grayscale,
        png::BitDepth::Eight,
        &[128],
    );
    let photo = decode(&write_source(&directory, "gray.png", &gray));
    let pixel = photo.image().pixels()[0];
    assert!((pixel.red() - pixel.green()).abs() < 1.0e-5);
    assert!((pixel.green() - pixel.blue()).abs() < 1.0e-5);

    let mut gray_icc = gray;
    insert_before_idat(&mut gray_icc, b"iCCP", &chunk);
    assert!(matches!(
        decode_raster(
            write_source(&directory, "gray-icc.png", &gray_icc),
            &DecodeOptions::default(),
            &RasterCancellation::default()
        ),
        Err(DecodeError::ColorManagement)
    ));
}

#[test]
fn jpeg_orientation_is_applied_once_and_gps_is_not_retained() {
    let directory = TempDir::new().unwrap();
    let pixels = [255, 0, 0, 0, 255, 0];
    let oriented = jpeg_bytes(2, 1, &pixels, Some(exif(6, false)));
    let photo = decode(&write_source(&directory, "oriented.jpg", &oriented));
    assert_eq!((photo.image().width(), photo.image().height()), (1, 2));
    assert!(photo.metadata().orientation_consumed());
    assert_eq!(photo.metadata().exif().unwrap()[18], 1);

    let gps = jpeg_bytes(2, 1, &pixels, Some(exif(1, true)));
    let photo = decode(&write_source(&directory, "gps.jpg", &gps));
    assert!(photo.metadata().exif().is_none());
    assert!(
        photo
            .diagnostics()
            .iter()
            .any(|item| item.code == DiagnosticCode::MetadataDropped)
    );
}

#[test]
fn jpeg_all_orientations_embedded_icc_and_cmyk_guard_are_explicit() {
    let directory = TempDir::new().unwrap();
    let pixels = [
        255, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 0, 255, 0, 255, 0, 255, 255,
    ];
    for orientation in 1..=8 {
        let jpeg = jpeg_bytes(2, 3, &pixels, Some(exif(orientation, false)));
        let photo = decode(&write_source(
            &directory,
            &format!("orientation-{orientation}.jpg"),
            &jpeg,
        ));
        let expected = if orientation >= 5 { (3, 2) } else { (2, 3) };
        assert_eq!((photo.image().width(), photo.image().height()), expected);
        assert_eq!(photo.metadata().exif().unwrap()[18], 1);
    }

    let embedded = jpeg_with_icc(2, 3, &pixels);
    let photo = decode(&write_source(&directory, "profile.jpg", &embedded));
    assert!(matches!(photo.color(), ColorProvenance::EmbeddedIcc { .. }));

    let mut declared_cmyk = jpeg_bytes(2, 3, &pixels, None);
    set_jpeg_sof_components(&mut declared_cmyk, 4);
    assert!(matches!(
        decode_raster(
            write_source(&directory, "declared-cmyk.jpg", &declared_cmyk),
            &DecodeOptions::default(),
            &RasterCancellation::default()
        ),
        Err(DecodeError::UnsupportedFormat)
    ));

    let duplicate = jpeg_bytes(2, 3, &pixels, Some(duplicate_orientation_exif(6, 3)));
    assert!(matches!(
        decode_raster(
            write_source(&directory, "duplicate-orientation.jpg", &duplicate),
            &DecodeOptions::default(),
            &RasterCancellation::default()
        ),
        Err(DecodeError::Metadata)
    ));
}

#[test]
fn png_declaration_priority_duplicates_crc_and_unsupported_types_are_rejected() {
    let directory = TempDir::new().unwrap();
    let mut cicp = png_bytes(
        1,
        1,
        png::ColorType::Rgb,
        png::BitDepth::Eight,
        &[20, 40, 60],
    );
    insert_before_idat(&mut cicp, b"cICP", &[1, 13, 0, 1]);
    let photo = decode(&write_source(&directory, "cicp.png", &cicp));
    assert!(matches!(
        photo.color(),
        ColorProvenance::PngDeclared {
            selected: PngSelectedColorSource::Cicp,
            ..
        }
    ));

    let photo = decode(&write_source(
        &directory,
        "gamma-chrm.png",
        &gamma_chrm_png(),
    ));
    assert!(matches!(
        photo.color(),
        ColorProvenance::PngDeclared {
            selected: PngSelectedColorSource::ChromaticitiesAndGamma,
            ..
        }
    ));

    let mut duplicate_srgb = png_bytes(1, 1, png::ColorType::Rgb, png::BitDepth::Eight, &[0, 0, 0]);
    insert_before_idat(&mut duplicate_srgb, b"sRGB", &[0]);
    assert!(matches!(
        decode_raster(
            write_source(&directory, "duplicate.png", &duplicate_srgb),
            &DecodeOptions::default(),
            &RasterCancellation::default()
        ),
        Err(DecodeError::ColorManagement)
    ));

    let mut corrupt_crc = png_bytes(1, 1, png::ColorType::Rgb, png::BitDepth::Eight, &[1, 2, 3]);
    let idat = corrupt_crc
        .windows(4)
        .position(|window| window == b"IDAT")
        .unwrap();
    corrupt_crc[idat + 4] ^= 1;
    assert!(matches!(
        decode_raster(
            write_source(&directory, "crc.png", &corrupt_crc),
            &DecodeOptions::default(),
            &RasterCancellation::default()
        ),
        Err(DecodeError::CorruptInput)
    ));

    for (name, raw_color_type) in [("palette.png", 3_u8), ("gray-alpha.png", 4_u8)] {
        let mut bytes = png_bytes(1, 1, png::ColorType::Rgb, png::BitDepth::Eight, &[0, 0, 0]);
        bytes[25] = raw_color_type;
        let crc = png_crc(b"IHDR", &bytes[16..29]);
        bytes[29..33].copy_from_slice(&crc.to_be_bytes());
        assert!(matches!(
            decode_raster(
                write_source(&directory, name, &bytes),
                &DecodeOptions::default(),
                &RasterCancellation::default()
            ),
            Err(DecodeError::UnsupportedFormat)
        ));
    }
}

#[test]
fn png_higher_priority_cicp_ignores_malformed_and_oversized_lower_iccp() {
    let directory = TempDir::new().unwrap();
    for (name, lower) in [
        ("malformed-lower.png", b"not-an-iccp".to_vec()),
        ("oversized-lower.png", vec![0x41; 4096]),
    ] {
        let mut bytes = png_bytes(
            1,
            1,
            png::ColorType::Rgb,
            png::BitDepth::Eight,
            &[10, 20, 30],
        );
        insert_before_idat(&mut bytes, b"iCCP", &lower);
        insert_before_idat(&mut bytes, b"cICP", &[1, 13, 0, 1]);
        let mut options = DecodeOptions::default();
        options.limits.max_icc_bytes = 1024;
        let photo = decode_raster(
            write_source(&directory, name, &bytes),
            &options,
            &RasterCancellation::default(),
        )
        .unwrap();
        assert!(matches!(
            photo.color(),
            ColorProvenance::PngDeclared {
                selected: PngSelectedColorSource::Cicp,
                ..
            }
        ));
    }

    let mut duplicate = png_bytes(1, 1, png::ColorType::Rgb, png::BitDepth::Eight, &[0, 0, 0]);
    insert_before_idat(&mut duplicate, b"cICP", &[1, 13, 0, 1]);
    insert_before_idat(&mut duplicate, b"cICP", &[1, 13, 0, 1]);
    assert!(matches!(
        decode_raster(
            write_source(&directory, "duplicate-selected.png", &duplicate),
            &DecodeOptions::default(),
            &RasterCancellation::default()
        ),
        Err(DecodeError::ColorManagement)
    ));
}

#[test]
fn selected_iccp_rejects_trailing_zlib_and_decompression_bombs() {
    let directory = TempDir::new().unwrap();
    let make_png = |payload: &[u8]| {
        let mut bytes = png_bytes(1, 1, png::ColorType::Rgb, png::BitDepth::Eight, &[0, 0, 0]);
        let mut chunk = b"profile\0\0".to_vec();
        chunk.extend_from_slice(payload);
        insert_before_idat(&mut bytes, b"iCCP", &chunk);
        bytes
    };
    let limits = ResourceLimits::default();
    let icc = srgb_profile(&limits).unwrap().to_icc(&limits).unwrap();
    let mut zlib = ZlibEncoder::new(Vec::new(), Compression::default());
    zlib.write_all(&icc).unwrap();
    let mut trailing = zlib.finish().unwrap();
    trailing.extend_from_slice(b"trailing");
    assert!(matches!(
        decode_raster(
            write_source(&directory, "trailing.png", &make_png(&trailing)),
            &DecodeOptions::default(),
            &RasterCancellation::default()
        ),
        Err(DecodeError::CorruptInput)
    ));

    let mut zlib = ZlibEncoder::new(Vec::new(), Compression::best());
    zlib.write_all(&vec![0_u8; 128 * 1024]).unwrap();
    let bomb = zlib.finish().unwrap();
    let mut options = DecodeOptions::default();
    options.limits.max_icc_bytes = 1024;
    assert!(matches!(
        decode_raster(
            write_source(&directory, "bomb-icc.png", &make_png(&bomb)),
            &options,
            &RasterCancellation::default()
        ),
        Err(DecodeError::Limit(_))
    ));
}

#[test]
fn bmp_padding_and_top_down_rows_are_handled_by_the_decoder() {
    let directory = TempDir::new().unwrap();
    let pixels = [255, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 255];
    let mut bmp = Vec::new();
    BmpEncoder::new(&mut bmp)
        .write_image(&pixels, 2, 2, ExtendedColorType::Rgb8)
        .unwrap();
    let photo = decode(&write_source(&directory, "padded.bmp", &bmp));
    assert_eq!((photo.image().width(), photo.image().height()), (2, 2));

    // Convert a bottom-up 24-bit BMP to top-down while preserving row padding.
    let pixel_offset = u32::from_le_bytes(bmp[10..14].try_into().unwrap()) as usize;
    let row = 8_usize;
    bmp[22..26].copy_from_slice(&(-2_i32).to_le_bytes());
    bmp[pixel_offset..pixel_offset + row * 2].rotate_left(row);
    let top_down = decode(&write_source(&directory, "top-down.bmp", &bmp));
    assert_eq!(top_down.image().pixels().len(), 4);
}

#[test]
fn bmp_v4_v5_unsupported_color_spaces_are_typed_rejections() {
    let directory = TempDir::new().unwrap();
    let mut rgba = Vec::new();
    BmpEncoder::new(&mut rgba)
        .write_image(&[1, 2, 3, 255], 1, 1, ExtendedColorType::Rgba8)
        .unwrap();
    let declared = decode(&write_source(&directory, "srgb-v4.bmp", &rgba));
    assert!(matches!(declared.color(), ColorProvenance::DeclaredSrgb));

    for (name, value) in [
        ("calibrated.bmp", 0_u32),
        ("linked.bmp", 0x4c49_4e4b),
        ("embedded.bmp", 0x4d42_4544),
    ] {
        let mut unsupported = rgba.clone();
        unsupported[70..74].copy_from_slice(&value.to_le_bytes());
        assert!(matches!(
            decode_raster(
                write_source(&directory, name, &unsupported),
                &DecodeOptions::default(),
                &RasterCancellation::default()
            ),
            Err(DecodeError::ColorManagement)
        ));
    }

    let mut v5_profile = rgba;
    v5_profile[14..18].copy_from_slice(&124_u32.to_le_bytes());
    v5_profile.resize(134, 0);
    v5_profile[126..130].copy_from_slice(&124_u32.to_le_bytes());
    v5_profile[130..134].copy_from_slice(&4_u32.to_le_bytes());
    assert!(matches!(
        decode_raster(
            write_source(&directory, "v5-profile.bmp", &v5_profile),
            &DecodeOptions::default(),
            &RasterCancellation::default()
        ),
        Err(DecodeError::ColorManagement)
    ));
}

#[test]
fn strict_unprofiled_cancel_limits_digest_and_corruption_are_stable() {
    let directory = TempDir::new().unwrap();
    let mut bmp = Vec::new();
    BmpEncoder::new(&mut bmp)
        .write_image(&[1, 2, 3], 1, 1, ExtendedColorType::Rgb8)
        .unwrap();
    let path = write_source(&directory, "source.bmp", &bmp);
    let mut strict = DecodeOptions::default();
    strict.unprofiled = UnprofiledPolicy::Reject;
    assert!(matches!(
        decode_raster(&path, &strict, &RasterCancellation::default()),
        Err(DecodeError::ColorManagement)
    ));
    let cancelled = RasterCancellation::default();
    cancelled.cancel();
    assert!(matches!(
        decode_raster(&path, &DecodeOptions::default(), &cancelled),
        Err(DecodeError::Cancelled)
    ));

    let renamed = directory.path().join("renamed.anything");
    fs::rename(&path, &renamed).unwrap();
    assert_eq!(
        decode(&renamed).source_digest(),
        SourceDigestV1::from_bytes(&bmp)
    );
    bmp[54] ^= 1;
    let changed = write_source(&directory, "changed.bmp", &bmp);
    assert_ne!(
        decode(&changed).source_digest(),
        decode(&renamed).source_digest()
    );

    let png = png_bytes(1, 1, png::ColorType::Rgb, png::BitDepth::Eight, &[1, 2, 3]);
    let mut limited = DecodeOptions::default();
    limited.limits.max_pixels = 1;
    let mut bomb = png;
    replace_ihdr_width(&mut bomb, 2);
    assert!(matches!(
        decode_raster(
            write_source(&directory, "bomb.png", &bomb),
            &limited,
            &RasterCancellation::default()
        ),
        Err(DecodeError::Limit(_))
    ));
    assert!(matches!(
        decode_raster(
            write_source(&directory, "truncated.png", b"\x89PNG\r\n\x1a\n"),
            &DecodeOptions::default(),
            &RasterCancellation::default()
        ),
        Err(DecodeError::CorruptInput)
    ));

    let mut source_limited = DecodeOptions::default();
    source_limited.limits.max_source_bytes = 8;
    assert!(matches!(
        decode_raster(&renamed, &source_limited, &RasterCancellation::default()),
        Err(DecodeError::Limit(_))
    ));

    assert!(matches!(
        decode_raster(
            write_source(&directory, "unknown.bin", b"not an image"),
            &DecodeOptions::default(),
            &RasterCancellation::default()
        ),
        Err(DecodeError::UnsupportedFormat)
    ));
}

#[cfg(unix)]
#[test]
fn symlinks_and_non_regular_sources_are_rejected_without_following() {
    use std::os::unix::fs::symlink;
    let directory = TempDir::new().unwrap();
    let target = write_source(
        &directory,
        "target.png",
        &png_bytes(1, 1, png::ColorType::Rgb, png::BitDepth::Eight, &[0, 0, 0]),
    );
    let link = directory.path().join("link.png");
    symlink(target, &link).unwrap();
    assert!(matches!(
        decode_raster(
            link,
            &DecodeOptions::default(),
            &RasterCancellation::default()
        ),
        Err(DecodeError::UnsupportedFormat)
    ));
}
