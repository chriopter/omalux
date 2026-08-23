use std::{fs, io::Cursor, path::Path};

use image::ImageDecoder;
use omalux::{
    develop::{CpuImage, RgbaPixel},
    io::{
        AtomicOutputError, AtomicOutputOptions, ColorProvenance, DecodedPhoto, EncodeCancellation,
        EncodeError, EncodeOptions, JpegEncodeInput, JpegEncodeRequest, MetadataBundle,
        ResourceLimits, SdrRangePolicy, SignalRelation, SourceDigestV1,
        color::SceneToDisplayTransform, encode_jpeg,
    },
};

fn options() -> EncodeOptions {
    let mut options = EncodeOptions::default();
    options.range = SdrRangePolicy::Reject;
    options
}

fn request<'a>(
    image: &'a CpuImage,
    relation: SignalRelation,
    metadata: &'a MetadataBundle,
    destination: &'a Path,
    limits: &'a ResourceLimits,
    cancellation: &'a EncodeCancellation,
) -> JpegEncodeRequest<'a> {
    JpegEncodeRequest {
        input: JpegEncodeInput {
            image,
            signal_relation: relation,
            metadata,
        },
        destination,
        source_identity: None,
        encode: options(),
        atomic: AtomicOutputOptions::default(),
        limits,
        cancellation,
    }
}

#[test]
fn raw_like_artifact_must_render_to_display_before_atomic_jpeg() {
    let limits = ResourceLimits::default();
    let raw = CpuImage::new(
        3,
        1,
        vec![
            RgbaPixel::new(0.18, 0.18, 0.18, 1.0).unwrap(),
            RgbaPixel::new(3.0, 0.4, 0.1, 1.0).unwrap(),
            RgbaPixel::new(-0.1, 0.05, 0.2, 1.0).unwrap(),
        ],
    )
    .unwrap();
    let metadata = MetadataBundle::default();
    let directory = tempfile::tempdir().unwrap();
    let rejected = directory.path().join("direct.jpg");
    assert!(matches!(
        encode_jpeg(request(
            &raw,
            SignalRelation::SceneRelatedRaw,
            &metadata,
            &rejected,
            &limits,
            &EncodeCancellation::default(),
        )),
        Err(EncodeError::SceneToDisplayRenderingRequired)
    ));
    assert!(!rejected.exists());

    let mut display_pixels = vec![RgbaPixel::new(0.0, 0.0, 0.0, 1.0).unwrap(); raw.pixels().len()];
    let scene_report = SceneToDisplayTransform::new()
        .transform_scanline(
            raw.pixels(),
            &mut display_pixels,
            SignalRelation::SceneRelatedRaw,
            &limits,
        )
        .unwrap();
    assert_eq!(
        scene_report.output_signal_relation,
        SignalRelation::LinearizedDisplayReferred
    );
    let display = CpuImage::new(raw.width(), raw.height(), display_pixels).unwrap();
    let output = directory.path().join("rendered.jpg");
    let report = encode_jpeg(request(
        &display,
        scene_report.output_signal_relation,
        &metadata,
        &output,
        &limits,
        &EncodeCancellation::default(),
    ))
    .unwrap();
    assert_eq!((report.width, report.height), (3, 1));
    let decoder =
        image::codecs::jpeg::JpegDecoder::new(Cursor::new(fs::read(output).unwrap())).unwrap();
    assert_eq!(decoder.dimensions(), (3, 1));
}

#[test]
fn raster_display_artifact_encodes_and_atomic_forbid_preserves_existing() {
    let limits = ResourceLimits::default();
    let raster = DecodedPhoto::new(
        CpuImage::new(
            2,
            1,
            vec![
                RgbaPixel::new(0.1, 0.2, 0.3, 1.0).unwrap(),
                RgbaPixel::new(0.8, 0.7, 0.6, 1.0).unwrap(),
            ],
        )
        .unwrap(),
        MetadataBundle::default(),
        SourceDigestV1::from_bytes(b"synthetic raster pipeline"),
        ColorProvenance::DeclaredSrgb,
        SignalRelation::LinearizedDisplayReferred,
        Vec::new(),
        &limits,
    )
    .unwrap();
    let directory = tempfile::tempdir().unwrap();
    let output = directory.path().join("raster.jpg");
    encode_jpeg(request(
        raster.image(),
        raster.signal_relation(),
        raster.metadata(),
        &output,
        &limits,
        &EncodeCancellation::default(),
    ))
    .unwrap();
    let original = fs::read(&output).unwrap();
    assert!(matches!(
        encode_jpeg(request(
            raster.image(),
            raster.signal_relation(),
            raster.metadata(),
            &output,
            &limits,
            &EncodeCancellation::default(),
        )),
        Err(EncodeError::Output(AtomicOutputError::DestinationExists))
    ));
    assert_eq!(fs::read(output).unwrap(), original);
}
