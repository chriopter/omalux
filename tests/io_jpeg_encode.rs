use std::{fs, path::Path};

use grainroom::{
    develop::{CpuImage, RgbaPixel},
    io::{
        AtomicOutputError, AtomicOutputOptions, EncodeCancellation, EncodeError, EncodeOptions,
        JpegEncodeInput, JpegEncodeRequest, LimitError, MetadataBundle, OverwritePolicy,
        ResourceLimits, SignalRelation, encode_jpeg,
    },
};
use sha2::{Digest, Sha256};

fn image(width: u32, height: u32) -> CpuImage {
    let count = usize::try_from(u64::from(width) * u64::from(height)).unwrap();
    CpuImage::new(
        width,
        height,
        (0..count)
            .map(|index| {
                let value = (index % 13) as f32 / 12.0;
                RgbaPixel::new(value, 0.25, 1.0 - value, 1.0).unwrap()
            })
            .collect(),
    )
    .unwrap()
}

fn request<'a>(
    image: &'a CpuImage,
    metadata: &'a MetadataBundle,
    output: &'a Path,
    limits: &'a ResourceLimits,
    cancellation: &'a EncodeCancellation,
) -> JpegEncodeRequest<'a> {
    JpegEncodeRequest {
        input: JpegEncodeInput {
            image,
            signal_relation: SignalRelation::LinearizedDisplayReferred,
            metadata,
        },
        destination: output,
        source_path: None,
        encode: EncodeOptions::default(),
        atomic: AtomicOutputOptions::default(),
        limits,
        cancellation,
    }
}

#[test]
fn writes_decodable_quality_90_jpeg_with_exact_icc() {
    let directory = tempfile::tempdir().unwrap();
    let output = directory.path().join("out.jpg");
    let report = encode_jpeg(request(
        &image(16, 8),
        &MetadataBundle::default(),
        &output,
        &ResourceLimits::default(),
        &EncodeCancellation::default(),
    ))
    .unwrap();
    assert_eq!(report.quality, 90);
    let bytes = fs::read(&output).unwrap();
    assert_eq!(report.output_bytes, bytes.len() as u64);
    let markers = markers(&bytes);
    let mut icc_parts = markers
        .iter()
        .filter_map(|(marker, data)| {
            (*marker == 0xe2 && data.starts_with(b"ICC_PROFILE\0"))
                .then_some((data[12], &data[14..]))
        })
        .collect::<Vec<_>>();
    icc_parts.sort_by_key(|part| part.0);
    let icc = icc_parts
        .into_iter()
        .flat_map(|part| part.1.iter().copied())
        .collect::<Vec<_>>();
    assert_eq!(<[u8; 32]>::from(Sha256::digest(&icc)), report.icc.sha256);
    let decoder = image::codecs::jpeg::JpegDecoder::new(std::io::Cursor::new(bytes)).unwrap();
    use image::ImageDecoder;
    assert_eq!(decoder.dimensions(), (16, 8));
}

#[test]
fn failures_never_publish_or_leave_temporary_names() {
    let directory = tempfile::tempdir().unwrap();
    let output = directory.path().join("absent.jpg");
    let source = image(2, 2);
    let cancellation = EncodeCancellation::default();
    cancellation.cancel();
    let error = encode_jpeg(request(
        &source,
        &MetadataBundle::default(),
        &output,
        &ResourceLimits::default(),
        &cancellation,
    ))
    .unwrap_err();
    assert!(matches!(error, EncodeError::Cancelled));
    assert_clean(directory.path(), &output);

    let raw = JpegEncodeRequest {
        input: JpegEncodeInput {
            image: &source,
            signal_relation: SignalRelation::SceneRelatedRaw,
            metadata: &MetadataBundle::default(),
        },
        destination: &output,
        source_path: None,
        encode: EncodeOptions::default(),
        atomic: AtomicOutputOptions::default(),
        limits: &ResourceLimits::default(),
        cancellation: &EncodeCancellation::default(),
    };
    assert!(matches!(
        encode_jpeg(raw),
        Err(EncodeError::SceneToDisplayRenderingRequired)
    ));
    assert_clean(directory.path(), &output);
}

#[test]
fn output_limit_and_existing_destination_are_transactional() {
    let directory = tempfile::tempdir().unwrap();
    let output = directory.path().join("bounded.jpg");
    let limits = ResourceLimits::default().with_max_output_bytes(32);
    assert!(matches!(
        encode_jpeg(request(
            &image(16, 16),
            &MetadataBundle::default(),
            &output,
            &limits,
            &EncodeCancellation::default(),
        )),
        Err(EncodeError::Limit(LimitError::OutputBytes { .. }))
    ));
    assert_clean(directory.path(), &output);

    fs::write(&output, b"keep").unwrap();
    assert!(matches!(
        encode_jpeg(request(
            &image(1, 1),
            &MetadataBundle::default(),
            &output,
            &ResourceLimits::default(),
            &EncodeCancellation::default(),
        )),
        Err(EncodeError::Output(AtomicOutputError::DestinationExists))
    ));
    assert_eq!(fs::read(&output).unwrap(), b"keep");
}

#[test]
fn sanitized_exif_is_embedded_without_private_payloads() {
    let directory = tempfile::tempdir().unwrap();
    let output = directory.path().join("metadata.jpg");
    let exif = iso_with_private_tags();
    let private = b"SYNTHETIC_LOCATION_AND_DEVICE_PAYLOAD".to_vec();
    let metadata = MetadataBundle::try_new(
        Some(exif),
        Some(private.clone()),
        Some(b"PRIVATE-IPTC".to_vec()),
        true,
        &ResourceLimits::default(),
    )
    .unwrap();
    let report = encode_jpeg(request(
        &image(2, 2),
        &metadata,
        &output,
        &ResourceLimits::default(),
        &EncodeCancellation::default(),
    ))
    .unwrap();
    assert!(report.metadata.gps_removed);
    assert!(report.metadata.orientation_removed);
    let bytes = fs::read(output).unwrap();
    assert!(!bytes.windows(private.len()).any(|window| window == private));
    assert!(!bytes.windows(7).any(|window| window == b"PRIVATE"));
    let exif_marker = markers(&bytes)
        .into_iter()
        .find(|(marker, data)| *marker == 0xe1 && data.starts_with(b"Exif\0\0"))
        .unwrap();
    assert!(
        exif_marker
            .1
            .windows(2)
            .any(|window| window == 400_u16.to_le_bytes())
    );
}

#[test]
fn jpeg_dimension_limit_is_checked_before_codec() {
    let source = image(65_536, 1);
    let directory = tempfile::tempdir().unwrap();
    let output = directory.path().join("too-wide.jpg");
    assert!(matches!(
        encode_jpeg(request(
            &source,
            &MetadataBundle::default(),
            &output,
            &ResourceLimits::default(),
            &EncodeCancellation::default(),
        )),
        Err(EncodeError::InvalidOptions)
    ));
    assert_clean(directory.path(), &output);
}

#[test]
fn source_output_hardlink_collision_is_preserved() {
    let directory = tempfile::tempdir().unwrap();
    let source_path = directory.path().join("source.jpg");
    let output = directory.path().join("same-inode.jpg");
    fs::write(&source_path, b"source").unwrap();
    fs::hard_link(&source_path, &output).unwrap();
    let source = image(1, 1);
    let metadata = MetadataBundle::default();
    let limits = ResourceLimits::default();
    let cancellation = EncodeCancellation::default();
    let mut request = request(&source, &metadata, &output, &limits, &cancellation);
    request.source_path = Some(&source_path);
    request.atomic = AtomicOutputOptions::default().with_overwrite(OverwritePolicy::Replace);
    assert!(matches!(
        encode_jpeg(request),
        Err(EncodeError::Output(AtomicOutputError::InputOutputCollision))
    ));
    assert_eq!(fs::read(source_path).unwrap(), b"source");
}

fn assert_clean(directory: &Path, output: &Path) {
    assert!(!output.exists());
    assert!(fs::read_dir(directory).unwrap().all(|entry| {
        !entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".grainroom-output-")
    }));
}

fn markers(bytes: &[u8]) -> Vec<(u8, Vec<u8>)> {
    assert!(bytes.starts_with(&[0xff, 0xd8]));
    let mut result = Vec::new();
    let mut offset = 2;
    while offset + 1 < bytes.len() {
        assert_eq!(bytes[offset], 0xff);
        while bytes.get(offset) == Some(&0xff) {
            offset += 1;
        }
        let marker = bytes[offset];
        offset += 1;
        if marker == 0xd9 || marker == 0xda {
            break;
        }
        let length = usize::from(u16::from_be_bytes([bytes[offset], bytes[offset + 1]]));
        let start = offset + 2;
        let end = offset + length;
        result.push((marker, bytes[start..end].to_vec()));
        offset = end;
    }
    result
}

fn iso_with_private_tags() -> Vec<u8> {
    // IFD0 orientation, GPS and Exif pointer. Exif IFD contains ISO and
    // UserComment; only ISO is retained by the exporter.
    let mut bytes = vec![0_u8; 94];
    bytes[..8].copy_from_slice(b"II*\0\x08\0\0\0");
    bytes[8..10].copy_from_slice(&3_u16.to_le_bytes());
    for (index, (tag, kind, count, value)) in [
        (0x0112_u16, 3_u16, 1_u32, 1_u32),
        (0x8825, 4, 1, 90),
        (0x8769, 4, 1, 50),
    ]
    .into_iter()
    .enumerate()
    {
        let at = 10 + index * 12;
        bytes[at..at + 2].copy_from_slice(&tag.to_le_bytes());
        bytes[at + 2..at + 4].copy_from_slice(&kind.to_le_bytes());
        bytes[at + 4..at + 8].copy_from_slice(&count.to_le_bytes());
        bytes[at + 8..at + 12].copy_from_slice(&value.to_le_bytes());
    }
    bytes[50..52].copy_from_slice(&2_u16.to_le_bytes());
    for (index, (tag, kind, count, value)) in [
        (0x8827_u16, 3_u16, 1_u32, 400_u32),
        (0x9286, 7, 4, u32::from_le_bytes(*b"PRIV")),
    ]
    .into_iter()
    .enumerate()
    {
        let at = 52 + index * 12;
        bytes[at..at + 2].copy_from_slice(&tag.to_le_bytes());
        bytes[at + 2..at + 4].copy_from_slice(&kind.to_le_bytes());
        bytes[at + 4..at + 8].copy_from_slice(&count.to_le_bytes());
        bytes[at + 8..at + 12].copy_from_slice(&value.to_le_bytes());
    }
    bytes
}
