use std::{path::Path, sync::LazyLock};

#[cfg(feature = "heic")]
use std::fs;

use grainroom::{
    develop::{CpuImage, RgbaPixel},
    io::{
        AtomicOutputOptions, EncodeCancellation, EncodeError, EncodeOptions,
        EncodeWorkingSetProfile, HeicEncodeRequest, JpegEncodeInput, JpegMetadataFootprint,
        LimitError, MetadataBundle, OutputFormat, ResourceLimits, SignalRelation, encode_heic,
        probe_heic_capability,
    },
};

fn image(width: u32, height: u32) -> CpuImage {
    let count = (u64::from(width) * u64::from(height)) as usize;
    CpuImage::new(
        width,
        height,
        (0..count)
            .map(|index| {
                let value = (index % 17) as f32 / 16.0;
                RgbaPixel::new(value, 0.3, 1.0 - value, 1.0).unwrap()
            })
            .collect(),
    )
    .unwrap()
}

fn request<'a>(
    image: &'a CpuImage,
    output: &'a Path,
    limits: &'a ResourceLimits,
    cancellation: &'a EncodeCancellation,
) -> HeicEncodeRequest<'a> {
    static METADATA: LazyLock<MetadataBundle> = LazyLock::new(MetadataBundle::default);
    let mut encode = EncodeOptions::default();
    encode.format = OutputFormat::Heic;
    HeicEncodeRequest {
        input: JpegEncodeInput {
            image,
            signal_relation: SignalRelation::LinearizedDisplayReferred,
            metadata: &METADATA,
        },
        destination: output,
        source_identity: None,
        encode,
        atomic: AtomicOutputOptions::default(),
        limits,
        cancellation,
    }
}

#[cfg(feature = "heic")]
#[test]
fn x265_capability_and_odd_sized_atomic_encode_work() {
    let capability = probe_heic_capability().unwrap();
    assert!(capability.encoder.to_ascii_lowercase().contains("x265"));
    assert!(capability.eight_bit && capability.ten_bit);

    let directory = tempfile::tempdir().unwrap();
    let output = directory.path().join("odd.heic");
    let report = encode_heic(request(
        &image(17, 9),
        &output,
        &ResourceLimits::default(),
        &EncodeCancellation::default(),
    ))
    .unwrap();
    let bytes = fs::read(output).unwrap();
    assert_eq!(report.output_bytes, bytes.len() as u64);
    assert!(bytes.windows(4).any(|window| window == b"ftyp"));
}

#[cfg(feature = "heic")]
#[test]
fn raw_cancel_and_output_limits_are_transactional() {
    let directory = tempfile::tempdir().unwrap();
    let output = directory.path().join("bounded.heic");
    let source = image(32, 32);
    let limits = ResourceLimits::default().with_max_output_bytes(64);
    assert!(matches!(
        encode_heic(request(
            &source,
            &output,
            &limits,
            &EncodeCancellation::default()
        )),
        Err(EncodeError::Limit(LimitError::OutputBytes { .. }))
    ));
    assert!(!output.exists());

    let default_limits = ResourceLimits::default();
    let default_cancellation = EncodeCancellation::default();
    let mut raw = request(&source, &output, &default_limits, &default_cancellation);
    raw.input.signal_relation = SignalRelation::SceneRelatedRaw;
    assert!(matches!(
        encode_heic(raw),
        Err(EncodeError::SceneToDisplayRenderingRequired)
    ));
    assert!(!output.exists());
}

#[test]
fn native_working_allowance_accepts_exact_peak_and_rejects_peak_minus_one() {
    let metadata = JpegMetadataFootprint {
        input_metadata_bytes: 0,
        output_exif_bytes: 0,
        output_icc_bytes: 588,
        transform_profile_bytes: 1156,
    };
    let estimate = ResourceLimits::default()
        .estimate_encode_working_set(17, 9, EncodeWorkingSetProfile::HeicRgb8X265, metadata)
        .unwrap();
    let exact = ResourceLimits::default().with_max_working_bytes(estimate.peak_bytes);
    assert_eq!(
        exact
            .estimate_encode_working_set(17, 9, EncodeWorkingSetProfile::HeicRgb8X265, metadata)
            .unwrap()
            .peak_bytes,
        estimate.peak_bytes
    );
    let below = ResourceLimits::default().with_max_working_bytes(estimate.peak_bytes - 1);
    assert!(matches!(
        below.estimate_encode_working_set(17, 9, EncodeWorkingSetProfile::HeicRgb8X265, metadata),
        Err(LimitError::WorkingBytes { .. })
    ));
}

#[cfg(not(feature = "heic"))]
#[test]
fn disabled_feature_fails_without_touching_output() {
    let directory = tempfile::tempdir().unwrap();
    let output = directory.path().join("disabled.heic");
    assert!(matches!(
        encode_heic(request(
            &image(1, 1),
            &output,
            &ResourceLimits::default(),
            &EncodeCancellation::default()
        )),
        Err(EncodeError::HeicBackendNotBuilt)
    ));
    assert!(!output.exists());
    assert!(matches!(
        probe_heic_capability(),
        Err(EncodeError::HeicBackendNotBuilt)
    ));
}
