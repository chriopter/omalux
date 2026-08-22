use std::path::{Path, PathBuf};
use std::process::Command;

pub(super) fn is_qt_image(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| {
            matches!(
                extension.to_ascii_lowercase().as_str(),
                "jpg" | "jpeg" | "png" | "bmp"
            )
        })
        .unwrap_or(false)
}

pub(super) fn develop_raw_preview(path: &Path, generation: u64) -> Result<PathBuf, String> {
    let output_path = std::env::temp_dir().join(format!(
        "grainroom-preview-{}-{generation}.ppm",
        std::process::id()
    ));

    let output = Command::new("dcraw_emu")
        .args(raw_preview_arguments())
        .arg(path)
        .output()
        .map_err(|error| format!("Could not start LibRaw: {error}"))?;

    if !output.status.success() {
        let details = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Could not decode RAW: {}", details.trim()));
    }

    std::fs::write(&output_path, output.stdout)
        .map_err(|error| format!("Could not cache RAW preview: {error}"))?;
    Ok(output_path)
}

fn raw_preview_arguments() -> [&'static str; 8] {
    // Intentionally omit `-t`: LibRaw applies camera orientation exactly once.
    // The resulting PPM has no EXIF orientation for Qt to apply again.
    ["-w", "-h", "-o", "1", "-q", "0", "-Z", "-"]
}

#[cfg(test)]
mod tests {
    use super::{is_qt_image, raw_preview_arguments};
    use std::path::Path;

    #[test]
    fn routes_standard_images_directly_to_qt() {
        assert!(is_qt_image(Path::new("portrait.JPG")));
        assert!(is_qt_image(Path::new("scan.png")));
        assert!(is_qt_image(Path::new("reference.bmp")));
    }

    #[test]
    fn routes_camera_raw_files_to_libraw() {
        assert!(!is_qt_image(Path::new("capture.dng")));
        assert!(!is_qt_image(Path::new("capture.nef")));
        assert!(!is_qt_image(Path::new("capture.cr3")));
    }

    #[test]
    fn raw_preview_delegates_orientation_to_libraw_exactly_once() {
        let arguments = raw_preview_arguments();
        assert!(!arguments.contains(&"-t"));
        assert!(!arguments.contains(&"-j"));
    }
}
