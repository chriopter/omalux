use grainroom::{
    develop::{DevelopSettings, PresetCatalog, PresetDocument},
    io::{
        AlphaPolicy, DecodeOptions, MetadataPolicy, OutputFormat, OutputProfile, OverwritePolicy,
        ResourceLimits, SdrRangePolicy,
    },
    job::{
        CancellationToken, DevelopJob, DevelopJobRunner, DevelopOutput, NoProgress,
        PresetSelection, ProductionPhotoDecoder, ProductionPhotoEncoder,
    },
};
use serde_json::json;
use std::{
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
};
use tempfile::TempDir;

/// A private, bounded JPEG owned by one completed preview generation.
///
/// Keeping the directory handle in this value makes replacement and shutdown
/// cleanup automatic; no predictable shared `/tmp` pathname is used.
pub(super) struct PreviewArtifact {
    _directory: TempDir,
    path: PathBuf,
}

impl PreviewArtifact {
    pub(super) fn path(&self) -> &Path {
        &self.path
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

pub(super) fn develop_preview(
    source: &Path,
    settings: DevelopSettings,
    cancellation: &CancellationToken,
) -> Result<PreviewArtifact, String> {
    let directory = tempfile::Builder::new()
        .prefix("grainroom-preview-")
        .tempdir()
        .map_err(|error| format!("Could not create private preview storage: {error}"))?;
    std::fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(0o700))
        .map_err(|error| format!("Could not secure private preview storage: {error}"))?;
    let path = directory.path().join("preview.jpg");
    run_job(
        source,
        &path,
        settings,
        OutputFormat::Jpeg,
        88,
        preview_limits(),
        OverwritePolicy::Forbid,
        cancellation,
    )?;
    Ok(PreviewArtifact {
        _directory: directory,
        path,
    })
}

pub(super) fn export_photo(
    source: &Path,
    destination: &Path,
    settings: DevelopSettings,
    format: OutputFormat,
    quality: u8,
    cancellation: &CancellationToken,
) -> Result<(), String> {
    run_job(
        source,
        destination,
        settings,
        format,
        quality,
        ResourceLimits::default(),
        OverwritePolicy::Replace,
        cancellation,
    )
}

fn preview_limits() -> ResourceLimits {
    // The preview is still full resolution until the core gains an audited
    // downsampling decode profile, but its artifact is strictly capped and
    // every decode/develop allocation remains core-bounded.
    ResourceLimits::default().with_max_output_bytes(64 << 20)
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
    cancellation: &CancellationToken,
) -> Result<(), String> {
    let catalog = PresetCatalog::built_in().map_err(|error| error.to_string())?;
    let mut decode = DecodeOptions::default();
    decode.limits = limits;
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
    DevelopJobRunner::new(catalog)
        .run(
            &job,
            &ProductionPhotoDecoder::new(),
            &ProductionPhotoEncoder::new(limits),
            cancellation,
            &mut NoProgress,
        )
        .map(|_| ())
        .map_err(|failure| {
            format!(
                "Development failed at {:?} ({:?})",
                failure.error.stage, failure.error.code
            )
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use grainroom::develop::{ParameterOverrideValue, parse_parameter_override};
    use std::fs;

    fn jpeg_fixture(path: &Path) {
        let image = image::RgbImage::from_pixel(2, 2, image::Rgb([80, 120, 160]));
        image.save(path).unwrap();
    }

    #[test]
    fn catalog_and_settings_are_core_owned_json() {
        let catalog: serde_json::Value =
            serde_json::from_str(&built_in_catalog_json().unwrap()).unwrap();
        assert_eq!(catalog["presets"][0]["id"], "neutral");
        let settings = built_in_settings("neutral").unwrap();
        let parsed: DevelopSettings =
            serde_json::from_str(&settings_json(&settings).unwrap()).unwrap();
        assert_eq!(parsed, settings);
    }

    #[test]
    fn preview_is_private_core_pipeline_jpeg_and_cleans_up() {
        let input = tempfile::NamedTempFile::with_suffix(".jpg").unwrap();
        jpeg_fixture(input.path());
        let preview = develop_preview(
            input.path(),
            DevelopSettings::default(),
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
    fn pointwise_and_color_v1_settings_reach_the_same_job_adapter() {
        let input = tempfile::NamedTempFile::with_suffix(".jpg").unwrap();
        jpeg_fixture(input.path());
        for expression in ["basics.contrast=12", "color_mixer.red.saturation=15"] {
            let mut settings = DevelopSettings::default();
            let value = parse_parameter_override(expression).unwrap();
            assert!(matches!(value.value(), ParameterOverrideValue::Scalar(_)));
            grainroom::develop::apply_parameter_overrides(&settings, &[value])
                .map(|resolved| settings = resolved)
                .unwrap();
            develop_preview(input.path(), settings, &CancellationToken::new()).unwrap();
        }
    }

    #[test]
    fn pre_cancelled_preview_publishes_nothing() {
        let input = tempfile::NamedTempFile::with_suffix(".jpg").unwrap();
        jpeg_fixture(input.path());
        let cancellation = CancellationToken::new();
        cancellation.cancel();
        assert!(develop_preview(input.path(), DevelopSettings::default(), &cancellation).is_err());
    }
}
