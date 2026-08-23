use std::{
    path::Path,
    sync::{Arc, Mutex},
};

use grainroom::{
    develop::{
        CpuImage, DevelopSettings, LocalAdjustments, ParameterOverride, PresetDocument, RadialMask,
        RgbaPixel,
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
    observed: Arc<Mutex<Vec<(SignalRelation, f32)>>>,
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
        self.observed.lock().unwrap().push((
            artifact.signal_relation(),
            artifact.image().pixels()[0].red(),
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
fn non_neutral_override_is_rejected_until_pipeline_budget_is_proven() {
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
    assert_eq!(failure.error.code, JobErrorCode::UnprovenPipelineBudget);
    assert!(encoder.calls.lock().unwrap().is_empty());
}

#[test]
fn every_allocating_stage_family_is_fail_closed_without_an_estimator() {
    let mut clarity = DevelopSettings::default();
    clarity.basics.clarity = 1.0;
    let mut effects = DevelopSettings::default();
    effects.effects.bloom = 1.0;
    let mut geometry = DevelopSettings::default();
    geometry.geometry.straighten_degrees = 1.0;
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

    for (index, settings) in [clarity, effects, geometry, radial].into_iter().enumerate() {
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
        assert_eq!(failure.error.code, JobErrorCode::UnprovenPipelineBudget);
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
