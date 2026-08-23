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
mod export;
mod loader;
mod metadata;
mod theme;

use self::export::{display_file_name, encode_rendered_export, local_destination};
use self::loader::{develop_raw_preview, is_qt_image};
use self::metadata::{human_file_size, read_metadata};
use self::theme::{ThemeColors, load_omarchy_theme, omarchy_current_theme_path};
use cxx_qt_lib::{QString, QUrl};
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

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
        let generation = self.rust().generation.fetch_add(1, Ordering::SeqCst) + 1;
        self.as_mut()
            .set_metadata_text(QString::from("READING METADATA…"));
        self.as_mut().rust_mut().source_path = Some(path.clone());
        let metadata_path = path.clone();
        let metadata_thread = self.qt_thread();
        std::thread::spawn(move || {
            let metadata = read_metadata(&metadata_path);
            let _ = metadata_thread.queue(move |mut backend| {
                if backend.rust().generation.load(Ordering::SeqCst) == generation {
                    backend.as_mut().set_metadata_text(QString::from(&metadata));
                }
            });
        });

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

        self.as_mut().set_status(QString::from("Saving original…"));
        let qt_thread = self.qt_thread();
        std::thread::spawn(move || {
            let result = std::fs::copy(&source, &destination);
            let _ = qt_thread.queue(move |mut backend| match result {
                Ok(_) => backend.as_mut().set_status(QString::from(&format!(
                    "Saved original · {}",
                    display_file_name(&destination)
                ))),
                Err(error) => backend
                    .as_mut()
                    .set_status(QString::from(&format!("Could not save original: {error}"))),
            });
        });
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
        let qt_thread = self.qt_thread();
        std::thread::spawn(move || {
            let result = encode_rendered_export(
                &temporary_png,
                &destination,
                &format,
                quality,
                width,
                height,
            );
            let _ = std::fs::remove_file(&temporary_png);
            let _ = qt_thread.queue(move |mut backend| match result {
                Ok(()) => backend.as_mut().set_status(QString::from(&format!(
                    "Saved {format} · {}",
                    display_file_name(&destination)
                ))),
                Err(message) => backend.as_mut().set_status(QString::from(&message)),
            });
        });
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
