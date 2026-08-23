use crate::{
    develop::{DevelopPipeline, DevelopSettings, PipelineError, PresetCatalog, PresetDocument},
    io::{
        LimitError,
        color::{SceneRenderError, SceneToDisplayTransform},
    },
};

use super::{
    CancellationToken, DecodedArtifact, DevelopJob, DevelopJobFailure, DevelopJobOutcome,
    DevelopJobReport, JobErrorCode, JobStage, PhotoDecoder, PhotoEncoder, PresetSelection,
    ProgressSink, PublicationRequest, PublicationStatus, ReportDigest, ReportSignalRelation,
    SceneRenderSummary, WorkingArtifact, services::stable_code,
};

#[derive(Clone, Debug)]
pub struct DevelopJobRunner {
    catalog: PresetCatalog,
    pipeline: DevelopPipeline,
    scene_transform: SceneToDisplayTransform,
}

impl DevelopJobRunner {
    pub fn new(catalog: PresetCatalog) -> Self {
        Self {
            catalog,
            pipeline: DevelopPipeline,
            scene_transform: SceneToDisplayTransform::new(),
        }
    }

    pub fn built_in() -> Result<Self, crate::develop::PresetCatalogError> {
        PresetCatalog::built_in().map(Self::new)
    }

    pub fn run<D, E, P>(
        &self,
        job: &DevelopJob,
        decoder: &D,
        encoder: &E,
        cancellation: &CancellationToken,
        progress: &mut P,
    ) -> Result<DevelopJobReport, DevelopJobFailure>
    where
        D: PhotoDecoder,
        E: PhotoEncoder,
        P: ProgressSink,
    {
        let mut report = DevelopJobReport::pending();
        self.check_cancelled(JobStage::Validate, cancellation, &report)?;
        if job.input.as_os_str().is_empty()
            || job.output.as_os_str().is_empty()
            || job.decode.validate().is_err()
            || job.output_options.validate().is_err()
        {
            return Err(DevelopJobFailure::new(
                JobStage::Validate,
                JobErrorCode::InvalidOptions,
                report,
            ));
        }
        progress.stage_completed(JobStage::Validate);

        self.check_cancelled(JobStage::Decode, cancellation, &report)?;
        let decoded = decoder
            .decode_path_once(&job.input, &job.decode, cancellation)
            .map_err(|error| {
                DevelopJobFailure::new(JobStage::Decode, stable_code(&error).into(), report.clone())
            })?;
        self.check_cancelled(JobStage::Decode, cancellation, &report)?;
        let source_identity = decoded.source_identity;
        let artifact = DecodedArtifact::try_from_photo(decoded.photo, &job.decode.limits).map_err(
            |error| {
                let code = match error {
                    crate::io::DecodedPhotoError::Limit(_) => JobErrorCode::ResourceLimit,
                    crate::io::DecodedPhotoError::Image(_)
                    | crate::io::DecodedPhotoError::ColorRelationMismatch => {
                        JobErrorCode::ColorManagement
                    }
                };
                DevelopJobFailure::new(JobStage::Decode, code, report.clone())
            },
        )?;
        let digest = match &artifact {
            DecodedArtifact::Scene(value) => value.source_digest(),
            DecodedArtifact::Display(value) => value.source_digest(),
        };
        let input_relation = match &artifact {
            DecodedArtifact::Scene(value) => value.signal_relation(),
            DecodedArtifact::Display(value) => value.signal_relation(),
        };
        report.source_digest_v1 = Some(ReportDigest(*digest.as_bytes()));
        report.input_signal_relation = Some(input_relation.into());
        progress.stage_completed(JobStage::Decode);

        self.check_cancelled(JobStage::ResolveSettings, cancellation, &report)?;
        let document = self.resolve_preset(&job.preset).map_err(|code| {
            DevelopJobFailure::new(JobStage::ResolveSettings, code, report.clone())
        })?;
        if !job.overrides.is_empty() {
            return Err(DevelopJobFailure::new(
                JobStage::ResolveSettings,
                JobErrorCode::UnprovenPipelineBudget,
                report,
            ));
        }
        let settings = &document.settings;
        self.pipeline
            .preflight_with_context(settings, Some(&digest.develop_render_context()))
            .map_err(|error| {
                DevelopJobFailure::new(
                    JobStage::ResolveSettings,
                    pipeline_error_code(&error),
                    report.clone(),
                )
            })?;
        // Current develop stages contain legacy infallible allocations whose
        // combined peak is not yet covered by a stage-aware estimator. The
        // job boundary therefore admits only the allocation-free neutral path
        // instead of claiming an optimistic full-image budget.
        if !settings.is_neutral() {
            return Err(DevelopJobFailure::new(
                JobStage::ResolveSettings,
                JobErrorCode::UnprovenPipelineBudget,
                report,
            ));
        }
        progress.stage_completed(JobStage::ResolveSettings);

        let display = match artifact {
            DecodedArtifact::Scene(mut scene) => {
                self.process_artifact(
                    &mut scene,
                    settings,
                    digest,
                    cancellation,
                    progress,
                    &report,
                )?;
                self.check_cancelled(JobStage::SceneRender, cancellation, &report)?;
                let (display, scene_report) = scene
                    .render_to_display(&self.scene_transform, &job.decode.limits)
                    .map_err(|error| {
                        DevelopJobFailure::new(
                            JobStage::SceneRender,
                            scene_error_code(&error),
                            report.clone(),
                        )
                    })?;
                self.check_cancelled(JobStage::SceneRender, cancellation, &report)?;
                report.scene_render = Some(SceneRenderSummary::from(scene_report));
                progress.stage_completed(JobStage::SceneRender);
                display
            }
            DecodedArtifact::Display(mut display) => {
                self.process_artifact(
                    &mut display,
                    settings,
                    digest,
                    cancellation,
                    progress,
                    &report,
                )?;
                display
            }
        };

        report.output_signal_relation = Some(ReportSignalRelation::LinearizedDisplayReferred);
        self.check_cancelled(JobStage::Encode, cancellation, &report)?;
        let encode_options = job.output_options.as_encode_options();
        let receipt = encoder
            .encode_display(
                PublicationRequest {
                    destination: &job.output,
                    source_identity,
                    overwrite: job.overwrite,
                },
                &display,
                &encode_options,
                cancellation,
            )
            .map_err(|error| {
                DevelopJobFailure::new(JobStage::Encode, stable_code(&error).into(), report.clone())
            })?;
        // Encoder success is the publication commit point. Do not reinterpret
        // a visible destination as a retryable cancellation afterwards.
        progress.stage_completed(JobStage::Encode);
        progress.stage_completed(JobStage::Complete);
        report.outcome = match receipt.publication {
            PublicationStatus::PublishedAndDurable => DevelopJobOutcome::PublishedAndDurable {
                bytes_written: receipt.bytes_written,
            },
            PublicationStatus::PublishedButNotDurable => {
                DevelopJobOutcome::PublishedButNotDurable {
                    bytes_written: receipt.bytes_written,
                }
            }
        };
        Ok(report)
    }

    fn process_artifact<R: super::artifact::ArtifactRelation>(
        &self,
        artifact: &mut WorkingArtifact<R>,
        settings: &DevelopSettings,
        digest: crate::io::SourceDigestV1,
        cancellation: &CancellationToken,
        progress: &mut impl ProgressSink,
        report: &DevelopJobReport,
    ) -> Result<(), DevelopJobFailure> {
        self.check_cancelled(JobStage::Develop, cancellation, report)?;
        debug_assert!(settings.is_neutral());
        let _ = (artifact, digest);
        self.check_cancelled(JobStage::Develop, cancellation, report)?;
        progress.stage_completed(JobStage::Develop);
        Ok(())
    }

    fn resolve_preset<'a>(
        &'a self,
        selection: &'a PresetSelection,
    ) -> Result<&'a PresetDocument, JobErrorCode> {
        let document = match selection {
            PresetSelection::CatalogId(id) => {
                self.catalog.get(id).ok_or(JobErrorCode::InvalidOptions)?
            }
            PresetSelection::Document(document) => document.as_ref(),
        };
        document
            .validate()
            .map_err(|_| JobErrorCode::InvalidOptions)?;
        Ok(document)
    }

    fn check_cancelled(
        &self,
        stage: JobStage,
        cancellation: &CancellationToken,
        report: &DevelopJobReport,
    ) -> Result<(), DevelopJobFailure> {
        if cancellation.is_cancelled() {
            Err(DevelopJobFailure::new(
                stage,
                JobErrorCode::Cancelled,
                report.clone(),
            ))
        } else {
            Ok(())
        }
    }
}

fn pipeline_error_code(error: &PipelineError) -> JobErrorCode {
    match error {
        PipelineError::InvalidSettings(_)
        | PipelineError::StageNotImplemented(_)
        | PipelineError::MissingRenderContext(_) => JobErrorCode::InvalidOptions,
        PipelineError::InvalidImage(_) | PipelineError::NumericFailure { .. } => {
            JobErrorCode::Internal
        }
    }
}

fn scene_error_code(error: &SceneRenderError) -> JobErrorCode {
    match error {
        SceneRenderError::Limit(LimitError::Allocation)
        | SceneRenderError::Limit(LimitError::ArithmeticOverflow)
        | SceneRenderError::Limit(LimitError::DecodedBytes { .. })
        | SceneRenderError::Limit(LimitError::EmptyDimensions)
        | SceneRenderError::Limit(LimitError::InvalidConfiguration)
        | SceneRenderError::Limit(LimitError::MetadataBytes { .. })
        | SceneRenderError::Limit(LimitError::PixelCount { .. })
        | SceneRenderError::Limit(LimitError::SourceBytes { .. })
        | SceneRenderError::Limit(LimitError::WorkingBytes { .. })
        | SceneRenderError::Allocation => JobErrorCode::ResourceLimit,
        SceneRenderError::InvalidSignalRelation { .. }
        | SceneRenderError::LengthMismatch { .. }
        | SceneRenderError::NonFiniteOutput { .. } => JobErrorCode::ColorManagement,
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use crate::{
        develop::{CpuImage, RgbaPixel},
        io::{
            AlphaPolicy, ColorProvenance, DecodeError, DecodeOptions, DecodedPhoto, Diagnostic,
            EncodeError, EncodeOptions, MetadataBundle, MetadataPolicy, OutputFormat,
            OutputProfile, OverwritePolicy, RawBackendName, RawMatrixSource,
            RawProcessingProvenance, ResourceLimits, SdrRangePolicy, SignalRelation,
            SourceDigestV1, SourceFileIdentity, WhiteBalanceProvenance,
        },
        job::{
            CancellationToken, DecodedSource, DevelopJob, DevelopJobRunner, DevelopOutput,
            DisplayReferred, EncodeReceipt, NoProgress, PhotoDecoder, PhotoEncoder,
            PresetSelection, PublicationRequest, PublicationStatus, WorkingArtifact,
        },
    };

    struct RawDecoder;
    impl PhotoDecoder for RawDecoder {
        type Error = DecodeError;

        fn decode_path_once(
            &self,
            _input: &Path,
            _options: &DecodeOptions,
            _cancellation: &CancellationToken,
        ) -> Result<DecodedSource, Self::Error> {
            let image =
                CpuImage::new(1, 1, vec![RgbaPixel::new(0.18, 0.18, 0.18, 0.37).unwrap()]).unwrap();
            let photo = DecodedPhoto::new(
                image,
                MetadataBundle::default(),
                SourceDigestV1::from_bytes(b"raw fake"),
                ColorProvenance::RawMatrix {
                    matrix: RawMatrixSource::CameraDatabase,
                    white_balance: WhiteBalanceProvenance::Camera,
                    processing: RawProcessingProvenance {
                        backend: RawBackendName::LibRawDcrawEmu,
                        backend_version: Some("test".to_owned()),
                        full_resolution: true,
                        linear_16_bit: true,
                        output_rec2020: true,
                        embedded_matrix_enabled: true,
                        ahd_demosaic: true,
                    },
                },
                SignalRelation::SceneRelatedRaw,
                Vec::<Diagnostic>::new(),
                &ResourceLimits::default(),
            )
            .unwrap();
            Ok(DecodedSource {
                photo,
                source_identity: SourceFileIdentity::from_file(&tempfile::tempfile().unwrap())
                    .unwrap(),
            })
        }
    }

    struct RelationEncoder;
    impl PhotoEncoder for RelationEncoder {
        type Error = EncodeError;

        fn encode_display(
            &self,
            _publication: PublicationRequest<'_>,
            artifact: &WorkingArtifact<DisplayReferred>,
            _options: &EncodeOptions,
            _cancellation: &CancellationToken,
        ) -> Result<EncodeReceipt, Self::Error> {
            assert_eq!(
                artifact.signal_relation(),
                SignalRelation::LinearizedDisplayReferred
            );
            assert_eq!(
                artifact.image().pixels()[0].alpha().to_bits(),
                0.37_f32.to_bits()
            );
            Ok(EncodeReceipt {
                bytes_written: 1,
                publication: PublicationStatus::PublishedAndDurable,
            })
        }
    }

    #[test]
    fn raw_is_scene_rendered_before_the_display_only_encoder() {
        let job = DevelopJob {
            input: "input.raw".into(),
            output: "output.jpg".into(),
            decode: DecodeOptions::default(),
            output_options: DevelopOutput::new(
                OutputFormat::Jpeg,
                90,
                OutputProfile::Srgb,
                MetadataPolicy::StripAll,
                AlphaPolicy::Flatten([0.0, 0.0, 0.0]),
                SdrRangePolicy::Reject,
            ),
            overwrite: OverwritePolicy::Forbid,
            preset: PresetSelection::CatalogId("neutral".to_owned()),
            overrides: Vec::new(),
        };
        let report = DevelopJobRunner::built_in()
            .unwrap()
            .run(
                &job,
                &RawDecoder,
                &RelationEncoder,
                &CancellationToken::new(),
                &mut NoProgress,
            )
            .unwrap();
        assert!(report.scene_render.is_some());
        assert_eq!(
            report.output_signal_relation,
            Some(super::ReportSignalRelation::LinearizedDisplayReferred)
        );
    }
}
