use cxx_qt_lib::QUrl;
use std::path::{Path, PathBuf};
use std::process::Command;

pub(super) fn local_destination(url: &QUrl) -> Option<PathBuf> {
    url.to_local_file()
        .map(|path| PathBuf::from(path.to_string()))
}

pub(super) fn display_file_name(path: &Path) -> &str {
    path.file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("photograph")
}

pub(super) fn encode_rendered_export(
    temporary_png: &Path,
    destination: &Path,
    format: &str,
    quality: i32,
    width: i32,
    height: i32,
) -> Result<(), String> {
    if width < 1 || height < 1 {
        return Err("Could not encode export: invalid image dimensions".to_owned());
    }
    let dimensions = format!("{width}x{height}!");
    let output = Command::new("magick")
        .arg(temporary_png)
        .args(["-background", "black", "-alpha", "remove", "-alpha", "off"])
        .args(["-resize", &dimensions])
        .args(["-quality", &quality.to_string()])
        .arg(destination)
        .output()
        .map_err(|error| format!("Could not start image encoder: {error}"))?;

    if output.status.success() {
        Ok(())
    } else {
        let details = String::from_utf8_lossy(&output.stderr);
        Err(format!("Could not encode {format}: {}", details.trim()))
    }
}
