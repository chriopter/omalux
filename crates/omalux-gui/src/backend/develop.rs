use omalux::{
    develop::{
        DevelopSettings, ParameterKind, PresetCatalog, PresetDocument, apply_parameter_overrides,
        estimate_develop_working_set, parameter_registry, parse_parameter_override,
    },
    io::{
        AlphaPolicy, AtomicOutputError, AtomicOutputOptions, AtomicOutputOutcome, DecodeOptions,
        MetadataPolicy, OutputFormat, OutputProfile, OverwritePolicy, ResourceLimits,
        SdrRangePolicy, SourceFileIdentity, write_atomic_output_for_source,
    },
    job::{
        CancellationToken, DecodedSource, DevelopJob, DevelopJobFailure, DevelopJobReport,
        DevelopJobRunner, DevelopOutput, NoProgress, PhotoDecoder, PresetSelection,
        ProductionPhotoDecoder, ProductionPhotoEncoder,
    },
};
use serde_json::json;
use std::{
    fs::File,
    io,
    os::unix::fs::{FileExt, PermissionsExt},
    path::{Path, PathBuf},
};
use tempfile::TempDir;

#[derive(Debug)]
pub(super) enum GuiJobError {
    Setup(String),
    Develop(DevelopJobFailure),
}

impl std::fmt::Display for GuiJobError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Setup(message) => formatter.write_str(message),
            Self::Develop(failure) => write!(
                formatter,
                "Development failed at {:?} ({:?})",
                failure.error.stage, failure.error.code
            ),
        }
    }
}

/// A private, bounded JPEG owned by one completed preview generation.
///
/// Keeping the directory handle in this value makes replacement and shutdown
/// cleanup automatic; no predictable shared `/tmp` pathname is used.
pub(super) struct PreviewArtifact {
    _directory: TempDir,
    path: PathBuf,
    report: DevelopJobReport,
}

impl PreviewArtifact {
    pub(super) fn path(&self) -> &Path {
        &self.path
    }

    pub(super) fn report(&self) -> &DevelopJobReport {
        &self.report
    }

    #[cfg(test)]
    fn directory_path(&self) -> &Path {
        self._directory.path()
    }
}

pub(super) fn built_in_catalog_json() -> Result<String, String> {
    let catalog = PresetCatalog::built_in().map_err(|error| error.to_string())?;
    serde_json::to_string(&json!({
        "presets": catalog.documents().iter().map(|document| json!({
            "id": document.id,
            "name": document.name,
        })).collect::<Vec<_>>()
    }))
    .map_err(|error| error.to_string())
}

pub(super) fn built_in_settings(id: &str) -> Result<DevelopSettings, String> {
    PresetCatalog::built_in()
        .map_err(|error| error.to_string())?
        .get(id)
        .map(|document| document.settings.clone())
        .ok_or_else(|| "Unknown built-in preset".to_owned())
}

pub(super) fn settings_json(settings: &DevelopSettings) -> Result<String, String> {
    serde_json::to_string(settings).map_err(|error| error.to_string())
}

pub(super) fn supported_parameters_json() -> Result<String, String> {
    let limits = ResourceLimits::default();
    let mut supported = Vec::new();
    for parameter in parameter_registry() {
        if parameter.kind != ParameterKind::Scalar {
            continue;
        }
        let active = if parameter.neutral != parameter.maximum {
            parameter.maximum
        } else {
            parameter.minimum
        };
        let Ok(parsed) = parse_parameter_override(&format!("{}={active}", parameter.id)) else {
            continue;
        };
        let Ok(settings) = apply_parameter_overrides(&DevelopSettings::default(), &[parsed]) else {
            // Composite parameters such as crop extents cannot necessarily be
            // activated safely in isolation and are omitted from this scalar
            // control capability view.
            continue;
        };
        if estimate_develop_working_set(64, 64, &settings, &limits).is_ok() {
            supported.push(parameter.id);
        }
    }
    serde_json::to_string(&supported).map_err(|error| error.to_string())
}

pub(super) fn develop_preview(
    source: &Path,
    settings: DevelopSettings,
    full_resolution: bool,
    cancellation: &CancellationToken,
) -> Result<PreviewArtifact, GuiJobError> {
    let directory = tempfile::Builder::new()
        .prefix("omalux-preview-")
        .tempdir()
        .map_err(|error| {
            GuiJobError::Setup(format!("Could not create private preview storage: {error}"))
        })?;
    std::fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(0o700)).map_err(
        |error| GuiJobError::Setup(format!("Could not secure private preview storage: {error}")),
    )?;
    let path = directory.path().join("preview.jpg");
    let report = run_job(
        source,
        &path,
        settings,
        OutputFormat::Jpeg,
        88,
        preview_limits(),
        OverwritePolicy::Forbid,
        if full_resolution {
            None
        } else {
            Some(PREVIEW_LONG_EDGE)
        },
        cancellation,
    )?;
    Ok(PreviewArtifact {
        _directory: directory,
        path,
        report,
    })
}

pub(super) fn export_photo(
    source: &Path,
    destination: &Path,
    settings: DevelopSettings,
    format: OutputFormat,
    quality: u8,
    cancellation: &CancellationToken,
) -> Result<DevelopJobReport, GuiJobError> {
    run_job(
        source,
        destination,
        settings,
        format,
        quality,
        ResourceLimits::default(),
        OverwritePolicy::Replace,
        None,
        cancellation,
    )
}

pub(super) fn save_original_atomic(
    source: &Path,
    destination: &Path,
) -> Result<AtomicOutputOutcome, String> {
    use rustix::fs::{self, FileType, Mode, OFlags};

    let source = fs::open(
        source,
        OFlags::RDONLY | OFlags::NONBLOCK | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map(File::from)
    .map_err(|error| format!("Could not safely open original: {error}"))?;
    let stat =
        fs::fstat(&source).map_err(|error| format!("Could not inspect original: {error}"))?;
    if FileType::from_raw_mode(stat.st_mode) != FileType::RegularFile {
        return Err("Original is not a regular file".to_owned());
    }
    let source_size =
        u64::try_from(stat.st_size).map_err(|_| "Original has an invalid size".to_owned())?;
    let identity = SourceFileIdentity::from_file(&source).map_err(|error| error.to_string())?;
    match write_atomic_output_for_source(
        destination,
        Some(identity),
        AtomicOutputOptions::default().with_overwrite(OverwritePolicy::Replace),
        |output| copy_held_file(&source, source_size, output),
    ) {
        Ok(outcome) => Ok(outcome),
        Err(AtomicOutputError::PublishedButNotDurable(_)) => {
            Ok(AtomicOutputOutcome::PublishedButNotDurable)
        }
        Err(error) => Err(error.to_string()),
    }
}

fn copy_held_file(source: &File, size: u64, output: &mut File) -> io::Result<()> {
    use std::io::Write;
    let mut offset = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];
    while offset < size {
        let remaining =
            usize::try_from((size - offset).min(buffer.len() as u64)).map_err(io::Error::other)?;
        let count = source.read_at(&mut buffer[..remaining], offset)?;
        if count == 0 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "original changed while being copied",
            ));
        }
        output.write_all(&buffer[..count])?;
        offset = offset
            .checked_add(count as u64)
            .ok_or_else(|| io::Error::other("original size overflow"))?;
    }
    Ok(())
}

/// Interactive previews develop a bounded proxy so slider feedback stays
/// far below perceptual latency; exports keep full resolution.
const PREVIEW_LONG_EDGE: u32 = 2048;

fn preview_limits() -> ResourceLimits {
    ResourceLimits::default().with_max_output_bytes(64 << 20)
}

/// Decoder that memoizes the downscaled decode of the most recent source so
/// interactive slider changes re-run only the develop stages, never the
/// multi-second full decode. The cache key covers path, size, and mtime; the
/// runner's own proxy step becomes a no-op on the already reduced image.
struct CachingProxyDecoder {
    inner: ProductionPhotoDecoder,
    long_edge: u32,
}

type ProxyCacheKey = (std::path::PathBuf, u64, Option<std::time::SystemTime>, u32);
static PROXY_CACHE: std::sync::Mutex<Option<(ProxyCacheKey, omalux::io::DecodedPhoto)>> =
    std::sync::Mutex::new(None);

impl PhotoDecoder for CachingProxyDecoder {
    type Error = <ProductionPhotoDecoder as PhotoDecoder>::Error;

    fn decode_path_once(
        &self,
        input: &Path,
        options: &DecodeOptions,
        cancellation: &CancellationToken,
    ) -> Result<DecodedSource, Self::Error> {
        let metadata = std::fs::metadata(input).ok();
        let key: ProxyCacheKey = (
            input.to_owned(),
            metadata.as_ref().map(|m| m.len()).unwrap_or(0),
            metadata.and_then(|m| m.modified().ok()),
            self.long_edge,
        );
        if let Some((cached_key, photo)) = PROXY_CACHE.lock().unwrap().as_ref()
            && *cached_key == key
            && let Ok(file) = std::fs::File::open(input)
            && let Ok(source) = DecodedSource::from_held_file(photo.clone(), file)
        {
            return Ok(source);
        }
        let mut decoded = self.inner.decode_path_once(input, options, cancellation)?;
        decoded
            .photo
            .downscale_to_long_edge(self.long_edge)
            .map_err(|_| omalux::io::DecodeError::CorruptInput)?;
        *PROXY_CACHE.lock().unwrap() = Some((key, decoded.photo.clone()));
        Ok(decoded)
    }
}

fn preview_raw_cache() -> Option<std::path::PathBuf> {
    let base = std::env::var_os("XDG_CACHE_HOME")
        .map(std::path::PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| Path::new(&home).join(".cache")))?;
    let directory = base.join("omalux").join("raw-decode");
    std::fs::create_dir_all(&directory).ok()?;
    std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o700)).ok();
    Some(directory)
}

#[allow(clippy::too_many_arguments)]
fn run_job(
    source: &Path,
    destination: &Path,
    settings: DevelopSettings,
    format: OutputFormat,
    quality: u8,
    limits: ResourceLimits,
    overwrite: OverwritePolicy,
    proxy_long_edge: Option<u32>,
    cancellation: &CancellationToken,
) -> Result<DevelopJobReport, GuiJobError> {
    let catalog = PresetCatalog::built_in()
        .map_err(|error| GuiJobError::Setup(format!("Built-in preset catalog failed: {error}")))?;
    let mut decode = DecodeOptions::default();
    decode.limits = limits;
    decode.proxy_long_edge = proxy_long_edge;
    if proxy_long_edge.is_some() {
        decode.raw.decode_cache = preview_raw_cache();
    }
    let job = DevelopJob {
        input: source.to_owned(),
        output: destination.to_owned(),
        decode,
        output_options: DevelopOutput::new(
            format,
            quality,
            OutputProfile::Srgb,
            MetadataPolicy::StripLocation,
            AlphaPolicy::Flatten([0.0; 3]),
            SdrRangePolicy::ClipAndReport,
        ),
        overwrite,
        preset: PresetSelection::document(PresetDocument::new(
            "gui-session",
            "GUI session",
            settings,
        )),
        overrides: Vec::new(),
    };
    let runner = DevelopJobRunner::new(catalog);
    let encoder = ProductionPhotoEncoder::new(limits);
    let outcome = match proxy_long_edge {
        Some(long_edge) => runner.run(
            &job,
            &CachingProxyDecoder {
                inner: ProductionPhotoDecoder::new(),
                long_edge,
            },
            &encoder,
            cancellation,
            &mut NoProgress,
        ),
        None => runner.run(
            &job,
            &ProductionPhotoDecoder::new(),
            &encoder,
            cancellation,
            &mut NoProgress,
        ),
    };
    outcome.map_err(GuiJobError::Develop)
}

#[cfg(test)]
mod tests {
    use super::*;
    use omalux::develop::{
        LocalAdjustments, ParameterOverrideValue, RadialMask, parse_parameter_override,
    };
    use std::fs;

    fn jpeg_fixture(path: &Path) {
        let image = image::RgbImage::from_pixel(8, 6, image::Rgb([80, 120, 160]));
        image.save(path).unwrap();
    }

    fn geometry_radial_settings() -> DevelopSettings {
        use omalux::develop::CurvePoint;

        let mut settings = DevelopSettings::default();
        settings.basics.exposure_ev = 0.5;
        settings.basics.clarity = 12.0;
        settings.geometry.quarter_turns_clockwise = 1;
        settings.geometry.straighten_degrees = 2.0;
        settings.tone_curves.master.points = vec![
            CurvePoint { x: 0.0, y: 0.0 },
            CurvePoint { x: 0.25, y: 0.18 },
            CurvePoint { x: 0.65, y: 0.78 },
            CurvePoint { x: 1.0, y: 1.0 },
        ];
        settings.color_mixer.red.saturation = 15.0;
        settings.effects.bloom = 8.0;
        settings.radial_masks.masks.push(RadialMask {
            id: "gui-positive-local-mask".to_owned(),
            enabled: true,
            center_x: 0.5,
            center_y: 0.5,
            radius_x: 0.35,
            radius_y: 0.3,
            rotation_degrees: 15.0,
            feather: 0.5,
            opacity: 0.8,
            invert: false,
            adjustments: LocalAdjustments {
                exposure_ev: 0.75,
                brightness: 8.0,
                sharpness: 12.0,
                ..LocalAdjustments::default()
            },
        });
        settings
    }

    fn local_exposure_settings(exposure_ev: f32) -> DevelopSettings {
        let mut settings = DevelopSettings::default();
        settings.radial_masks.masks.push(RadialMask {
            id: "gui-local-exposure-only".to_owned(),
            enabled: true,
            center_x: 0.5,
            center_y: 0.5,
            radius_x: 2.0,
            radius_y: 2.0,
            rotation_degrees: 0.0,
            feather: 0.0,
            opacity: 1.0,
            invert: false,
            adjustments: LocalAdjustments {
                exposure_ev,
                ..LocalAdjustments::default()
            },
        });
        settings
    }

    #[test]
    fn catalog_and_settings_are_core_owned_json() {
        let catalog: serde_json::Value =
            serde_json::from_str(&built_in_catalog_json().unwrap()).unwrap();
        assert_eq!(catalog["presets"].as_array().unwrap().len(), 28);
        assert_eq!(catalog["presets"][0]["id"], "community-amber-grain");
        assert!(
            catalog["presets"]
                .as_array()
                .unwrap()
                .iter()
                .any(|entry| entry["id"] == "neutral")
        );
        let settings = built_in_settings("neutral").unwrap();
        let parsed: DevelopSettings =
            serde_json::from_str(&settings_json(&settings).unwrap()).unwrap();
        assert_eq!(parsed, settings);
    }

    #[test]
    fn supported_control_list_is_derived_from_the_bounded_core_profiles() {
        let ids: Vec<String> = serde_json::from_str(&supported_parameters_json().unwrap()).unwrap();
        assert!(ids.iter().any(|id| id == "basics.exposure_ev"));
        assert!(ids.iter().any(|id| id == "basics.contrast"));
        assert!(ids.iter().any(|id| id == "color_mixer.red.saturation"));
        for spatial in [
            "basics.clarity",
            "effects.bloom",
            "effects.halation",
            "effects.sharpness",
        ] {
            assert!(ids.iter().any(|id| id == spatial));
        }
        for geometry in [
            "geometry.quarter_turns_clockwise",
            "geometry.straighten_degrees",
            "geometry.perspective_horizontal",
            "geometry.perspective_vertical",
            "geometry.crop.width",
            "geometry.crop.height",
        ] {
            assert!(ids.iter().any(|id| id == geometry));
        }
        // Crop origins require a compatible extent and therefore travel as a
        // complete settingsJson transaction, not as an isolated scalar.
        assert!(!ids.iter().any(|id| id == "geometry.crop.x"));
        assert!(!ids.iter().any(|id| id == "geometry.crop.y"));
        // Structured mask controls are transported by settingsJson rather
        // than the scalar setParameter bridge. In particular, the bridge must
        // never advertise local sharpness as an unrestricted -100..100
        // scalar while the bounded pipeline intentionally rejects negatives.
        assert!(
            !ids.iter()
                .any(|id| id == "radial_masks[].adjustments.sharpness")
        );
        assert!(
            !ids.iter()
                .any(|id| id == "radial_masks[].adjustments.exposure_ev")
        );
        let clarity = parse_parameter_override("basics.clarity=100").unwrap();
        let clarity = apply_parameter_overrides(&DevelopSettings::default(), &[clarity]).unwrap();
        assert_eq!(
            ids.iter().any(|id| id == "basics.clarity"),
            estimate_develop_working_set(64, 64, &clarity, &ResourceLimits::default()).is_ok()
        );
    }

    #[test]
    fn settings_json_preserves_geometry_and_positive_radial_profiles() {
        let settings = geometry_radial_settings();
        let parsed: DevelopSettings =
            serde_json::from_str(&settings_json(&settings).unwrap()).unwrap();
        assert_eq!(parsed, settings);
        assert_eq!(parsed.radial_masks.masks[0].adjustments.exposure_ev, 0.75);
        let estimate =
            estimate_develop_working_set(8, 6, &parsed, &ResourceLimits::default()).unwrap();
        assert!(estimate.profile.geometry_v1);
        assert!(estimate.profile.radial_masks_v1);
        assert!(estimate.profile.color_v1);
        assert!(estimate.profile.spatial_v1);

        let mut negative = parsed;
        negative.radial_masks.masks[0].adjustments.sharpness = -1.0;
        assert!(
            estimate_develop_working_set(8, 6, &negative, &ResourceLimits::default(),).is_err()
        );
    }

    #[test]
    fn preview_is_private_core_pipeline_jpeg_and_cleans_up() {
        let input = tempfile::NamedTempFile::with_suffix(".jpg").unwrap();
        jpeg_fixture(input.path());
        let preview = develop_preview(
            input.path(),
            DevelopSettings::default(),
            false,
            &CancellationToken::new(),
        )
        .unwrap();
        assert!(preview.path().is_file());
        assert_eq!(
            fs::metadata(preview.directory_path())
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o700
        );
        assert!(fs::metadata(preview.path()).unwrap().len() <= 64 << 20);
        let path = preview.path().to_owned();
        drop(preview);
        assert!(!path.exists());
    }

    #[test]
    fn preview_artifact_isolates_local_exposure_from_settings_json() {
        let input = tempfile::NamedTempFile::with_suffix(".jpg").unwrap();
        jpeg_fixture(input.path());
        let neutral = develop_preview(
            input.path(),
            DevelopSettings::default(),
            false,
            &CancellationToken::new(),
        )
        .unwrap();
        let local_settings: DevelopSettings =
            serde_json::from_str(&settings_json(&local_exposure_settings(1.0)).unwrap()).unwrap();
        let local = develop_preview(
            input.path(),
            local_settings,
            false,
            &CancellationToken::new(),
        )
        .unwrap();
        let sum = |path: &Path| {
            image::open(path)
                .unwrap()
                .to_rgb8()
                .pixels()
                .flat_map(|pixel| pixel.0)
                .map(u64::from)
                .sum::<u64>()
        };
        assert!(sum(local.path()) > sum(neutral.path()));
    }

    #[test]
    fn pointwise_and_color_v1_settings_reach_the_same_job_adapter() {
        let input = tempfile::NamedTempFile::with_suffix(".jpg").unwrap();
        jpeg_fixture(input.path());
        for expression in ["basics.contrast=12", "color_mixer.red.saturation=15"] {
            let mut settings = DevelopSettings::default();
            let value = parse_parameter_override(expression).unwrap();
            assert!(matches!(value.value(), ParameterOverrideValue::Scalar(_)));
            omalux::develop::apply_parameter_overrides(&settings, &[value])
                .map(|resolved| settings = resolved)
                .unwrap();
            develop_preview(input.path(), settings, false, &CancellationToken::new()).unwrap();
        }
    }

    #[test]
    fn representative_built_ins_reach_the_gui_preview_job_adapter() {
        let input = tempfile::NamedTempFile::with_suffix(".jpg").unwrap();
        jpeg_fixture(input.path());
        for id in [
            "personal-verbania",
            "personal-blume",
            "series-alpine-cross",
            "community-honey-hour",
            "personal-lampe-1",
        ] {
            let preview = develop_preview(
                input.path(),
                built_in_settings(id).unwrap(),
                false,
                &CancellationToken::new(),
            )
            .unwrap_or_else(|error| panic!("built-in {id} failed in GUI preview: {error}"));
            assert!(preview.path().is_file(), "{id}");
            assert_eq!(image::image_dimensions(preview.path()).unwrap(), (8, 6));
        }
    }

    #[test]
    fn combined_color_spatial_settings_reach_preview_and_report_the_real_profile() {
        use omalux::job::ReportDevelopWorkingSetProfile;

        let input = tempfile::NamedTempFile::with_suffix(".jpg").unwrap();
        jpeg_fixture(input.path());
        let overrides = [
            parse_parameter_override("color_mixer.red.saturation=15").unwrap(),
            parse_parameter_override("basics.clarity=12").unwrap(),
            parse_parameter_override("effects.bloom=8").unwrap(),
        ];
        let settings = apply_parameter_overrides(&DevelopSettings::default(), &overrides).unwrap();
        let preview =
            develop_preview(input.path(), settings, false, &CancellationToken::new()).unwrap();
        assert_eq!(
            preview.report().develop_working_set.profile(),
            Some(ReportDevelopWorkingSetProfile::ColorSpatialV1)
        );
    }

    #[test]
    fn geometry_radial_settings_reach_preview_and_jpeg_export() {
        let input = tempfile::NamedTempFile::with_suffix(".jpg").unwrap();
        jpeg_fixture(input.path());
        let settings = geometry_radial_settings();

        let preview = develop_preview(
            input.path(),
            settings.clone(),
            false,
            &CancellationToken::new(),
        )
        .unwrap();
        let preview_profile = preview.report().develop_working_set.profile().unwrap();
        assert!(preview_profile.geometry_v1);
        assert!(preview_profile.radial_masks_v1);
        assert!(preview_profile.color_v1);
        assert!(preview_profile.spatial_v1);
        assert_eq!(image::image_dimensions(preview.path()).unwrap(), (6, 8));

        let directory = tempfile::tempdir().unwrap();
        let output = directory.path().join("geometry-radial.jpg");
        let report = export_photo(
            input.path(),
            &output,
            settings,
            OutputFormat::Jpeg,
            90,
            &CancellationToken::new(),
        )
        .unwrap();
        let export_profile = report.develop_working_set.profile().unwrap();
        assert!(export_profile.geometry_v1);
        assert!(export_profile.radial_masks_v1);
        assert!(export_profile.color_v1);
        assert!(export_profile.spatial_v1);
        assert_eq!(image::image_dimensions(output).unwrap(), (6, 8));
    }

    #[cfg(feature = "heic")]
    #[test]
    fn geometry_radial_settings_reach_heic_export() {
        let input = tempfile::NamedTempFile::with_suffix(".jpg").unwrap();
        jpeg_fixture(input.path());
        let directory = tempfile::tempdir().unwrap();
        let output = directory.path().join("geometry-radial.heic");
        let report = export_photo(
            input.path(),
            &output,
            geometry_radial_settings(),
            OutputFormat::Heic,
            90,
            &CancellationToken::new(),
        )
        .unwrap();
        let profile = report.develop_working_set.profile().unwrap();
        assert!(profile.geometry_v1);
        assert!(profile.radial_masks_v1);
        assert!(profile.color_v1);
        assert!(profile.spatial_v1);
        assert!(output.is_file());
        assert!(fs::metadata(output).unwrap().len() > 0);
    }

    #[test]
    fn pre_cancelled_preview_publishes_nothing() {
        let input = tempfile::NamedTempFile::with_suffix(".jpg").unwrap();
        jpeg_fixture(input.path());
        let cancellation = CancellationToken::new();
        cancellation.cancel();
        assert!(
            develop_preview(
                input.path(),
                DevelopSettings::default(),
                false,
                &cancellation
            )
            .is_err()
        );
    }

    #[test]
    fn original_copy_rejects_same_path_and_hardlink() {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("source.bin");
        fs::write(&source, b"private original").unwrap();
        assert!(save_original_atomic(&source, &source).is_err());
        let hardlink = directory.path().join("hardlink.bin");
        fs::hard_link(&source, &hardlink).unwrap();
        assert!(save_original_atomic(&source, &hardlink).is_err());
        assert_eq!(fs::read(&source).unwrap(), b"private original");
    }

    #[test]
    fn original_copy_is_atomic_private_and_exact() {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("source.bin");
        let destination = directory.path().join("copy.bin");
        fs::write(&source, b"original bytes").unwrap();
        assert_eq!(
            save_original_atomic(&source, &destination).unwrap(),
            AtomicOutputOutcome::PublishedAndDurable
        );
        assert_eq!(fs::read(&destination).unwrap(), b"original bytes");
        assert_eq!(
            fs::metadata(destination).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
}
