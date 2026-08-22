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
        .args(["-w", "-h", "-o", "1", "-q", "0", "-Z", "-"])
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

#[cfg(test)]
mod tests {
    use super::is_qt_image;
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
}
