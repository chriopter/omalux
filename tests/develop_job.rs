use std::{
    path::Path,
    sync::{Arc, Mutex},
};

use omalux::{
    develop::{
        CpuImage, CurvePoint, DevelopSettings, DevelopWorkingSetProfile, LocalAdjustments,
        ParameterOverride, PresetDocument, RadialMask, RgbaPixel, estimate_develop_working_set,
    },
    io::{
        AlphaPolicy, AssumedProfileReason, ColorProvenance, DecodeError, DecodeOptions,
        DecodedPhoto, Diagnostic, EncodeError, EncodeOptions, MetadataBundle, MetadataPolicy,
        OutputFormat, OutputProfile, OverwritePolicy, ResourceLimits, SdrRangePolicy,
        SignalRelation, SourceDigestV1,
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
        DecodedSource::from_held_file(self.photo.clone(), tempfile::tempfile().unwrap())
            .map_err(|_| DecodeError::InvalidOptions)
    }
}

#[derive(Clone)]
struct FakeEncoder {
    calls: Arc<Mutex<Vec<&'static str>>>,
    observed: Arc<Mutex<Vec<ObservedArtifact>>>,
    publication: PublicationStatus,
    cancel_on_publish: bool,
}

type ObservedArtifact = (SignalRelation, f32, Vec<[u32; 4]>);

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
        artifact: &WorkingArtifact<omalux::job::DisplayReferred>,
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
            summary: None,
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
    assert!(json.contains("io.omacom.omalux.develop-job-report"));
    assert!(json.contains("\"schema_version\":4"));
    assert!(json.contains("\"pointwise_v1\":true"));
    assert!(json.contains("\"estimated_peak_bytes\":64"));
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
    };
    let encoder = FakeEncoder::default();
    let mut request = job();
    request
        .overrides
        .push(ParameterOverride::scalar("basics.exposure_ev", 0.25));
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
        report.develop_working_set.profile(),
        Some(omalux::job::ReportDevelopWorkingSetProfile::PointwiseV1)
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
    assert_eq!(report.develop_working_set.estimated_peak_bytes(), 64);
}

#[test]
fn structured_curve_preset_and_scalar_color_overrides_run_as_color_v1() {
    let mut settings = DevelopSettings::default();
    settings.tone_curves.master.points = vec![
        CurvePoint { x: 0.0, y: 0.0 },
        CurvePoint { x: 0.5, y: 0.7 },
        CurvePoint { x: 1.0, y: 1.0 },
    ];
    let decoder = FakeDecoder {
        photo: decoded(),
        cancel_after_decode: false,
        calls: Arc::default(),
    };
    let encoder = FakeEncoder::default();
    let mut request = job();
    request.preset = PresetSelection::document(PresetDocument::new(
        "color-v1-job",
        "Color V1 job",
        settings,
    ));
    request.overrides = vec![
        ParameterOverride::scalar("basics.contrast", 8.0),
        ParameterOverride::scalar("color_mixer.red.saturation", 20.0),
        ParameterOverride::scalar("color_grading.midtones.hue_degrees", 210.0),
        ParameterOverride::scalar("color_grading.midtones.saturation", 15.0),
    ];
    request.decode.limits.max_working_bytes = 176;

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
    assert_eq!(
        report.develop_working_set.profile(),
        Some(omalux::job::ReportDevelopWorkingSetProfile::ColorV1)
    );
    assert_eq!(report.develop_working_set.estimated_peak_bytes(), 176);
    assert_ne!(encoder.observed.lock().unwrap()[0].1, 0.18);

    let rejected_encoder = FakeEncoder::default();
    request.decode.limits.max_working_bytes = 175;
    let failure = DevelopJobRunner::built_in()
        .unwrap()
        .run(
            &request,
            &decoder,
            &rejected_encoder,
            &CancellationToken::new(),
            &mut Stages::default(),
        )
        .unwrap_err();
    assert_eq!(failure.error.stage, JobStage::ResolveSettings);
    assert_eq!(failure.error.code, JobErrorCode::ResourceLimit);
    assert_eq!(
        failure.report.develop_working_set.estimated_peak_bytes(),
        176
    );
    assert!(rejected_encoder.calls.lock().unwrap().is_empty());
}

#[test]
fn exact_pointwise_peak_succeeds_and_peak_minus_one_never_reaches_encoder() {
    let decoder = FakeDecoder {
        photo: decoded(),
        cancel_after_decode: false,
        calls: Arc::default(),
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
    assert_eq!(report.develop_working_set.estimated_peak_bytes(), 64);
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
        failure.report.develop_working_set.estimated_peak_bytes(),
        64
    );
    assert!(rejected_encoder.calls.lock().unwrap().is_empty());
}

#[test]
fn color_spatial_job_reports_json_profile_and_rejects_peak_minus_one_before_develop() {
    let mut settings = DevelopSettings::default();
    settings.basics.clarity = 35.0;
    settings.tone_curves.master.points[1].y = 0.8;
    let preset = PresetDocument::new("color-spatial-job-gate", "Color spatial job gate", settings);

    let decoder = FakeDecoder {
        photo: decoded(),
        cancel_after_decode: false,
        calls: Arc::default(),
    };
    let encoder = FakeEncoder::default();
    let mut exact = job();
    exact.preset = PresetSelection::document(preset.clone());
    exact.decode.limits.max_working_bytes = 632;
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
    assert_eq!(report.develop_working_set.estimated_peak_bytes(), 632);
    assert_eq!(
        report.develop_working_set.profile(),
        Some(omalux::job::ReportDevelopWorkingSetProfile::ColorSpatialV1)
    );
    let json = serde_json::to_string(&report).unwrap();
    assert!(json.contains("\"color_v1\":true"));
    assert!(json.contains("\"spatial_v1\":true"));
    assert!(json.contains("\"estimated_peak_bytes\":632"));

    let rejected_encoder = FakeEncoder::default();
    let mut below = job();
    below.preset = PresetSelection::document(preset);
    below.decode.limits.max_working_bytes = 631;
    let mut stages = Stages::default();
    let failure = DevelopJobRunner::built_in()
        .unwrap()
        .run(
            &below,
            &decoder,
            &rejected_encoder,
            &CancellationToken::new(),
            &mut stages,
        )
        .unwrap_err();
    assert_eq!(failure.error.stage, JobStage::ResolveSettings);
    assert_eq!(failure.error.code, JobErrorCode::ResourceLimit);
    assert_eq!(
        failure.report.develop_working_set.estimated_peak_bytes(),
        632
    );
    assert_eq!(stages.0, [JobStage::Validate, JobStage::Decode]);
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
fn negative_local_sharpness_is_fail_closed_before_develop() {
    let mut settings = DevelopSettings::default();
    settings.radial_masks.masks.push(RadialMask {
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
            sharpness: -1.0,
            ..LocalAdjustments::default()
        },
    });
    let decoder = FakeDecoder {
        photo: decoded(),
        cancel_after_decode: false,
        calls: Arc::default(),
    };
    let encoder = FakeEncoder::default();
    let mut request = job();
    request.preset = PresetSelection::document(PresetDocument::new(
        "negative-local-sharpness",
        "Negative local sharpness",
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

#[test]
fn publication_commit_is_not_reinterpreted_as_late_cancellation() {
    let decoder = FakeDecoder {
        photo: decoded(),
        cancel_after_decode: false,
        calls: Arc::default(),
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
        omalux::job::DevelopJobOutcome::PublishedButNotDurable { bytes_written: 37 }
    ));
}
