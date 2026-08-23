use std::{fs, fs::File, io::Cursor, path::Path};

use image::ImageDecoder;
use omalux::{
    develop::{CpuImage, RgbaPixel},
    io::{
        AlphaPolicy, AssumedProfileReason, AtomicOutputOptions, ColorProvenance, DecodeError,
        DecodeOptions, DecodedPhoto, Diagnostic, EncodeCancellation, EncodeError, EncodeOptions,
        MetadataBundle, MetadataPolicy, OutputFormat, OutputProfile, OverwritePolicy,
        RawMatrixSource, RawProcessingProvenance, ResourceLimits, SdrRangePolicy, SignalRelation,
        SourceDigestV1, WhiteBalanceProvenance, encode_jpeg,
    },
    job::{
        CancellationToken, DecodedSource, DevelopJob, DevelopJobOutcome, DevelopJobRunner,
        DevelopOutput, DisplayReferred, EncodeReceipt, JobErrorCode, JobStage, NoProgress,
        PhotoDecoder, PhotoEncoder, PresetSelection, PublicationRequest, PublicationStatus,
        WorkingArtifact,
    },
};

struct HeldSyntheticDecoder {
    source: File,
    photo: DecodedPhoto,
}

impl PhotoDecoder for HeldSyntheticDecoder {
    type Error = DecodeError;

    fn decode_path_once(
        &self,
        _input: &Path,
        _options: &DecodeOptions,
        _cancellation: &CancellationToken,
    ) -> Result<DecodedSource, Self::Error> {
        DecodedSource::from_held_file(
            self.photo.clone(),
            self.source.try_clone().map_err(DecodeError::Input)?,
        )
        .map_err(|_| DecodeError::InvalidOptions)
    }
}

struct AtomicJpegEncoder<'a> {
    limits: &'a ResourceLimits,
    reported_publication: Option<PublicationStatus>,
}

impl PhotoEncoder for AtomicJpegEncoder<'_> {
    type Error = EncodeError;

    fn encode_display(
        &self,
        publication: PublicationRequest<'_>,
        artifact: &WorkingArtifact<DisplayReferred>,
        options: &EncodeOptions,
        cancellation: &CancellationToken,
    ) -> Result<EncodeReceipt, Self::Error> {
        if cancellation.is_cancelled() {
            return Err(EncodeError::Cancelled);
        }
        let codec_cancellation = EncodeCancellation::default();
        let report = encode_jpeg(omalux::io::JpegEncodeRequest {
            input: omalux::io::JpegEncodeInput {
                image: artifact.image(),
                signal_relation: artifact.signal_relation(),
                metadata: artifact.metadata(),
            },
            destination: publication.destination,
            source_identity: Some(publication.source.identity()),
            encode: *options,
            atomic: AtomicOutputOptions::default().with_overwrite(publication.overwrite),
            limits: self.limits,
            cancellation: &codec_cancellation,
        })?;
        Ok(EncodeReceipt {
            bytes_written: report.output_bytes,
            publication: self
                .reported_publication
                .unwrap_or(PublicationStatus::PublishedAndDurable),
            summary: None,
        })
    }
}

fn photo(relation: SignalRelation) -> DecodedPhoto {
    let image = CpuImage::new(
        3,
        1,
        match relation {
            SignalRelation::LinearizedDisplayReferred => vec![
                RgbaPixel::new(0.1, 0.2, 0.3, 1.0).unwrap(),
                RgbaPixel::new(0.5, 0.4, 0.3, 1.0).unwrap(),
                RgbaPixel::new(0.8, 0.7, 0.6, 1.0).unwrap(),
            ],
            SignalRelation::SceneRelatedRaw => vec![
                RgbaPixel::new(0.18, 0.18, 0.18, 1.0).unwrap(),
                RgbaPixel::new(3.0, 0.4, 0.1, 1.0).unwrap(),
                RgbaPixel::new(-0.1, 0.05, 0.2, 1.0).unwrap(),
            ],
            _ => unreachable!("test constructs only current signal relations"),
        },
    )
    .unwrap();
    let color = match relation {
        SignalRelation::LinearizedDisplayReferred => ColorProvenance::AssumedSrgb {
            reason: AssumedProfileReason::MissingProfile,
        },
        SignalRelation::SceneRelatedRaw => ColorProvenance::RawMatrix {
            matrix: RawMatrixSource::CameraDatabase,
            white_balance: WhiteBalanceProvenance::Camera,
            processing: RawProcessingProvenance::libraw_dcraw_emu(Some("synthetic".to_owned())),
        },
        _ => unreachable!("test constructs only current signal relations"),
    };
    DecodedPhoto::new(
        image,
        MetadataBundle::default(),
        SourceDigestV1::from_bytes(match relation {
            SignalRelation::LinearizedDisplayReferred => b"synthetic display job",
            SignalRelation::SceneRelatedRaw => b"synthetic raw job",
            _ => unreachable!("test constructs only current signal relations"),
        }),
        color,
        relation,
        Vec::<Diagnostic>::new(),
        &ResourceLimits::default(),
    )
    .unwrap()
}

fn job(input: &Path, output: &Path, overwrite: OverwritePolicy) -> DevelopJob {
    DevelopJob {
        input: input.to_owned(),
        output: output.to_owned(),
        decode: DecodeOptions::default(),
        output_options: DevelopOutput::new(
            OutputFormat::Jpeg,
            90,
            OutputProfile::Srgb,
            MetadataPolicy::StripLocation,
            AlphaPolicy::Flatten([0.0, 0.0, 0.0]),
            SdrRangePolicy::Reject,
        ),
        overwrite,
        preset: PresetSelection::CatalogId("neutral".to_owned()),
        overrides: Vec::new(),
    }
}

fn dimensions(path: &Path) -> (u32, u32) {
    let bytes = fs::read(path).unwrap();
    image::codecs::jpeg::JpegDecoder::new(Cursor::new(bytes))
        .unwrap()
        .dimensions()
}

#[test]
fn display_and_raw_jobs_reach_real_atomic_jpeg_with_the_captured_identity() {
    let directory = tempfile::tempdir().unwrap();
    let limits = ResourceLimits::default();
    let runner = DevelopJobRunner::built_in().unwrap();

    for (name, relation) in [
        ("display", SignalRelation::LinearizedDisplayReferred),
        ("raw", SignalRelation::SceneRelatedRaw),
    ] {
        let input = directory.path().join(format!("{name}.source"));
        let output = directory.path().join(format!("{name}.jpg"));
        fs::write(&input, name.as_bytes()).unwrap();
        let decoder = HeldSyntheticDecoder {
            source: File::open(&input).unwrap(),
            photo: photo(relation),
        };
        let report = runner
            .run(
                &job(&input, &output, OverwritePolicy::Forbid),
                &decoder,
                &AtomicJpegEncoder {
                    limits: &limits,
                    reported_publication: None,
                },
                &CancellationToken::new(),
                &mut NoProgress,
            )
            .unwrap();
        assert_eq!(dimensions(&output), (3, 1));
        assert!(matches!(
            report.outcome,
            DevelopJobOutcome::PublishedAndDurable { bytes_written } if bytes_written > 0
        ));
        assert_eq!(
            report.scene_render.is_some(),
            relation == SignalRelation::SceneRelatedRaw
        );
    }
}

#[test]
fn built_in_mask_preset_reaches_the_real_raw_signal_and_jpeg_path() {
    let directory = tempfile::tempdir().unwrap();
    let limits = ResourceLimits::default();
    let runner = DevelopJobRunner::built_in().unwrap();
    let input = directory.path().join("synthetic.raw");
    let output = directory.path().join("mask-preset.jpg");
    fs::write(&input, b"synthetic raw source identity").unwrap();
    let decoder = HeldSyntheticDecoder {
        source: File::open(&input).unwrap(),
        photo: photo(SignalRelation::SceneRelatedRaw),
    };
    let mut request = job(&input, &output, OverwritePolicy::Forbid);
    request.preset = PresetSelection::CatalogId("personal-lampe-1".to_owned());
    request.output_options = DevelopOutput::new(
        OutputFormat::Jpeg,
        90,
        OutputProfile::Srgb,
        MetadataPolicy::StripLocation,
        AlphaPolicy::Flatten([0.0, 0.0, 0.0]),
        SdrRangePolicy::ClipAndReport,
    );
    let report = runner
        .run(
            &request,
            &decoder,
            &AtomicJpegEncoder {
                limits: &limits,
                reported_publication: None,
            },
            &CancellationToken::new(),
            &mut NoProgress,
        )
        .unwrap();
    assert!(report.scene_render.is_some());
    assert!(
        report
            .develop_working_set
            .profile()
            .unwrap()
            .radial_masks_v1
    );
    assert_eq!(dimensions(&output), (3, 1));
}

#[test]
fn hardlink_collision_is_atomic_and_degraded_durability_stays_typed() {
    let directory = tempfile::tempdir().unwrap();
    let limits = ResourceLimits::default();
    let runner = DevelopJobRunner::built_in().unwrap();
    let input = directory.path().join("source");
    let output = directory.path().join("alias.jpg");
    fs::write(&input, b"source remains intact").unwrap();
    fs::hard_link(&input, &output).unwrap();
    let decoder = HeldSyntheticDecoder {
        source: File::open(&input).unwrap(),
        photo: photo(SignalRelation::LinearizedDisplayReferred),
    };
    let failure = runner
        .run(
            &job(&input, &output, OverwritePolicy::Replace),
            &decoder,
            &AtomicJpegEncoder {
                limits: &limits,
                reported_publication: None,
            },
            &CancellationToken::new(),
            &mut NoProgress,
        )
        .unwrap_err();
    assert_eq!(failure.error.stage, JobStage::Encode);
    assert_eq!(failure.error.code, JobErrorCode::DestinationConflict);
    assert_eq!(fs::read(&input).unwrap(), b"source remains intact");
    assert_eq!(fs::read(&output).unwrap(), b"source remains intact");

    let durable_output = directory.path().join("degraded.jpg");
    let report = runner
        .run(
            &job(&input, &durable_output, OverwritePolicy::Forbid),
            &decoder,
            &AtomicJpegEncoder {
                limits: &limits,
                reported_publication: Some(PublicationStatus::PublishedButNotDurable),
            },
            &CancellationToken::new(),
            &mut NoProgress,
        )
        .unwrap();
    assert!(durable_output.is_file());
    assert!(matches!(
        report.outcome,
        DevelopJobOutcome::PublishedButNotDurable { bytes_written } if bytes_written > 0
    ));
}
