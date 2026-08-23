use std::{
    path::Path,
    sync::{Arc, Mutex},
};

use grainroom::{
    develop::{
        CpuImage, DevelopSettings, DevelopWorkingSetProfile, LocalAdjustments, ParameterOverride,
        PresetDocument, RadialMask, RgbaPixel, estimate_develop_working_set,
    },
    io::{
        AlphaPolicy, AssumedProfileReason, ColorProvenance, DecodeError, DecodeOptions,
        DecodedPhoto, Diagnostic, EncodeError, EncodeOptions, MetadataBundle, MetadataPolicy,
        OutputFormat, OutputProfile, OverwritePolicy, ResourceLimits, SdrRangePolicy,
        SignalRelation, SourceDigestV1, SourceFileIdentity,
    },
    job::{
        CancellationToken, DecodedSource, DevelopJob, DevelopJobRunner, DevelopOutput,
        EncodeReceipt, JobErrorCode, JobStage, PhotoDecoder, PhotoEncoder, PresetSelection,
        ProgressSink, PublicationRequest, PublicationStatus, WorkingArtifact,
    },
};

#[derive(Clone)]
struct FakeDecoder {
    photo: DecodedPhoto,
    cancel_after_decode: bool,
    calls: Arc<Mutex<Vec<&'static str>>>,
    source_identity: SourceFileIdentity,
}

impl PhotoDecoder for FakeDecoder {
    type Error = DecodeError;

    fn decode_path_once(
        &self,
        _input: &Path,
        _options: &DecodeOptions,
        cancellation: &CancellationToken,
    ) -> Result<DecodedSource, Self::Error> {
        self.calls.lock().unwrap().push("decode");
        if self.cancel_after_decode {
            cancellation.cancel();
        }
        Ok(DecodedSource {
            photo: self.photo.clone(),
            source_identity: self.source_identity,
        })
    }
}

#[derive(Clone)]
struct FakeEncoder {
    calls: Arc<Mutex<Vec<&'static str>>>,
    observed: Arc<Mutex<Vec<(SignalRelation, f32, Vec<[u32; 4]>)>>>,
    publication: PublicationStatus,
    cancel_on_publish: bool,
}

impl Default for FakeEncoder {
    fn default() -> Self {
        Self {
            calls: Arc::default(),
            observed: Arc::default(),
            publication: PublicationStatus::PublishedAndDurable,
            cancel_on_publish: false,
        }
    }
}

impl PhotoEncoder for FakeEncoder {
    type Error = EncodeError;

    fn encode_display(
        &self,
        _publication: PublicationRequest<'_>,
        artifact: &WorkingArtifact<grainroom::job::DisplayReferred>,
        _options: &EncodeOptions,
        cancellation: &CancellationToken,
    ) -> Result<EncodeReceipt, Self::Error> {
        self.calls.lock().unwrap().push("encode");
        let pixels = artifact
            .image()
            .pixels()
            .iter()
            .map(|pixel| {
                [
                    pixel.red().to_bits(),
                    pixel.green().to_bits(),
                    pixel.blue().to_bits(),
                    pixel.alpha().to_bits(),
                ]
            })
            .collect();
        self.observed.lock().unwrap().push((
            artifact.signal_relation(),
            artifact.image().pixels()[0].red(),
            pixels,
        ));
        if self.cancel_on_publish {
            cancellation.cancel();
        }
        Ok(EncodeReceipt {
            bytes_written: 37,
            publication: self.publication,
        })
    }
}

#[derive(Default)]
struct Stages(Vec<JobStage>);

impl ProgressSink for Stages {
    fn stage_completed(&mut self, stage: JobStage) {
        self.0.push(stage);
    }
}

fn image(red: f32) -> CpuImage {
    CpuImage::new(
        2,
        1,
        vec![
            RgbaPixel::new(red, red, red, 0.25).unwrap(),
            RgbaPixel::new(red * 2.0, red, red / 2.0, 1.0).unwrap(),
        ],
    )
    .unwrap()
}

fn decoded() -> DecodedPhoto {
    let relation = SignalRelation::LinearizedDisplayReferred;
    let color = ColorProvenance::AssumedSrgb {
        reason: AssumedProfileReason::MissingProfile,
    };
    DecodedPhoto::new(
        image(0.18),
        MetadataBundle::default(),
        SourceDigestV1::from_bytes(b"fake source bytes"),
        color,
        relation,
        Vec::<Diagnostic>::new(),
        &ResourceLimits::default(),
    )
    .unwrap()
}

fn source_identity() -> SourceFileIdentity {
    SourceFileIdentity::from_file(&tempfile::tempfile().unwrap()).unwrap()
}

fn job() -> DevelopJob {
    DevelopJob {
        input: "/virtual/input.raw".into(),
        output: "/virtual/output.jpg".into(),
        decode: DecodeOptions::default(),
        output_options: DevelopOutput::new(
            OutputFormat::Jpeg,
            90,
            OutputProfile::Srgb,
            MetadataPolicy::StripLocation,
            AlphaPolicy::Flatten([0.0, 0.0, 0.0]),
            SdrRangePolicy::Reject,
        ),
        overwrite: OverwritePolicy::Forbid,
        preset: PresetSelection::CatalogId("neutral".to_owned()),
        overrides: Vec::new(),
    }
}

#[test]
fn display_job_bypasses_scene_render() {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let decoder = FakeDecoder {
        photo: decoded(),
        cancel_after_decode: false,
        calls: calls.clone(),
        source_identity: source_identity(),
    };
    let encoder = FakeEncoder {
        calls: calls.clone(),
        ..FakeEncoder::default()
    };
    let request = job();
    let mut stages = Stages::default();
    let report = DevelopJobRunner::built_in()
        .unwrap()
        .run(
            &request,
            &decoder,
            &encoder,
            &CancellationToken::new(),
            &mut stages,
        )
        .unwrap();

    assert_eq!(*calls.lock().unwrap(), ["decode", "encode"]);
    assert_eq!(encoder.observed.lock().unwrap()[0].1, 0.18);
    assert!(report.scene_render.is_none());
    assert_eq!(
        stages.0,
        [
            JobStage::Validate,
            JobStage::Decode,
            JobStage::ResolveSettings,
            JobStage::Develop,
            JobStage::Encode,
            JobStage::Complete,
        ]
    );
}

#[test]
fn cancellation_after_decoder_prevents_all_later_work() {
    let decoder = FakeDecoder {
        photo: decoded(),
        cancel_after_decode: true,
        calls: Arc::default(),
        source_identity: source_identity(),
    };
    let encoder = FakeEncoder::default();
    let failure = DevelopJobRunner::built_in()
        .unwrap()
        .run(
            &job(),
            &decoder,
            &encoder,
            &CancellationToken::new(),
            &mut Stages::default(),
        )
        .unwrap_err();
    assert_eq!(failure.error.stage, JobStage::Decode);
    assert_eq!(failure.error.code, JobErrorCode::Cancelled);
    assert!(encoder.calls.lock().unwrap().is_empty());
}

#[test]
fn reports_are_versioned_and_do_not_serialize_paths() {
    let decoder = FakeDecoder {
        photo: decoded(),
        cancel_after_decode: false,
        calls: Arc::default(),
        source_identity: source_identity(),
    };
    let report = DevelopJobRunner::built_in()
        .unwrap()
        .run(
            &job(),
            &decoder,
            &FakeEncoder::default(),
            &CancellationToken::new(),
            &mut Stages::default(),
        )
        .unwrap();
    let json = serde_json::to_string(&report).unwrap();
    assert!(json.contains("io.omacom.grainroom.develop-job-report"));
    assert!(json.contains("\"schema_version\":2"));
    assert!(json.contains("\"profile\":\"pointwise_v1\""));
    assert!(json.contains("\"job_peak_bytes\":64"));
    assert!(!json.contains("/virtual/"));
    assert!(!json.contains("input.raw"));
    assert!(!json.contains("output.jpg"));
}

#[test]
fn invalid_preset_fails_after_decode_but_before_develop_or_encode() {
    let decoder = FakeDecoder {
        photo: decoded(),
        cancel_after_decode: false,
        calls: Arc::default(),
        source_identity: source_identity(),
    };
    let encoder = FakeEncoder::default();
    let mut request = job();
    request.preset = PresetSelection::CatalogId("missing".to_owned());
    let failure = DevelopJobRunner::built_in()
        .unwrap()
        .run(
            &request,
            &decoder,
            &encoder,
            &CancellationToken::new(),
            &mut Stages::default(),
        )
        .unwrap_err();
    assert_eq!(failure.error.stage, JobStage::ResolveSettings);
    assert_eq!(failure.error.code, JobErrorCode::InvalidOptions);
    assert!(encoder.calls.lock().unwrap().is_empty());
}

#[test]
fn pointwise_override_is_applied_to_the_encoded_artifact() {
    let decoder = FakeDecoder {
        photo: decoded(),
        cancel_after_decode: false,
        calls: Arc::default(),
        source_identity: source_identity(),
    };
    let encoder = FakeEncoder::default();
    let mut request = job();
    request
        .overrides
        .push(ParameterOverride::scalar("basics.brightness", 25.0));
    let report = DevelopJobRunner::built_in()
        .unwrap()
        .run(
            &request,
            &decoder,
            &encoder,
            &CancellationToken::new(),
            &mut Stages::default(),
        )
        .unwrap();
    assert_eq!(*encoder.calls.lock().unwrap(), ["encode"]);
    let expected = f64::from(0.18_f32) * 2.0_f64.powf(0.25);
    assert_eq!(encoder.observed.lock().unwrap()[0].1, expected as f32);
    assert_eq!(
        report.develop_working_set.unwrap().profile,
        grainroom::job::ReportDevelopWorkingSetProfile::PointwiseV1
    );
}

#[test]
fn pointwise_preset_is_applied_to_the_encoded_artifact() {
    let mut settings = DevelopSettings::default();
    settings.basics.brightness = 100.0;
    let estimate = estimate_develop_working_set(
        2,
        1,
        &settings,
        &ResourceLimits::default().with_max_working_bytes(64),
    )
    .unwrap();
    assert_eq!(estimate.profile, DevelopWorkingSetProfile::PointwiseV1);
    assert_eq!(estimate.peak_bytes, 64);

    let decoder = FakeDecoder {
        photo: decoded(),
        cancel_after_decode: false,
        calls: Arc::default(),
        source_identity: source_identity(),
    };
    let encoder = FakeEncoder::default();
    let mut request = job();
    request.preset = PresetSelection::document(PresetDocument::new(
        "pointwise-job-gate",
        "Pointwise job gate",
        settings,
    ));
    let report = DevelopJobRunner::built_in()
        .unwrap()
        .run(
            &request,
            &decoder,
            &encoder,
            &CancellationToken::new(),
            &mut Stages::default(),
        )
        .unwrap();
    assert_eq!(*encoder.calls.lock().unwrap(), ["encode"]);
    assert_eq!(encoder.observed.lock().unwrap()[0].1, 0.36_f32);
    assert_eq!(report.develop_working_set.unwrap().job_peak_bytes, 64);
}

#[test]
fn exact_pointwise_peak_succeeds_and_peak_minus_one_never_reaches_encoder() {
    let decoder = FakeDecoder {
        photo: decoded(),
        cancel_after_decode: false,
        calls: Arc::default(),
        source_identity: source_identity(),
    };
    let encoder = FakeEncoder::default();
    let mut exact = job();
    exact.decode.limits.max_working_bytes = 64;
    let report = DevelopJobRunner::built_in()
        .unwrap()
        .run(
            &exact,
            &decoder,
            &encoder,
            &CancellationToken::new(),
            &mut Stages::default(),
        )
        .unwrap();
    assert_eq!(report.develop_working_set.unwrap().job_peak_bytes, 64);
    assert_eq!(*encoder.calls.lock().unwrap(), ["encode"]);

    let rejected_encoder = FakeEncoder::default();
    let mut below = job();
    below.decode.limits.max_working_bytes = 63;
    let failure = DevelopJobRunner::built_in()
        .unwrap()
        .run(
            &below,
            &decoder,
            &rejected_encoder,
            &CancellationToken::new(),
            &mut Stages::default(),
        )
        .unwrap_err();
    assert_eq!(failure.error.stage, JobStage::ResolveSettings);
    assert_eq!(failure.error.code, JobErrorCode::ResourceLimit);
    assert_eq!(
        failure.report.develop_working_set.unwrap().job_peak_bytes,
        64
    );
    assert!(rejected_encoder.calls.lock().unwrap().is_empty());
}

#[test]
fn grain_seed_depends_on_content_not_renamed_paths() {
    let mut settings = DevelopSettings::default();
    settings.effects.grain.amount = 61.0;
    let preset = PresetDocument::new("grain-rename", "Grain rename", settings);
    let render = |input: &str, output: &str| {
        let decoder = FakeDecoder {
            photo: decoded(),
            cancel_after_decode: false,
            calls: Arc::default(),
            source_identity: source_identity(),
        };
        let encoder = FakeEncoder::default();
        let mut request = job();
        request.input = input.into();
        request.output = output.into();
        request.preset = PresetSelection::document(preset.clone());
        DevelopJobRunner::built_in()
            .unwrap()
            .run(
                &request,
                &decoder,
                &encoder,
                &CancellationToken::new(),
                &mut Stages::default(),
            )
            .unwrap();
        encoder.observed.lock().unwrap()[0].2.clone()
    };
    assert_eq!(
        render("/renamed/one.jpg", "/out/one.jpg"),
        render("/elsewhere/two.jpg", "/out/two.jpg")
    );
}

#[test]
fn every_unprofiled_stage_family_is_fail_closed_before_develop() {
    let mut clarity = DevelopSettings::default();
    clarity.basics.clarity = 1.0;
    let mut geometry = DevelopSettings::default();
    geometry.geometry.straighten_degrees = 1.0;
    let mut curves = DevelopSettings::default();
    curves.tone_curves.master.points[1].y = 0.75;
    let mut mixer = DevelopSettings::default();
    mixer.color_mixer.red.saturation = 1.0;
    let mut grading = DevelopSettings::default();
    grading.color_grading.midtones.saturation = 1.0;
    let mut radial = DevelopSettings::default();
    radial.radial_masks.masks.push(RadialMask {
        id: "resource-test".to_owned(),
        enabled: true,
        center_x: 0.5,
        center_y: 0.5,
        radius_x: 0.25,
        radius_y: 0.25,
        rotation_degrees: 0.0,
        feather: 0.5,
        opacity: 1.0,
        invert: false,
        adjustments: LocalAdjustments {
            brightness: 1.0,
            ..LocalAdjustments::default()
        },
    });

    let mut bloom = DevelopSettings::default();
    bloom.effects.bloom = 1.0;
    let mut halation = DevelopSettings::default();
    halation.effects.halation = 1.0;
    let mut sharpness = DevelopSettings::default();
    sharpness.effects.sharpness = 1.0;

    for (index, settings) in [
        clarity, geometry, curves, mixer, grading, radial, bloom, halation, sharpness,
    ]
    .into_iter()
    .enumerate()
    {
        let decoder = FakeDecoder {
            photo: decoded(),
            cancel_after_decode: false,
            calls: Arc::default(),
            source_identity: source_identity(),
        };
        let encoder = FakeEncoder::default();
        let mut request = job();
        request.preset = PresetSelection::document(PresetDocument::new(
            format!("resource-test-{index}"),
            "Resource test",
            settings,
        ));
        let mut stages = Stages::default();
        let failure = DevelopJobRunner::built_in()
            .unwrap()
            .run(
                &request,
                &decoder,
                &encoder,
                &CancellationToken::new(),
                &mut stages,
            )
            .unwrap_err();
        assert_eq!(failure.error.stage, JobStage::ResolveSettings);
        assert_eq!(failure.error.code, JobErrorCode::UnprovenPipelineBudget);
        assert_eq!(stages.0, [JobStage::Validate, JobStage::Decode]);
        assert!(encoder.calls.lock().unwrap().is_empty());
    }
}

#[test]
fn publication_commit_is_not_reinterpreted_as_late_cancellation() {
    let decoder = FakeDecoder {
        photo: decoded(),
        cancel_after_decode: false,
        calls: Arc::default(),
        source_identity: source_identity(),
    };
    let encoder = FakeEncoder {
        publication: PublicationStatus::PublishedButNotDurable,
        cancel_on_publish: true,
        ..FakeEncoder::default()
    };
    let report = DevelopJobRunner::built_in()
        .unwrap()
        .run(
            &job(),
            &decoder,
            &encoder,
            &CancellationToken::new(),
            &mut Stages::default(),
        )
        .unwrap();
    assert!(matches!(
        report.outcome,
        grainroom::job::DevelopJobOutcome::PublishedButNotDurable { bytes_written: 37 }
    ));
}
