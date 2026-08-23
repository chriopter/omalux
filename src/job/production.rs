//! Production Qt-free decoder and publication services.

use std::{fs::File, io, os::unix::fs::FileExt, path::Path};

use rustix::fs::{self, FileType, Mode, OFlags};

use crate::{
    io::{
        AtomicOutputOptions, AtomicOutputOutcome, DecodeError, DecodeOptions, EncodeCancellation,
        EncodeError, EncodeOptions, HeicEncodeRequest, JpegEncodeInput, JpegEncodeRequest,
        OutputFormat, ResourceLimits,
        raster::{RasterCancellation, decode_raster_file},
        raw::{RawCancellation, RawExecutionOptions, decode_raw_file, trusted_dcraw_execution},
    },
    job::{
        CancellationToken, DecodedSource, DisplayReferred, EncodeReceipt, EncodeSummary,
        HeicNclxSummary, PhotoDecoder, PhotoEncoder, PublicationRequest, PublicationStatus,
        ReportDigest, WorkingArtifact,
    },
};

#[derive(Clone, Debug)]
pub struct ProductionPhotoDecoder {
    raw: Option<RawExecutionOptions>,
}

impl ProductionPhotoDecoder {
    /// Creates a decoder with an optional, fixed system RAW backend. Raster
    /// decoding remains available when the backend is not installed or fails
    /// the trusted-file policy.
    pub fn new() -> Self {
        Self {
            raw: trusted_dcraw_execution().ok(),
        }
    }

    #[cfg(test)]
    fn with_raw(raw: RawExecutionOptions) -> Self {
        Self { raw: Some(raw) }
    }
}

impl Default for ProductionPhotoDecoder {
    fn default() -> Self {
        Self::new()
    }
}

impl PhotoDecoder for ProductionPhotoDecoder {
    type Error = DecodeError;

    fn decode_path_once(
        &self,
        input: &Path,
        options: &DecodeOptions,
        cancellation: &CancellationToken,
    ) -> Result<DecodedSource, Self::Error> {
        options.validate()?;
        if cancellation.is_cancelled() {
            return Err(DecodeError::Cancelled);
        }
        let fd = fs::open(
            input,
            OFlags::RDONLY | OFlags::NONBLOCK | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(map_source_open)?;
        self.decode_opened(File::from(fd), input, options, cancellation)
    }
}

impl ProductionPhotoDecoder {
    fn decode_opened(
        &self,
        file: File,
        source_name: &Path,
        options: &DecodeOptions,
        cancellation: &CancellationToken,
    ) -> Result<DecodedSource, DecodeError> {
        let source_lease = file.try_clone().map_err(DecodeError::Input)?;
        let stat = fs::fstat(&file).map_err(|error| {
            DecodeError::Input(io::Error::from_raw_os_error(error.raw_os_error()))
        })?;
        if FileType::from_raw_mode(stat.st_mode) != FileType::RegularFile {
            return Err(DecodeError::UnsupportedFormat);
        }
        let advertised = u64::try_from(stat.st_size).map_err(|_| DecodeError::UnsupportedFormat)?;
        options
            .limits
            .check_source_bytes(advertised)
            .map_err(DecodeError::Limit)?;
        let mut signature = [0_u8; 8];
        let read = file
            .read_at(&mut signature, 0)
            .map_err(DecodeError::Input)?;

        let raster = RasterCancellation::from_flag(cancellation.shared_flag());
        let raw = RawCancellation::from_flag(cancellation.shared_flag());
        let result = if raster_signature(&signature[..read]) {
            decode_raster_file(file, options, &raster)
        } else {
            let execution = self
                .raw
                .as_ref()
                .ok_or(DecodeError::RawBackendUnavailable)?;
            decode_raw_file(file, source_name, options, execution, &raw)
        }?;
        let decoded = DecodedSource::from_held_file(result.0, source_lease)
            .map_err(|error| DecodeError::Input(io::Error::other(error)))?;
        if decoded.source_identity() != result.1 {
            return Err(DecodeError::Input(io::Error::other(
                "source identity changed across the held descriptor",
            )));
        }
        Ok(decoded)
    }
}

fn raster_signature(bytes: &[u8]) -> bool {
    bytes.starts_with(&[0xff, 0xd8, 0xff])
        || bytes.starts_with(b"\x89PNG\r\n\x1a\n")
        || bytes.starts_with(b"BM")
}

fn map_source_open(error: rustix::io::Errno) -> DecodeError {
    if error == rustix::io::Errno::LOOP {
        DecodeError::UnsupportedFormat
    } else {
        DecodeError::Input(io::Error::from_raw_os_error(error.raw_os_error()))
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ProductionJpegEncoder {
    limits: ResourceLimits,
}

/// Codec-dispatching production encoder used by the CLI and GUI adapters.
/// The display artifact is borrowed directly by the selected backend.
#[derive(Clone, Copy, Debug)]
pub struct ProductionPhotoEncoder {
    limits: ResourceLimits,
}

impl ProductionPhotoEncoder {
    pub const fn new(limits: ResourceLimits) -> Self {
        Self { limits }
    }
}

impl PhotoEncoder for ProductionPhotoEncoder {
    type Error = EncodeError;

    fn encode_display(
        &self,
        publication: PublicationRequest<'_>,
        artifact: &WorkingArtifact<DisplayReferred>,
        options: &EncodeOptions,
        cancellation: &CancellationToken,
    ) -> Result<EncodeReceipt, Self::Error> {
        match options.format {
            OutputFormat::Jpeg => {
                encode_jpeg_display(self.limits, publication, artifact, options, cancellation)
            }
            OutputFormat::Heic => {
                encode_heic_display(self.limits, publication, artifact, options, cancellation)
            }
        }
    }
}

impl ProductionJpegEncoder {
    pub const fn new(limits: ResourceLimits) -> Self {
        Self { limits }
    }
}

impl PhotoEncoder for ProductionJpegEncoder {
    type Error = EncodeError;

    fn encode_display(
        &self,
        publication: PublicationRequest<'_>,
        artifact: &WorkingArtifact<DisplayReferred>,
        options: &EncodeOptions,
        cancellation: &CancellationToken,
    ) -> Result<EncodeReceipt, Self::Error> {
        encode_jpeg_display(self.limits, publication, artifact, options, cancellation)
    }
}

fn encode_jpeg_display(
    limits: ResourceLimits,
    publication: PublicationRequest<'_>,
    artifact: &WorkingArtifact<DisplayReferred>,
    options: &EncodeOptions,
    cancellation: &CancellationToken,
) -> Result<EncodeReceipt, EncodeError> {
    if options.format != OutputFormat::Jpeg {
        return Err(EncodeError::UnsupportedFormat);
    }
    let encode_cancellation = EncodeCancellation::from_flag(cancellation.shared_flag());
    let report = crate::io::encode_jpeg(JpegEncodeRequest {
        input: JpegEncodeInput {
            image: artifact.image(),
            signal_relation: artifact.signal_relation(),
            metadata: artifact.metadata(),
        },
        destination: publication.destination,
        source_identity: Some(publication.source.identity()),
        encode: *options,
        atomic: AtomicOutputOptions::default().with_overwrite(publication.overwrite),
        limits: &limits,
        cancellation: &encode_cancellation,
    })?;
    let publication = match report.outcome {
        AtomicOutputOutcome::PublishedAndDurable => PublicationStatus::PublishedAndDurable,
        AtomicOutputOutcome::PublishedButNotDurable => PublicationStatus::PublishedButNotDurable,
    };
    Ok(EncodeReceipt {
        bytes_written: report.output_bytes,
        publication,
        summary: Some(EncodeSummary::Jpeg {
            quality: report.quality,
            icc_sha256: ReportDigest(report.icc.sha256),
            clipped_samples: report.clipped_samples,
            alpha_flattened_pixels: report.alpha_flattened_pixels,
        }),
    })
}

fn encode_heic_display(
    limits: ResourceLimits,
    publication: PublicationRequest<'_>,
    artifact: &WorkingArtifact<DisplayReferred>,
    options: &EncodeOptions,
    cancellation: &CancellationToken,
) -> Result<EncodeReceipt, EncodeError> {
    if options.format != OutputFormat::Heic {
        return Err(EncodeError::UnsupportedFormat);
    }
    let encode_cancellation = EncodeCancellation::from_flag(cancellation.shared_flag());
    let report = crate::io::encode_heic(HeicEncodeRequest {
        input: JpegEncodeInput {
            image: artifact.image(),
            signal_relation: artifact.signal_relation(),
            metadata: artifact.metadata(),
        },
        destination: publication.destination,
        source_identity: Some(publication.source.identity()),
        encode: *options,
        atomic: AtomicOutputOptions::default().with_overwrite(publication.overwrite),
        limits: &limits,
        cancellation: &encode_cancellation,
    })?;
    let publication = publication_status(report.outcome);
    Ok(EncodeReceipt {
        bytes_written: report.output_bytes,
        publication,
        summary: Some(EncodeSummary::Heic {
            quality: report.quality,
            bit_depth: report.bit_depth,
            libheif_version: report.libheif_version,
            encoder: report.encoder,
            icc_sha256: ReportDigest(report.icc.sha256),
            nclx: HeicNclxSummary {
                color_primaries: report.nclx.color_primaries,
                transfer_characteristics: report.nclx.transfer_characteristics,
                matrix_coefficients: report.nclx.matrix_coefficients,
                full_range: report.nclx.full_range,
            },
            clipped_samples: report.clipped_samples,
            alpha_flattened_pixels: report.alpha_flattened_pixels,
        }),
    })
}

fn publication_status(outcome: AtomicOutputOutcome) -> PublicationStatus {
    match outcome {
        AtomicOutputOutcome::PublishedAndDurable => PublicationStatus::PublishedAndDurable,
        AtomicOutputOutcome::PublishedButNotDurable => PublicationStatus::PublishedButNotDurable,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs as stdfs, os::unix::fs::PermissionsExt, time::Duration};
    use tempfile::tempdir;

    fn jpeg(path: &Path, rgb: [u8; 3]) {
        image::save_buffer_with_format(
            path,
            &rgb,
            1,
            1,
            image::ColorType::Rgb8,
            image::ImageFormat::Jpeg,
        )
        .unwrap();
    }

    fn jpeg_solid(path: &Path, width: u32, height: u32, rgb: [u8; 3]) {
        let pixels = (0..width * height).flat_map(|_| rgb).collect::<Vec<_>>();
        image::save_buffer_with_format(
            path,
            &pixels,
            width,
            height,
            image::ColorType::Rgb8,
            image::ImageFormat::Jpeg,
        )
        .unwrap();
    }

    fn geometry_mask_settings() -> crate::develop::DevelopSettings {
        use crate::develop::{LocalAdjustments, RadialMask};
        let mut settings = crate::develop::DevelopSettings::default();
        settings.geometry.quarter_turns_clockwise = 1;
        settings.geometry.straighten_degrees = 2.0;
        settings.radial_masks.masks.push(RadialMask {
            id: "production-budget-mask".to_owned(),
            enabled: true,
            center_x: 0.5,
            center_y: 0.5,
            radius_x: 0.25,
            radius_y: 0.25,
            rotation_degrees: 0.0,
            feather: 0.5,
            opacity: 1.0,
            invert: true,
            adjustments: LocalAdjustments {
                exposure_ev: 0.75,
                brightness: 5.0,
                sharpness: 10.0,
                ..LocalAdjustments::default()
            },
        });
        settings
    }

    fn full_component_settings() -> crate::develop::DevelopSettings {
        use crate::develop::CurvePoint;

        let mut settings = geometry_mask_settings();
        settings.basics.exposure_ev = 0.5;
        settings.tone_curves.master.points = vec![
            CurvePoint { x: 0.0, y: 0.0 },
            CurvePoint { x: 0.25, y: 0.18 },
            CurvePoint { x: 0.65, y: 0.78 },
            CurvePoint { x: 1.0, y: 1.0 },
        ];
        settings.color_mixer.red.saturation = 15.0;
        settings.basics.clarity = 15.0;
        settings.effects.bloom = 10.0;
        settings
    }

    fn geometry_mask_job(
        input: &Path,
        output: &Path,
        format: OutputFormat,
    ) -> crate::job::DevelopJob {
        use crate::{
            develop::PresetDocument,
            io::{AlphaPolicy, MetadataPolicy, OutputProfile, OverwritePolicy, SdrRangePolicy},
            job::{DevelopJob, DevelopOutput, PresetSelection},
        };
        DevelopJob {
            input: input.to_owned(),
            output: output.to_owned(),
            decode: DecodeOptions::default(),
            output_options: DevelopOutput::new(
                format,
                90,
                OutputProfile::Srgb,
                MetadataPolicy::StripLocation,
                AlphaPolicy::Flatten([0.0, 0.0, 0.0]),
                SdrRangePolicy::ClipAndReport,
            ),
            overwrite: OverwritePolicy::Forbid,
            preset: PresetSelection::document(PresetDocument::new(
                "geometry-mask-job",
                "Geometry mask job",
                geometry_mask_settings(),
            )),
            overrides: Vec::new(),
        }
    }

    fn full_component_job(
        input: &Path,
        output: &Path,
        format: OutputFormat,
    ) -> crate::job::DevelopJob {
        let mut job = geometry_mask_job(input, output, format);
        job.preset = crate::job::PresetSelection::document(crate::develop::PresetDocument::new(
            "full-component-job",
            "Full component job",
            full_component_settings(),
        ));
        job
    }

    fn assert_full_component_profile(report: &crate::job::DevelopJobReport) {
        let profile = report.develop_working_set.profile().unwrap();
        assert!(profile.pointwise_v1);
        assert!(profile.color_v1);
        assert!(profile.spatial_v1);
        assert!(profile.geometry_v1);
        assert!(profile.radial_masks_v1);
    }

    #[test]
    fn raster_jpeg_runs_real_geometry_radial_color_and_spatial_stack() {
        use crate::{
            develop::PresetCatalog,
            job::{DevelopJobRunner, NoProgress},
        };
        let directory = tempdir().unwrap();
        let input = directory.path().join("source.jpg");
        let output = directory.path().join("full-stack.jpg");
        jpeg_solid(&input, 4, 3, [30, 60, 90]);
        let report = DevelopJobRunner::new(PresetCatalog::built_in().unwrap())
            .run(
                &full_component_job(&input, &output, OutputFormat::Jpeg),
                &ProductionPhotoDecoder::new(),
                &ProductionPhotoEncoder::new(ResourceLimits::default()),
                &CancellationToken::new(),
                &mut NoProgress,
            )
            .unwrap();
        assert_full_component_profile(&report);
        let decoded = image::open(output).unwrap();
        assert_eq!((decoded.width(), decoded.height()), (3, 4));
    }

    #[cfg(feature = "heic")]
    #[test]
    fn raster_heic_runs_real_geometry_radial_color_and_spatial_stack() {
        use crate::{
            develop::PresetCatalog,
            job::{DevelopJobRunner, NoProgress},
        };
        let directory = tempdir().unwrap();
        let input = directory.path().join("source.jpg");
        let output = directory.path().join("full-stack.heic");
        jpeg_solid(&input, 4, 3, [30, 60, 90]);
        let report = DevelopJobRunner::new(PresetCatalog::built_in().unwrap())
            .run(
                &full_component_job(&input, &output, OutputFormat::Heic),
                &ProductionPhotoDecoder::new(),
                &ProductionPhotoEncoder::new(ResourceLimits::default()),
                &CancellationToken::new(),
                &mut NoProgress,
            )
            .unwrap();
        assert_full_component_profile(&report);
        assert!(output.exists());
    }

    #[test]
    fn negative_local_sharpness_never_creates_a_production_target() {
        use crate::{
            develop::PresetCatalog,
            job::{DevelopJobRunner, JobErrorCode, JobStage, NoProgress},
        };
        let directory = tempdir().unwrap();
        let input = directory.path().join("source.jpg");
        let output = directory.path().join("must-not-exist.jpg");
        jpeg_solid(&input, 4, 3, [30, 60, 90]);
        let mut job = geometry_mask_job(&input, &output, OutputFormat::Jpeg);
        let mut settings = geometry_mask_settings();
        settings.radial_masks.masks[0].adjustments.sharpness = -1.0;
        job.preset = crate::job::PresetSelection::document(crate::develop::PresetDocument::new(
            "negative-local-sharpness-production",
            "Negative local sharpness production",
            settings,
        ));
        let failure = DevelopJobRunner::new(PresetCatalog::built_in().unwrap())
            .run(
                &job,
                &ProductionPhotoDecoder::new(),
                &ProductionPhotoEncoder::new(ResourceLimits::default()),
                &CancellationToken::new(),
                &mut NoProgress,
            )
            .unwrap_err();
        assert_eq!(failure.error.stage, JobStage::ResolveSettings);
        assert_eq!(failure.error.code, JobErrorCode::UnprovenPipelineBudget);
        assert!(!output.exists());
    }

    #[test]
    fn raster_jpeg_geometry_mask_job_reports_exact_develop_peak() {
        use crate::{
            develop::PresetCatalog,
            job::{DevelopJobRunner, NoProgress},
        };
        let directory = tempdir().unwrap();
        let input = directory.path().join("source.jpg");
        let output = directory.path().join("developed.jpg");
        jpeg_solid(&input, 4, 3, [30, 60, 90]);
        let report = DevelopJobRunner::new(PresetCatalog::built_in().unwrap())
            .run(
                &geometry_mask_job(&input, &output, OutputFormat::Jpeg),
                &ProductionPhotoDecoder::new(),
                &ProductionPhotoEncoder::new(ResourceLimits::default()),
                &CancellationToken::new(),
                &mut NoProgress,
            )
            .unwrap();
        let profile = report.develop_working_set.profile().unwrap();
        assert!(profile.geometry_v1 && profile.radial_masks_v1);
        assert_eq!(report.develop_working_set.estimated_peak_bytes(), 768);
        let decoded = image::open(output).unwrap();
        assert_eq!((decoded.width(), decoded.height()), (3, 4));
    }

    #[cfg(feature = "heic")]
    #[test]
    fn raster_heic_geometry_mask_job_reports_exact_develop_peak() {
        use crate::{
            develop::PresetCatalog,
            job::{DevelopJobRunner, NoProgress},
        };
        let directory = tempdir().unwrap();
        let input = directory.path().join("source.jpg");
        let output = directory.path().join("developed.heic");
        jpeg_solid(&input, 4, 3, [30, 60, 90]);
        let report = DevelopJobRunner::new(PresetCatalog::built_in().unwrap())
            .run(
                &geometry_mask_job(&input, &output, OutputFormat::Heic),
                &ProductionPhotoDecoder::new(),
                &ProductionPhotoEncoder::new(ResourceLimits::default()),
                &CancellationToken::new(),
                &mut NoProgress,
            )
            .unwrap();
        let profile = report.develop_working_set.profile().unwrap();
        assert!(profile.geometry_v1 && profile.radial_masks_v1);
        assert_eq!(report.develop_working_set.estimated_peak_bytes(), 768);
        assert!(output.exists());
    }

    #[test]
    fn raw_jpeg_runs_exposure_geometry_color_spatial_and_mask_stack() {
        use crate::{
            develop::PresetCatalog,
            job::{DevelopJobRunner, NoProgress},
        };
        let directory = tempdir().unwrap();
        let input = directory.path().join("source.nef");
        let output = directory.path().join("developed.jpg");
        stdfs::write(&input, b"synthetic raw source").unwrap();
        let report = DevelopJobRunner::new(PresetCatalog::built_in().unwrap())
            .run(
                &full_component_job(&input, &output, OutputFormat::Jpeg),
                &ProductionPhotoDecoder::with_raw(fake_raw_backend(directory.path())),
                &ProductionPhotoEncoder::new(ResourceLimits::default()),
                &CancellationToken::new(),
                &mut NoProgress,
            )
            .unwrap();
        let profile = report.develop_working_set.profile().unwrap();
        assert!(profile.pointwise_v1);
        assert!(profile.color_v1);
        assert!(profile.spatial_v1);
        assert!(profile.geometry_v1);
        assert!(profile.radial_masks_v1);
        assert!(report.develop_working_set.estimated_peak_bytes() > 0);
        assert!(report.scene_render.is_some());
    }

    fn fake_raw_backend(directory: &Path) -> RawExecutionOptions {
        let executable = directory.join("dcraw_emu");
        stdfs::write(
            &executable,
            "#!/bin/sh\nout=''\nwhile [ \"$#\" -gt 0 ]; do\n  if [ \"$1\" = '-Z' ]; then shift; out=$1; fi\n  shift\ndone\nprintf 'P6\\n1 1\\n65535\\n\\377\\377\\100\\000\\000\\000' > \"$out\"\n",
        )
        .unwrap();
        stdfs::set_permissions(&executable, stdfs::Permissions::from_mode(0o700)).unwrap();
        let mut execution = RawExecutionOptions::new(&executable).unwrap();
        execution.staging_directory = directory.to_owned();
        execution.timeout = Duration::from_secs(2);
        execution
    }

    #[test]
    fn opened_raster_survives_path_replacement_and_keeps_original_identity() {
        let directory = tempdir().unwrap();
        let original = directory.path().join("photo.raw");
        let moved = directory.path().join("moved.jpg");
        jpeg(&original, [120, 80, 40]);
        let file = File::open(&original).unwrap();
        let identity = crate::io::SourceFileIdentity::from_file(&file).unwrap();
        stdfs::rename(&original, &moved).unwrap();
        stdfs::write(&original, b"replacement raw bytes").unwrap();

        let result = ProductionPhotoDecoder::new()
            .decode_opened(
                file,
                &original,
                &DecodeOptions::default(),
                &CancellationToken::new(),
            )
            .unwrap();
        assert_eq!(result.source_identity(), identity);
        assert_eq!(
            result.photo.source_digest(),
            crate::io::SourceDigestV1::from_bytes(&stdfs::read(moved).unwrap())
        );
    }

    #[test]
    fn unknown_signature_routes_the_same_open_file_to_raw() {
        let directory = tempdir().unwrap();
        let original = directory.path().join("source.jpg");
        stdfs::write(&original, b"synthetic raw source").unwrap();
        let file = File::open(&original).unwrap();
        let identity = crate::io::SourceFileIdentity::from_file(&file).unwrap();
        let decoder = ProductionPhotoDecoder::with_raw(fake_raw_backend(directory.path()));
        let result = decoder
            .decode_opened(
                file,
                &original,
                &DecodeOptions::default(),
                &CancellationToken::new(),
            )
            .unwrap();
        assert_eq!(result.source_identity(), identity);
        assert_eq!(
            result.photo.source_digest(),
            crate::io::SourceDigestV1::from_bytes(b"synthetic raw source")
        );
        assert_eq!(
            result.photo.signal_relation(),
            crate::io::SignalRelation::SceneRelatedRaw
        );
    }

    #[test]
    fn fake_raw_runs_once_through_scene_render_and_publishes_jpeg() {
        use crate::{
            develop::{ParameterOverride, PresetCatalog},
            io::{AlphaPolicy, MetadataPolicy, OutputProfile, OverwritePolicy, SdrRangePolicy},
            job::{
                DevelopJob, DevelopJobOutcome, DevelopJobRunner, DevelopOutput, NoProgress,
                PresetSelection, ReportSignalRelation,
            },
        };

        let directory = tempdir().unwrap();
        let input = directory.path().join("source.nef");
        let output = directory.path().join("developed.jpg");
        stdfs::write(&input, b"synthetic raw source").unwrap();
        let limits = ResourceLimits::default();
        let job = DevelopJob {
            input,
            output: output.clone(),
            decode: DecodeOptions::default(),
            output_options: DevelopOutput::new(
                OutputFormat::Jpeg,
                90,
                OutputProfile::Srgb,
                MetadataPolicy::StripLocation,
                AlphaPolicy::Reject,
                SdrRangePolicy::ClipAndReport,
            ),
            overwrite: OverwritePolicy::Forbid,
            preset: PresetSelection::CatalogId("neutral".to_owned()),
            overrides: vec![
                ParameterOverride::scalar("basics.brightness", 35.0),
                ParameterOverride::scalar("basics.clarity", 20.0),
                ParameterOverride::scalar("color_mixer.red.saturation", 15.0),
            ],
        };
        let decoder = ProductionPhotoDecoder::with_raw(fake_raw_backend(directory.path()));
        let report = DevelopJobRunner::new(PresetCatalog::built_in().unwrap())
            .run(
                &job,
                &decoder,
                &ProductionJpegEncoder::new(limits),
                &CancellationToken::new(),
                &mut NoProgress,
            )
            .unwrap();

        assert_eq!(
            report.input_signal_relation,
            Some(ReportSignalRelation::SceneRelatedRaw)
        );
        assert_eq!(
            report.output_signal_relation,
            Some(ReportSignalRelation::LinearizedDisplayReferred)
        );
        assert!(report.scene_render.is_some());
        assert_eq!(
            report.develop_working_set.profile(),
            Some(crate::job::ReportDevelopWorkingSetProfile::ColorSpatialV1)
        );
        // The RAW job peak is the maximum of its sequential develop and scene
        // render phases; the spatial develop phase dominates for this 1x1 case.
        assert_eq!(report.develop_working_set.estimated_peak_bytes(), 316);
        assert!(matches!(
            report.outcome,
            DevelopJobOutcome::PublishedAndDurable { bytes_written } if bytes_written > 0
        ));
        let decoded = image::open(&output).unwrap().to_rgb8();
        assert_eq!(decoded.dimensions(), (1, 1));
    }

    #[cfg(feature = "heic")]
    #[test]
    fn fake_raw_runs_once_through_scene_render_and_publishes_ten_bit_heic() {
        use crate::{
            develop::{ParameterOverride, PresetCatalog},
            io::{AlphaPolicy, MetadataPolicy, OutputProfile, OverwritePolicy, SdrRangePolicy},
            job::{
                DevelopJob, DevelopJobOutcome, DevelopJobRunner, DevelopOutput, EncodeSummary,
                NoProgress, PresetSelection, ReportSignalRelation,
            },
        };

        let directory = tempdir().unwrap();
        let input = directory.path().join("source.nef");
        let output = directory.path().join("developed.heic");
        stdfs::write(&input, b"synthetic raw source").unwrap();
        let limits = ResourceLimits::default();
        let job = DevelopJob {
            input,
            output: output.clone(),
            decode: DecodeOptions::default(),
            output_options: DevelopOutput::new(
                OutputFormat::Heic,
                90,
                OutputProfile::Srgb,
                MetadataPolicy::StripLocation,
                AlphaPolicy::Reject,
                SdrRangePolicy::ClipAndReport,
            ),
            overwrite: OverwritePolicy::Forbid,
            preset: PresetSelection::CatalogId("neutral".to_owned()),
            overrides: vec![ParameterOverride::scalar("basics.brightness", 35.0)],
        };
        let decoder = ProductionPhotoDecoder::with_raw(fake_raw_backend(directory.path()));
        let report = DevelopJobRunner::new(PresetCatalog::built_in().unwrap())
            .run(
                &job,
                &decoder,
                &ProductionPhotoEncoder::new(limits),
                &CancellationToken::new(),
                &mut NoProgress,
            )
            .unwrap();

        assert_eq!(
            report.input_signal_relation,
            Some(ReportSignalRelation::SceneRelatedRaw)
        );
        assert!(report.scene_render.is_some());
        assert!(matches!(
            report.encoding.as_deref(),
            Some(EncodeSummary::Heic { bit_depth: 10, .. })
        ));
        assert!(matches!(
            report.outcome,
            DevelopJobOutcome::PublishedAndDurable { bytes_written } if bytes_written > 0
        ));
        assert!(
            stdfs::read(output)
                .unwrap()
                .windows(4)
                .any(|window| window == b"ftyp")
        );
    }

    #[cfg(feature = "heic")]
    #[test]
    fn production_heic_adapter_propagates_cancellation_without_publication() {
        use crate::{
            io::{AlphaPolicy, MetadataPolicy, OutputProfile, OverwritePolicy, SdrRangePolicy},
            job::DecodedArtifact,
        };

        let directory = tempdir().unwrap();
        let input = directory.path().join("source.jpg");
        let output = directory.path().join("cancelled.heic");
        jpeg(&input, [30, 60, 90]);
        let limits = ResourceLimits::default();
        let decoded = ProductionPhotoDecoder::new()
            .decode_path_once(&input, &DecodeOptions::default(), &CancellationToken::new())
            .unwrap();
        let (photo, source) = decoded.into_parts();
        let DecodedArtifact::Display(artifact) =
            DecodedArtifact::try_from_photo(photo, &limits).unwrap()
        else {
            panic!("raster must be display-referred");
        };
        let cancellation = CancellationToken::new();
        cancellation.cancel();
        let options = EncodeOptions {
            format: OutputFormat::Heic,
            quality: 90,
            profile: OutputProfile::Srgb,
            metadata: MetadataPolicy::StripLocation,
            alpha: AlphaPolicy::Reject,
            range: SdrRangePolicy::ClipAndReport,
        };
        assert!(matches!(
            ProductionPhotoEncoder::new(limits).encode_display(
                PublicationRequest {
                    destination: &output,
                    source: &source,
                    overwrite: OverwritePolicy::Forbid,
                },
                &artifact,
                &options,
                &cancellation,
            ),
            Err(EncodeError::Cancelled)
        ));
        assert!(!output.exists());
    }

    #[test]
    fn corrupt_raster_magic_never_falls_back_to_raw() {
        let directory = tempdir().unwrap();
        let source = directory.path().join("corrupt.nef");
        stdfs::write(&source, b"\xff\xd8\xffbroken").unwrap();
        let decoder = ProductionPhotoDecoder::with_raw(fake_raw_backend(directory.path()));
        assert!(matches!(
            decoder.decode_path_once(
                &source,
                &DecodeOptions::default(),
                &CancellationToken::new()
            ),
            Err(DecodeError::CorruptInput)
                | Err(DecodeError::ColorManagement)
                | Err(DecodeError::Metadata)
        ));
    }

    #[test]
    fn pre_cancelled_job_never_opens_input_or_creates_target() {
        use crate::{
            develop::PresetCatalog,
            io::{AlphaPolicy, MetadataPolicy, OutputProfile, OverwritePolicy, SdrRangePolicy},
            job::{
                DevelopJob, DevelopJobRunner, DevelopOutput, JobErrorCode, NoProgress,
                PresetSelection,
            },
        };
        let directory = tempdir().unwrap();
        let output = directory.path().join("never-created.jpg");
        let limits = ResourceLimits::default();
        let job = DevelopJob {
            input: directory.path().join("does-not-exist.jpg"),
            output: output.clone(),
            decode: DecodeOptions::default(),
            output_options: DevelopOutput::new(
                OutputFormat::Jpeg,
                90,
                OutputProfile::Srgb,
                MetadataPolicy::StripLocation,
                AlphaPolicy::Reject,
                SdrRangePolicy::ClipAndReport,
            ),
            overwrite: OverwritePolicy::Forbid,
            preset: PresetSelection::CatalogId("neutral".to_owned()),
            overrides: Vec::new(),
        };
        let cancellation = CancellationToken::new();
        cancellation.cancel();
        let failure = DevelopJobRunner::new(PresetCatalog::built_in().unwrap())
            .run(
                &job,
                &ProductionPhotoDecoder::new(),
                &ProductionJpegEncoder::new(limits),
                &cancellation,
                &mut NoProgress,
            )
            .unwrap_err();
        assert_eq!(failure.error.code, JobErrorCode::Cancelled);
        assert!(!output.exists());
    }

    #[test]
    fn runner_holds_the_same_open_source_lease_through_encoder_commit() {
        use crate::{
            develop::PresetCatalog,
            io::{AlphaPolicy, MetadataPolicy, OutputProfile, OverwritePolicy, SdrRangePolicy},
            job::{DevelopJob, DevelopJobRunner, DevelopOutput, NoProgress, PresetSelection},
        };

        struct LeaseCheckingEncoder;
        impl PhotoEncoder for LeaseCheckingEncoder {
            type Error = EncodeError;

            fn encode_display(
                &self,
                publication: PublicationRequest<'_>,
                _artifact: &WorkingArtifact<DisplayReferred>,
                _options: &EncodeOptions,
                _cancellation: &CancellationToken,
            ) -> Result<EncodeReceipt, Self::Error> {
                assert!(publication.source.held_file().metadata().is_ok());
                assert_eq!(
                    publication.source.identity(),
                    crate::io::SourceFileIdentity::from_file(publication.source.held_file())
                        .unwrap()
                );
                Ok(EncodeReceipt {
                    bytes_written: 1,
                    publication: PublicationStatus::PublishedAndDurable,
                    summary: None,
                })
            }
        }

        let directory = tempdir().unwrap();
        let input = directory.path().join("source.png");
        jpeg(&input, [30, 60, 90]);
        let job = DevelopJob {
            input,
            output: directory.path().join("virtual.jpg"),
            decode: DecodeOptions::default(),
            output_options: DevelopOutput::new(
                OutputFormat::Jpeg,
                90,
                OutputProfile::Srgb,
                MetadataPolicy::StripLocation,
                AlphaPolicy::Reject,
                SdrRangePolicy::ClipAndReport,
            ),
            overwrite: OverwritePolicy::Forbid,
            preset: PresetSelection::CatalogId("neutral".to_owned()),
            overrides: Vec::new(),
        };
        DevelopJobRunner::new(PresetCatalog::built_in().unwrap())
            .run(
                &job,
                &ProductionPhotoDecoder::new(),
                &LeaseCheckingEncoder,
                &CancellationToken::new(),
                &mut NoProgress,
            )
            .unwrap();
    }
}
