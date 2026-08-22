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
        #[qproperty(QString, original_format, cxx_name = "originalFormat")]
        #[qproperty(QString, original_file_size, cxx_name = "originalFileSize")]
        #[qproperty(QString, metadata_text, cxx_name = "metadataText")]
        #[qproperty(QString, theme_background, cxx_name = "themeBackground")]
        #[qproperty(QString, theme_foreground, cxx_name = "themeForeground")]
        #[qproperty(QString, theme_accent, cxx_name = "themeAccent")]
        #[qproperty(QString, theme_selection, cxx_name = "themeSelection")]
        #[qproperty(QString, status)]
        #[qproperty(bool, loading)]
        type PhotoBackend = super::PhotoBackendRust;

        #[qinvokable]
        #[cxx_name = "openPhoto"]
        fn open_photo(self: Pin<&mut Self>, url: &QUrl);

        #[qinvokable]
        #[cxx_name = "saveOriginal"]
        fn save_original(self: Pin<&mut Self>, destination: &QUrl);

        #[qinvokable]
        #[cxx_name = "exportRendered"]
        fn export_rendered(
            self: Pin<&mut Self>,
            temporary_png: &QString,
            destination: &QUrl,
            format: &QString,
            quality: i32,
            width: i32,
            height: i32,
        );

        #[qinvokable]
        #[cxx_name = "reportExportError"]
        fn report_export_error(self: Pin<&mut Self>, message: &QString);

        #[qinvokable]
        #[cxx_name = "startThemeWatcher"]
        fn start_theme_watcher(self: Pin<&mut Self>);
    }

    impl cxx_qt::Threading for PhotoBackend {}
}

use core::pin::Pin;
use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::{QString, QUrl};
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

#[derive(Clone, Debug, PartialEq, Eq)]
struct ThemeColors {
    background: String,
    foreground: String,
    accent: String,
    selection: String,
}

impl Default for ThemeColors {
    fn default() -> Self {
        Self {
            background: "#101010".to_owned(),
            foreground: "#eeeeee".to_owned(),
            accent: "#5584aa".to_owned(),
            selection: "#263746".to_owned(),
        }
    }
}

pub struct PhotoBackendRust {
    preview_url: QUrl,
    file_name: QString,
    original_format: QString,
    original_file_size: QString,
    metadata_text: QString,
    theme_background: QString,
    theme_foreground: QString,
    theme_accent: QString,
    theme_selection: QString,
    status: QString,
    loading: bool,
    generation: AtomicU64,
    generated_preview: Option<PathBuf>,
    source_path: Option<PathBuf>,
    theme_watcher: Option<RecommendedWatcher>,
}

impl Default for PhotoBackendRust {
    fn default() -> Self {
        let theme = load_omarchy_theme();
        Self {
            preview_url: QUrl::default(),
            file_name: QString::default(),
            original_format: QString::from("—"),
            original_file_size: QString::from("—"),
            metadata_text: QString::from("NO PHOTOGRAPH LOADED"),
            theme_background: QString::from(&theme.background),
            theme_foreground: QString::from(&theme.foreground),
            theme_accent: QString::from(&theme.accent),
            theme_selection: QString::from(&theme.selection),
            status: QString::from("Open a JPEG or RAW photograph"),
            loading: false,
            generation: AtomicU64::new(0),
            generated_preview: None,
            source_path: None,
            theme_watcher: None,
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
        let original_format = path
            .extension()
            .and_then(|extension| extension.to_str())
            .unwrap_or("FILE")
            .to_ascii_uppercase();
        self.as_mut()
            .set_original_format(QString::from(&original_format));
        let original_file_size = std::fs::metadata(&path)
            .map(|metadata| human_file_size(metadata.len()))
            .unwrap_or_else(|_| "—".to_owned());
        self.as_mut()
            .set_original_file_size(QString::from(&original_file_size));
        self.as_mut()
            .set_metadata_text(QString::from(&read_metadata(&path)));
        self.as_mut().rust_mut().source_path = Some(path.clone());

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

    pub fn save_original(mut self: Pin<&mut Self>, destination: &QUrl) {
        let Some(source) = self.rust().source_path.clone() else {
            self.as_mut()
                .set_status(QString::from("Open a photograph before saving"));
            return;
        };
        let Some(destination) = local_destination(destination) else {
            self.as_mut().set_status(QString::from(
                "Only local export destinations are supported",
            ));
            return;
        };

        match std::fs::copy(&source, &destination) {
            Ok(_) => self.as_mut().set_status(QString::from(&format!(
                "Saved original · {}",
                display_file_name(&destination)
            ))),
            Err(error) => self
                .as_mut()
                .set_status(QString::from(&format!("Could not save original: {error}"))),
        }
    }

    pub fn export_rendered(
        mut self: Pin<&mut Self>,
        temporary_png: &QString,
        destination: &QUrl,
        format: &QString,
        quality: i32,
        width: i32,
        height: i32,
    ) {
        let temporary_png = PathBuf::from(temporary_png.to_string());
        let Some(destination) = local_destination(destination) else {
            let _ = std::fs::remove_file(&temporary_png);
            self.as_mut().set_status(QString::from(
                "Only local export destinations are supported",
            ));
            return;
        };
        let format = format.to_string().to_ascii_uppercase();
        if !matches!(format.as_str(), "JPEG" | "HEIC") {
            let _ = std::fs::remove_file(&temporary_png);
            self.as_mut()
                .set_status(QString::from("Unsupported export format"));
            return;
        }

        self.as_mut()
            .set_status(QString::from(&format!("Encoding {format}…")));
        let quality = quality.clamp(1, 100);
        let result = encode_rendered_export(
            &temporary_png,
            &destination,
            &format,
            quality,
            width,
            height,
        );
        let _ = std::fs::remove_file(&temporary_png);

        match result {
            Ok(()) => self.as_mut().set_status(QString::from(&format!(
                "Saved {format} · {}",
                display_file_name(&destination)
            ))),
            Err(message) => self.as_mut().set_status(QString::from(&message)),
        }
    }

    pub fn report_export_error(mut self: Pin<&mut Self>, message: &QString) {
        self.as_mut().set_status(message.clone());
    }

    pub fn start_theme_watcher(mut self: Pin<&mut Self>) {
        self.as_mut().apply_theme(load_omarchy_theme());

        let Some(current_theme) = omarchy_current_theme_path() else {
            return;
        };
        let pending = Arc::new(AtomicBool::new(false));
        let pending_from_event = Arc::clone(&pending);
        let qt_thread = self.qt_thread();
        let watcher = notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
            if event.is_err() || pending_from_event.swap(true, Ordering::AcqRel) {
                return;
            }
            let pending_after_update = Arc::clone(&pending_from_event);
            if qt_thread
                .queue(move |mut backend| {
                    // Theme switches replace Omarchy's `current` symlink, so
                    // rebuild the watches after every relevant filesystem event.
                    pending_after_update.store(false, Ordering::Release);
                    backend.as_mut().start_theme_watcher();
                })
                .is_err()
            {
                pending_from_event.store(false, Ordering::Release);
            }
        });

        let Ok(mut watcher) = watcher else {
            return;
        };
        let omarchy_state = current_theme
            .parent()
            .and_then(Path::parent)
            .map(Path::to_path_buf);
        let watches_state = omarchy_state
            .as_ref()
            .is_some_and(|path| watcher.watch(path, RecursiveMode::NonRecursive).is_ok());
        let watches_theme = watcher
            .watch(&current_theme, RecursiveMode::Recursive)
            .is_ok();
        if watches_state || watches_theme {
            self.as_mut().rust_mut().theme_watcher = Some(watcher);
        }
    }

    fn apply_theme(mut self: Pin<&mut Self>, theme: ThemeColors) {
        if self.theme_background().to_string() != theme.background {
            self.as_mut()
                .set_theme_background(QString::from(&theme.background));
        }
        if self.theme_foreground().to_string() != theme.foreground {
            self.as_mut()
                .set_theme_foreground(QString::from(&theme.foreground));
        }
        if self.theme_accent().to_string() != theme.accent {
            self.as_mut().set_theme_accent(QString::from(&theme.accent));
        }
        if self.theme_selection().to_string() != theme.selection {
            self.as_mut()
                .set_theme_selection(QString::from(&theme.selection));
        }
    }
}

fn omarchy_current_theme_path() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .map(|home| PathBuf::from(home).join(".local/state/omarchy/current/theme"))
}

fn load_omarchy_theme() -> ThemeColors {
    let fallback = ThemeColors::default();
    let Some(path) = omarchy_current_theme_path() else {
        return fallback;
    };
    let Ok(contents) = std::fs::read_to_string(path.join("colors.toml")) else {
        return fallback;
    };
    parse_theme_colors(&contents, fallback)
}

fn parse_theme_colors(contents: &str, mut colors: ThemeColors) -> ThemeColors {
    for line in contents.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let value = value
            .trim()
            .trim_matches(|character| character == '\"' || character == '\'')
            .to_owned();
        if value.is_empty() {
            continue;
        }
        match key.trim() {
            "background" => colors.background = value,
            "foreground" => colors.foreground = value,
            "accent" => colors.accent = value,
            "selection" => colors.selection = value,
            _ => {}
        }
    }
    colors
}

fn local_destination(url: &QUrl) -> Option<PathBuf> {
    url.to_local_file()
        .map(|path| PathBuf::from(path.to_string()))
}

fn display_file_name(path: &Path) -> &str {
    path.file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("photograph")
}

fn encode_rendered_export(
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
    use super::{ThemeColors, human_file_size, is_qt_image, metadata_field, parse_theme_colors};
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

    #[test]
    fn parses_the_omarchy_color_contract() {
        let colors = parse_theme_colors(
            "mode = 'dark'\nbackground = '#111c18'\nforeground = \"#C1C497\"\naccent = '#509475'\nselection = '#32473B'\n",
            ThemeColors::default(),
        );
        assert_eq!(colors.background, "#111c18");
        assert_eq!(colors.foreground, "#C1C497");
        assert_eq!(colors.accent, "#509475");
        assert_eq!(colors.selection, "#32473B");
    }
}
