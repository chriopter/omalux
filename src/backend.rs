#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;

        include!("cxx-qt-lib/qurl.h");
        type QUrl = cxx_qt_lib::QUrl;
    }

    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qproperty(QUrl, preview_url, cxx_name = "previewUrl")]
        #[qproperty(QString, file_name, cxx_name = "fileName")]
        #[qproperty(QString, metadata_text, cxx_name = "metadataText")]
        #[qproperty(QString, status)]
        #[qproperty(bool, loading)]
        type PhotoBackend = super::PhotoBackendRust;

        #[qinvokable]
        #[cxx_name = "openPhoto"]
        fn open_photo(self: Pin<&mut Self>, url: &QUrl);
    }

    impl cxx_qt::Threading for PhotoBackend {}
}

use core::pin::Pin;
use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::{QString, QUrl};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

pub struct PhotoBackendRust {
    preview_url: QUrl,
    file_name: QString,
    metadata_text: QString,
    status: QString,
    loading: bool,
    generation: AtomicU64,
    generated_preview: Option<PathBuf>,
}

impl Default for PhotoBackendRust {
    fn default() -> Self {
        Self {
            preview_url: QUrl::default(),
            file_name: QString::default(),
            metadata_text: QString::from("NO PHOTOGRAPH LOADED"),
            status: QString::from("Open a JPEG or RAW photograph"),
            loading: false,
            generation: AtomicU64::new(0),
            generated_preview: None,
        }
    }
}

impl Drop for PhotoBackendRust {
    fn drop(&mut self) {
        if let Some(path) = self.generated_preview.take() {
            let _ = std::fs::remove_file(path);
        }
    }
}

impl qobject::PhotoBackend {
    pub fn open_photo(mut self: Pin<&mut Self>, url: &QUrl) {
        let Some(local_file) = url.to_local_file() else {
            self.as_mut()
                .set_status(QString::from("Only local files are supported"));
            return;
        };

        let path = PathBuf::from(local_file.to_string());
        if !path.is_file() {
            self.as_mut()
                .set_status(QString::from("The selected file does not exist"));
            return;
        }

        let file_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("Photograph")
            .to_owned();
        self.as_mut().set_file_name(QString::from(&file_name));
        self.as_mut()
            .set_metadata_text(QString::from(&read_metadata(&path)));

        let generation = self.rust().generation.fetch_add(1, Ordering::SeqCst) + 1;

        if is_qt_image(&path) {
            if let Some(old_preview) = self.as_mut().rust_mut().generated_preview.take() {
                let _ = std::fs::remove_file(old_preview);
            }
            self.as_mut().set_preview_url(url.clone());
            self.as_mut().set_loading(false);
            self.as_mut().set_status(QString::from("Ready"));
            return;
        }

        self.as_mut().set_loading(true);
        self.as_mut()
            .set_status(QString::from("Developing RAW preview…"));
        let qt_thread = self.qt_thread();

        std::thread::spawn(move || {
            let result = develop_raw_preview(&path, generation);
            let _ = qt_thread.queue(move |mut backend| {
                if backend.rust().generation.load(Ordering::SeqCst) != generation {
                    if let Ok(path) = result {
                        let _ = std::fs::remove_file(path);
                    }
                    return;
                }

                backend.as_mut().set_loading(false);
                match result {
                    Ok(path) => {
                        if let Some(old_preview) = backend
                            .as_mut()
                            .rust_mut()
                            .generated_preview
                            .replace(path.clone())
                        {
                            let _ = std::fs::remove_file(old_preview);
                        }
                        let preview_url =
                            QUrl::from_local_file(&QString::from(path.to_string_lossy().as_ref()));
                        backend.as_mut().set_preview_url(preview_url);
                        backend
                            .as_mut()
                            .set_status(QString::from("Ready · LibRaw preview"));
                    }
                    Err(message) => {
                        backend.as_mut().set_status(QString::from(&message));
                    }
                }
            });
        });
    }
}

fn is_qt_image(path: &Path) -> bool {
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

fn read_metadata(path: &Path) -> String {
    let mut rows = Vec::new();
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("UNKNOWN")
        .to_ascii_uppercase();
    push_metadata(&mut rows, "FORMAT", &extension);

    if let Ok(metadata) = std::fs::metadata(path) {
        push_metadata(&mut rows, "FILE SIZE", &human_file_size(metadata.len()));
    }

    if is_qt_image(path) {
        read_imagemagick_metadata(path, &mut rows);
    } else {
        read_raw_metadata(path, &mut rows);
    }

    rows.join("\n")
}

fn read_imagemagick_metadata(path: &Path, rows: &mut Vec<String>) {
    const FORMAT: &str = "%m\x1f%wx%h\x1f%[EXIF:Make]\x1f%[EXIF:Model]\x1f%[EXIF:LensModel]\x1f%[EXIF:DateTimeOriginal]\x1f%[EXIF:ExposureTime]\x1f%[EXIF:FNumber]\x1f%[EXIF:ISOSpeedRatings]\x1f%[EXIF:FocalLength]";
    let Ok(output) = Command::new("magick")
        .args(["identify", "-quiet", "-format", FORMAT])
        .arg(path)
        .output()
    else {
        return;
    };
    if !output.status.success() {
        return;
    }

    let output = String::from_utf8_lossy(&output.stdout);
    let values: Vec<&str> = output.split('\x1f').collect();
    if values.len() < 10 {
        return;
    }

    push_metadata(rows, "CODEC", values[0]);
    push_metadata(rows, "DIMENSIONS", values[1]);
    let camera = [clean_metadata(values[2]), clean_metadata(values[3])]
        .into_iter()
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    push_metadata(rows, "CAMERA", &camera);
    push_metadata(rows, "LENS", &clean_metadata(values[4]));
    push_metadata(rows, "CAPTURED", &clean_metadata(values[5]));
    push_metadata(rows, "SHUTTER", &clean_metadata(values[6]));
    push_metadata(rows, "APERTURE", &clean_metadata(values[7]));
    push_metadata(rows, "ISO", &clean_metadata(values[8]));
    push_metadata(rows, "FOCAL LEN", &clean_metadata(values[9]));
}

fn read_raw_metadata(path: &Path, rows: &mut Vec<String>) {
    let Ok(output) = Command::new("dcraw_emu")
        .args(["-i", "-v"])
        .arg(path)
        .output()
    else {
        return;
    };
    if !output.status.success() {
        return;
    }

    let output = String::from_utf8_lossy(&output.stdout);
    for (label, field) in [
        ("CAMERA", "Camera"),
        ("CAPTURED", "Timestamp"),
        ("ISO", "ISO speed"),
        ("SHUTTER", "Shutter"),
        ("APERTURE", "Aperture"),
        ("FOCAL LEN", "Focal length"),
        ("DIMENSIONS", "Image size"),
    ] {
        if let Some(value) = metadata_field(&output, field) {
            push_metadata(rows, label, value);
        }
    }
}

fn metadata_field<'a>(output: &'a str, name: &str) -> Option<&'a str> {
    output.lines().find_map(|line| {
        let (key, value) = line.split_once(':')?;
        (key.trim() == name).then_some(value.trim())
    })
}

fn clean_metadata(value: &str) -> String {
    value
        .trim()
        .trim_matches('"')
        .replace("Undefined", "")
        .trim()
        .to_owned()
}

fn push_metadata(rows: &mut Vec<String>, label: &str, value: &str) {
    let value = value.trim();
    if !value.is_empty() {
        rows.push(format!("{label:<12} {value}"));
    }
}

fn human_file_size(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["B", "KB", "MB", "GB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{} {}", bytes, UNITS[unit])
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

fn develop_raw_preview(path: &Path, generation: u64) -> Result<PathBuf, String> {
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
    use super::{human_file_size, is_qt_image, metadata_field};
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
    fn formats_file_sizes_for_metadata_panel() {
        assert_eq!(human_file_size(512), "512 B");
        assert_eq!(human_file_size(1_572_864), "1.5 MB");
    }

    #[test]
    fn extracts_libraw_metadata_fields() {
        let output = "Camera: Fujifilm X-T5\nISO speed: 800\nShutter: 1/125 sec\n";
        assert_eq!(metadata_field(output, "Camera"), Some("Fujifilm X-T5"));
        assert_eq!(metadata_field(output, "ISO speed"), Some("800"));
    }
}
