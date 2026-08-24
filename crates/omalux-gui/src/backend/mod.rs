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
        #[qproperty(QString, preset_catalog_json, cxx_name = "presetCatalogJson")]
        #[qproperty(QString, selected_preset_id, cxx_name = "selectedPresetId")]
        #[qproperty(QString, settings_json, cxx_name = "settingsJson")]
        #[qproperty(
            QString,
            supported_parameters_json,
            cxx_name = "supportedParametersJson"
        )]
        #[qproperty(QString, theme_background, cxx_name = "themeBackground")]
        #[qproperty(QString, theme_foreground, cxx_name = "themeForeground")]
        #[qproperty(QString, theme_accent, cxx_name = "themeAccent")]
        #[qproperty(QString, theme_selection, cxx_name = "themeSelection")]
        #[qproperty(QString, status)]
        #[qproperty(QString, operation_history_json, cxx_name = "operationHistoryJson")]
        #[qproperty(QString, last_job_report_json, cxx_name = "lastJobReportJson")]
        #[qproperty(QString, last_preview_report_json, cxx_name = "lastPreviewReportJson")]
        #[qproperty(bool, loading)]
        type PhotoBackend = super::PhotoBackendRust;

        #[qinvokable]
        #[cxx_name = "openPhoto"]
        fn open_photo(self: Pin<&mut Self>, url: &QUrl);

        #[qinvokable]
        #[cxx_name = "urlForLocalPath"]
        fn url_for_local_path(self: Pin<&mut Self>, path: &QString) -> QUrl;

        #[qinvokable]
        #[cxx_name = "cancelDevelop"]
        fn cancel_develop(self: Pin<&mut Self>);

        #[qinvokable]
        #[cxx_name = "selectPreset"]
        fn select_preset(self: Pin<&mut Self>, id: &QString);

        #[qinvokable]
        #[cxx_name = "setParameter"]
        fn set_parameter(self: Pin<&mut Self>, id: &QString, value: f64);

        #[qinvokable]
        #[cxx_name = "saveOriginal"]
        fn save_original(self: Pin<&mut Self>, destination: &QUrl);

        #[qinvokable]
        #[cxx_name = "exportPhoto"]
        fn export_photo(self: Pin<&mut Self>, destination: &QUrl, format: &QString, quality: i32);

        #[qinvokable]
        #[cxx_name = "startThemeWatcher"]
        fn start_theme_watcher(self: Pin<&mut Self>);

        #[qinvokable]
        #[cxx_name = "reloadTheme"]
        fn reload_theme(self: Pin<&mut Self>);
    }

    impl cxx_qt::Threading for PhotoBackend {}

    unsafe extern "C++" {
        include!("theme_watcher.h");

        #[cxx_name = "installThemeWatcher"]
        fn install_theme_watcher(backend: Pin<&mut PhotoBackend>);
    }
}

use core::pin::Pin;
use cxx_qt::{CxxQtType, Threading};
mod develop;
mod export;
mod loader;
mod metadata;
mod theme;

use self::develop::{
    GuiJobError, PreviewArtifact, built_in_catalog_json, built_in_settings, develop_preview,
    export_photo, save_original_atomic, settings_json, supported_parameters_json,
};
use self::export::{absolute_local_path, display_file_name, local_destination};
use self::metadata::{human_file_size, read_metadata};
use self::theme::{ThemeColors, load_omarchy_theme};
use cxx_qt_lib::{QString, QUrl};
use omalux::develop::{DevelopSettings, apply_parameter_overrides, parse_parameter_override};
use omalux::io::OutputFormat;
use omalux::job::{CancellationToken, DevelopJobOutcome, DevelopJobReport};
use serde::Serialize;
use std::sync::atomic::{AtomicU64, Ordering};
use std::{
    collections::VecDeque,
    path::{Path, PathBuf},
};

pub struct PhotoBackendRust {
    preview_url: QUrl,
    file_name: QString,
    original_format: QString,
    original_file_size: QString,
    metadata_text: QString,
    preset_catalog_json: QString,
    selected_preset_id: QString,
    settings_json: QString,
    supported_parameters_json: QString,
    theme_background: QString,
    theme_foreground: QString,
    theme_accent: QString,
    theme_selection: QString,
    status: QString,
    operation_history_json: QString,
    last_job_report_json: QString,
    last_preview_report_json: QString,
    loading: bool,
    metadata_revision: AtomicU64,
    preview_revision: AtomicU64,
    operations: OperationTracker,
    generated_preview: Option<PreviewArtifact>,
    source_path: Option<PathBuf>,
    settings: DevelopSettings,
    preview_queue: PreviewQueue,
    preview_cancellation: Option<CancellationToken>,
    export_cancellation: Option<CancellationToken>,
}

struct PreviewRequest {
    revision: u64,
    operation_revision: u64,
    source: PathBuf,
    settings: DevelopSettings,
    /// Interactive requests develop a fast proxy; a refinement pass re-runs
    /// the same settings at full resolution once the queue is idle.
    full_resolution: bool,
}

#[derive(Default)]
struct PreviewQueue {
    worker_active: bool,
    pending: Option<PreviewRequest>,
}

impl PreviewQueue {
    fn enqueue(&mut self, request: PreviewRequest) -> Option<PreviewRequest> {
        self.pending = Some(request);
        self.take_if_idle()
    }

    fn worker_completed(&mut self) -> Option<PreviewRequest> {
        self.worker_active = false;
        self.take_if_idle()
    }

    fn cancel_pending(&mut self) {
        self.pending = None;
    }

    fn take_if_idle(&mut self) -> Option<PreviewRequest> {
        if self.worker_active {
            return None;
        }
        let next = self.pending.take()?;
        self.worker_active = true;
        Some(next)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum OperationKind {
    Preview,
    Export,
    SaveOriginal,
}

#[derive(Debug, Serialize)]
struct OperationRecord {
    revision: u64,
    kind: OperationKind,
    outcome: &'static str,
    warning: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    report: Option<serde_json::Value>,
}

#[derive(Default)]
struct OperationTracker {
    current: u64,
    history: VecDeque<OperationRecord>,
}

impl OperationTracker {
    const HISTORY_LIMIT: usize = 32;

    fn begin(&mut self) -> u64 {
        self.current = self
            .current
            .checked_add(1)
            .expect("GUI operation revision exhausted");
        self.current
    }

    fn complete(&mut self, record: OperationRecord) -> (bool, String) {
        let updates_main_status = record.revision == self.current;
        if self.history.len() == Self::HISTORY_LIMIT {
            self.history.pop_front();
        }
        self.history.push_back(record);
        let json = serde_json::to_string(&self.history).unwrap_or_else(|_| "[]".to_owned());
        (updates_main_status, json)
    }
}

fn export_completion(
    result: Result<DevelopJobReport, GuiJobError>,
    format: &str,
    destination: &str,
) -> (&'static str, bool, String, Option<serde_json::Value>) {
    match result {
        Ok(report) => {
            let serialized = serde_json::to_value(&report).ok();
            match &report.outcome {
                DevelopJobOutcome::PublishedAndDurable { .. } => (
                    "published_and_durable",
                    false,
                    format!("Saved {format} · {destination}"),
                    serialized,
                ),
                DevelopJobOutcome::PublishedButNotDurable { .. } => (
                    "published_but_not_durable",
                    true,
                    format!(
                        "Published with durability warning: directory sync not confirmed · {destination}"
                    ),
                    serialized,
                ),
                DevelopJobOutcome::Failure { .. } => (
                    "failure",
                    false,
                    "Development reported a failed outcome".to_owned(),
                    serialized,
                ),
            }
        }
        Err(error) => {
            let report = match &error {
                GuiJobError::Develop(failure) => serde_json::to_value(failure.report.as_ref()).ok(),
                GuiJobError::Setup(_) => None,
            };
            ("failure", false, error.to_string(), report)
        }
    }
}

fn preview_completion(
    result: &Result<PreviewArtifact, GuiJobError>,
) -> (&'static str, bool, String, Option<serde_json::Value>) {
    match result {
        Ok(artifact) => {
            let report = artifact.report();
            let serialized = serde_json::to_value(report).ok();
            match &report.outcome {
                DevelopJobOutcome::PublishedAndDurable { .. } => (
                    "published_and_durable",
                    false,
                    "Ready · CPU developed preview".to_owned(),
                    serialized,
                ),
                DevelopJobOutcome::PublishedButNotDurable { .. } => (
                    "published_but_not_durable",
                    true,
                    "Preview ready with durability warning: directory sync not confirmed"
                        .to_owned(),
                    serialized,
                ),
                DevelopJobOutcome::Failure { .. } => (
                    "failure",
                    false,
                    "Preview development reported a failed outcome".to_owned(),
                    serialized,
                ),
            }
        }
        Err(error) => {
            let report = match error {
                GuiJobError::Develop(failure) => serde_json::to_value(failure.report.as_ref()).ok(),
                GuiJobError::Setup(_) => None,
            };
            ("failure", false, error.to_string(), report)
        }
    }
}

fn original_completion(
    result: Result<omalux::io::AtomicOutputOutcome, String>,
    destination: &str,
) -> (&'static str, bool, String) {
    match result {
        Ok(omalux::io::AtomicOutputOutcome::PublishedAndDurable) => (
            "published_and_durable",
            false,
            format!("Saved original · {destination}"),
        ),
        Ok(omalux::io::AtomicOutputOutcome::PublishedButNotDurable) => (
            "published_but_not_durable",
            true,
            format!(
                "Published with durability warning: directory sync not confirmed · {destination}"
            ),
        ),
        Ok(_) => (
            "published",
            false,
            format!("Saved original · {destination}"),
        ),
        Err(error) => (
            "failure",
            false,
            format!("Could not save original: {error}"),
        ),
    }
}

impl Default for PhotoBackendRust {
    fn default() -> Self {
        let theme = load_omarchy_theme();
        let settings = built_in_settings("neutral").unwrap_or_default();
        Self {
            preview_url: QUrl::default(),
            file_name: QString::default(),
            original_format: QString::from("—"),
            original_file_size: QString::from("—"),
            metadata_text: QString::from("NO PHOTOGRAPH LOADED"),
            preset_catalog_json: QString::from(
                &built_in_catalog_json().unwrap_or_else(|_| "{\"presets\":[]}".to_owned()),
            ),
            selected_preset_id: QString::from("neutral"),
            settings_json: QString::from(
                &settings_json(&settings).unwrap_or_else(|_| "{}".to_owned()),
            ),
            supported_parameters_json: QString::from(
                &supported_parameters_json().unwrap_or_else(|_| "[]".to_owned()),
            ),
            theme_background: QString::from(&theme.background),
            theme_foreground: QString::from(&theme.foreground),
            theme_accent: QString::from(&theme.accent),
            theme_selection: QString::from(&theme.selection),
            status: QString::from("Open a JPEG or RAW photograph"),
            operation_history_json: QString::from("[]"),
            last_job_report_json: QString::from("{}"),
            last_preview_report_json: QString::from("{}"),
            loading: false,
            metadata_revision: AtomicU64::new(0),
            preview_revision: AtomicU64::new(0),
            operations: OperationTracker::default(),
            generated_preview: None,
            source_path: None,
            settings,
            preview_queue: PreviewQueue::default(),
            preview_cancellation: None,
            export_cancellation: None,
        }
    }
}

impl Drop for PhotoBackendRust {
    fn drop(&mut self) {
        if let Some(cancellation) = self.preview_cancellation.take() {
            cancellation.cancel();
        }
        if let Some(cancellation) = self.export_cancellation.take() {
            cancellation.cancel();
        }
    }
}

impl qobject::PhotoBackend {
    pub fn url_for_local_path(self: Pin<&mut Self>, path: &QString) -> QUrl {
        let path = absolute_local_path(Path::new(&path.to_string()));
        QUrl::from_local_file(&QString::from(path.to_string_lossy().as_ref()))
    }

    pub fn open_photo(mut self: Pin<&mut Self>, url: &QUrl) {
        let Some(local_file) = url.to_local_file() else {
            self.as_mut()
                .set_status(QString::from("Only local files are supported"));
            return;
        };

        let path = absolute_local_path(Path::new(&local_file.to_string()));
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
        let metadata_revision = self.rust().metadata_revision.fetch_add(1, Ordering::SeqCst) + 1;
        self.as_mut()
            .set_metadata_text(QString::from("READING METADATA…"));
        self.as_mut().rust_mut().source_path = Some(path.clone());
        let metadata_path = path.clone();
        let metadata_thread = self.qt_thread();
        std::thread::spawn(move || {
            let metadata = read_metadata(&metadata_path);
            let _ = metadata_thread.queue(move |mut backend| {
                if backend.rust().metadata_revision.load(Ordering::SeqCst) == metadata_revision {
                    backend.as_mut().set_metadata_text(QString::from(&metadata));
                }
            });
        });
        self.as_mut().request_preview();
    }

    pub fn cancel_develop(mut self: Pin<&mut Self>) {
        self.rust().preview_revision.fetch_add(1, Ordering::SeqCst);
        if let Some(cancellation) = self.as_mut().rust_mut().preview_cancellation.take() {
            cancellation.cancel();
        }
        self.as_mut().rust_mut().preview_queue.cancel_pending();
        self.as_mut().set_loading(false);
        let revision = self.as_mut().rust_mut().operations.begin();
        self.as_mut().finish_operation(
            OperationRecord {
                revision,
                kind: OperationKind::Preview,
                outcome: "cancelled",
                warning: false,
                report: None,
            },
            "Development cancelled".to_owned(),
        );
    }

    pub fn select_preset(mut self: Pin<&mut Self>, id: &QString) {
        let id = id.to_string();
        let Ok(settings) = built_in_settings(&id) else {
            self.as_mut()
                .set_status(QString::from("Unknown built-in preset"));
            return;
        };
        self.as_mut().rust_mut().settings = settings;
        self.as_mut().set_selected_preset_id(QString::from(&id));
        self.as_mut().publish_settings_json();
        self.as_mut().restart_preview();
    }

    pub fn set_parameter(mut self: Pin<&mut Self>, id: &QString, value: f64) {
        if !value.is_finite() {
            self.as_mut()
                .set_status(QString::from("Invalid parameter value"));
            return;
        }
        let id = id.to_string();
        let expression = format!("{id}={value}");
        let Ok(parameter) = parse_parameter_override(&expression) else {
            self.as_mut()
                .set_status(QString::from("Unknown or invalid parameter"));
            return;
        };
        let Ok(settings) = apply_parameter_overrides(&self.rust().settings, &[parameter]) else {
            self.as_mut()
                .set_status(QString::from("Parameter combination is invalid"));
            return;
        };
        self.as_mut().rust_mut().settings = settings;
        self.as_mut()
            .set_selected_preset_id(QString::from("custom"));
        self.as_mut().publish_settings_json();
        self.as_mut().restart_preview();
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

        let revision = self.as_mut().rust_mut().operations.begin();
        self.as_mut().set_status(QString::from("Saving original…"));
        let qt_thread = self.qt_thread();
        std::thread::spawn(move || {
            let result = save_original_atomic(&source, &destination);
            let _ = qt_thread.queue(move |mut backend| {
                let (outcome, warning, status) =
                    original_completion(result, display_file_name(&destination));
                backend.as_mut().finish_operation(
                    OperationRecord {
                        revision,
                        kind: OperationKind::SaveOriginal,
                        outcome,
                        warning,
                        report: None,
                    },
                    status,
                );
            });
        });
    }

    pub fn export_photo(
        mut self: Pin<&mut Self>,
        destination: &QUrl,
        format: &QString,
        quality: i32,
    ) {
        let Some(source) = self.rust().source_path.clone() else {
            self.as_mut()
                .set_status(QString::from("Open a photograph before exporting"));
            return;
        };
        let Some(destination) = local_destination(destination) else {
            self.as_mut().set_status(QString::from(
                "Only local export destinations are supported",
            ));
            return;
        };
        let format = format.to_string().to_ascii_uppercase();
        let output_format = match format.as_str() {
            "JPEG" | "JPG" => OutputFormat::Jpeg,
            "HEIC" | "HEIF" => OutputFormat::Heic,
            _ => {
                self.as_mut()
                    .set_status(QString::from("Unsupported export format"));
                return;
            }
        };

        if let Some(cancellation) = self.as_mut().rust_mut().export_cancellation.take() {
            cancellation.cancel();
        }
        let revision = self.as_mut().rust_mut().operations.begin();
        let cancellation = CancellationToken::new();
        self.as_mut().rust_mut().export_cancellation = Some(cancellation.clone());
        self.as_mut()
            .set_status(QString::from(&format!("Encoding {format}…")));
        let quality = quality.clamp(1, 100);
        let settings = self.rust().settings.clone();
        let qt_thread = self.qt_thread();
        std::thread::spawn(move || {
            let result = export_photo(
                &source,
                &destination,
                settings,
                output_format,
                quality as u8,
                &cancellation,
            );
            let _ = qt_thread.queue(move |mut backend| {
                let (outcome, warning, status, report) =
                    export_completion(result, &format, display_file_name(&destination));
                backend.as_mut().finish_operation(
                    OperationRecord {
                        revision,
                        kind: OperationKind::Export,
                        outcome,
                        warning,
                        report,
                    },
                    status,
                );
                if backend.rust().operations.current == revision {
                    backend.as_mut().rust_mut().export_cancellation = None;
                }
            });
        });
    }

    pub fn start_theme_watcher(mut self: Pin<&mut Self>) {
        self.as_mut().apply_theme(load_omarchy_theme());
        qobject::install_theme_watcher(self);
    }

    pub fn reload_theme(mut self: Pin<&mut Self>) {
        let theme = load_omarchy_theme();
        if std::env::var_os("OMALUX_THEME_WATCH_TRACE").is_some() {
            eprintln!("omalux-theme-reload:{}", theme.background);
        }
        self.as_mut().apply_theme(theme);
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

    fn publish_settings_json(mut self: Pin<&mut Self>) {
        if let Ok(json) = settings_json(&self.rust().settings) {
            self.as_mut().set_settings_json(QString::from(&json));
        }
    }

    fn restart_preview(mut self: Pin<&mut Self>) {
        if self.rust().source_path.is_none() {
            return;
        }
        self.as_mut().request_preview();
    }

    fn request_preview(mut self: Pin<&mut Self>) {
        let Some(source) = self.rust().source_path.clone() else {
            return;
        };
        let revision = self.rust().preview_revision.fetch_add(1, Ordering::SeqCst) + 1;
        let operation_revision = self.as_mut().rust_mut().operations.begin();
        if let Some(cancellation) = self.as_mut().rust_mut().preview_cancellation.take() {
            cancellation.cancel();
        }
        let request = PreviewRequest {
            revision,
            operation_revision,
            source,
            settings: self.rust().settings.clone(),
            full_resolution: false,
        };
        let next = self.as_mut().rust_mut().preview_queue.enqueue(request);
        self.as_mut().set_loading(true);
        self.as_mut()
            .set_status(QString::from("Developing CPU preview…"));
        if let Some(next) = next {
            self.as_mut().launch_preview(next);
        }
    }

    fn launch_preview(mut self: Pin<&mut Self>, request: PreviewRequest) {
        let cancellation = CancellationToken::new();
        self.as_mut().rust_mut().preview_cancellation = Some(cancellation.clone());
        let qt_thread = self.qt_thread();
        std::thread::spawn(move || {
            let result = develop_preview(
                &request.source,
                request.settings.clone(),
                request.full_resolution,
                &cancellation,
            );
            let _ = qt_thread.queue(move |mut backend| {
                backend.as_mut().rust_mut().preview_cancellation = None;
                let is_latest_preview =
                    backend.rust().preview_revision.load(Ordering::SeqCst) == request.revision;
                let (outcome, warning, status, report) = preview_completion(&result);
                if is_latest_preview {
                    backend.as_mut().set_loading(false);
                    if let Ok(artifact) = result {
                        if let Ok(report) = serde_json::to_string(artifact.report()) {
                            backend
                                .as_mut()
                                .set_last_preview_report_json(QString::from(&report));
                        }
                        let preview_url = QUrl::from_local_file(&QString::from(
                            artifact.path().to_string_lossy().as_ref(),
                        ));
                        backend.as_mut().rust_mut().generated_preview = Some(artifact);
                        backend.as_mut().set_preview_url(preview_url);
                    }
                }
                backend.as_mut().finish_operation(
                    OperationRecord {
                        revision: request.operation_revision,
                        kind: OperationKind::Preview,
                        outcome,
                        warning,
                        report,
                    },
                    status,
                );
                let mut next = backend.as_mut().rust_mut().preview_queue.worker_completed();
                if next.is_none() && !request.full_resolution && is_latest_preview {
                    // Idle after a proxy pass: refine the same settings at
                    // full resolution so zoomed inspection stays sharp.
                    next = backend
                        .as_mut()
                        .rust_mut()
                        .preview_queue
                        .enqueue(PreviewRequest {
                            full_resolution: true,
                            settings: request.settings,
                            ..request
                        });
                }
                if let Some(next) = next {
                    backend.as_mut().launch_preview(next);
                }
            });
        });
    }

    fn finish_operation(mut self: Pin<&mut Self>, record: OperationRecord, status: String) {
        let report = record.report.clone();
        let (updates_main_status, history) = self.as_mut().rust_mut().operations.complete(record);
        self.as_mut()
            .set_operation_history_json(QString::from(&history));
        if updates_main_status {
            if let Some(report) = report
                && let Ok(json) = serde_json::to_string(&report)
            {
                self.as_mut().set_last_job_report_json(QString::from(&json));
            }
            self.as_mut().set_status(QString::from(&status));
        }
    }
}

#[cfg(test)]
mod preview_queue_tests {
    use super::{
        OperationKind, OperationRecord, OperationTracker, PreviewQueue, PreviewRequest,
        export_completion, original_completion,
    };
    use crate::backend::develop::develop_preview;
    use omalux::develop::DevelopSettings;
    use omalux::{
        io::AtomicOutputOutcome,
        job::{CancellationToken, DevelopJobOutcome},
    };
    use std::path::PathBuf;

    fn request(revision: u64) -> PreviewRequest {
        PreviewRequest {
            revision,
            operation_revision: revision,
            source: PathBuf::from(format!("source-{revision}")),
            settings: DevelopSettings::default(),
            full_resolution: false,
        }
    }

    #[test]
    fn rapid_updates_keep_one_worker_and_only_the_latest_pending_revision() {
        let mut queue = PreviewQueue::default();
        assert_eq!(queue.enqueue(request(1)).unwrap().revision, 1);
        assert!(queue.worker_active);
        for revision in 2..=1_000 {
            assert!(queue.enqueue(request(revision)).is_none());
            assert!(queue.worker_active);
        }
        assert_eq!(queue.pending.as_ref().unwrap().revision, 1_000);
        assert_eq!(queue.worker_completed().unwrap().revision, 1_000);
        assert!(queue.worker_active);
        assert!(queue.worker_completed().is_none());
        assert!(!queue.worker_active);
    }

    fn record(revision: u64, kind: OperationKind) -> OperationRecord {
        OperationRecord {
            revision,
            kind,
            outcome: "published_and_durable",
            warning: false,
            report: None,
        }
    }

    fn complete(
        tracker: &mut OperationTracker,
        main_status: &mut &'static str,
        revision: u64,
        kind: OperationKind,
        status: &'static str,
    ) {
        let (updates_main, _) = tracker.complete(record(revision, kind));
        if updates_main {
            *main_status = status;
        }
    }

    #[test]
    fn e1_e2_out_of_order_keeps_e2_main_status_and_both_history_events() {
        let mut tracker = OperationTracker::default();
        let e1 = tracker.begin();
        let e2 = tracker.begin();
        let mut status = "running-e2";
        complete(&mut tracker, &mut status, e2, OperationKind::Export, "e2");
        complete(&mut tracker, &mut status, e1, OperationKind::Export, "e1");
        assert_eq!(status, "e2");
        assert_eq!(tracker.history.len(), 2);
    }

    #[test]
    fn s1_s2_out_of_order_keeps_s2_main_status_and_both_history_events() {
        let mut tracker = OperationTracker::default();
        let s1 = tracker.begin();
        let s2 = tracker.begin();
        let mut status = "running-s2";
        complete(
            &mut tracker,
            &mut status,
            s2,
            OperationKind::SaveOriginal,
            "s2",
        );
        complete(
            &mut tracker,
            &mut status,
            s1,
            OperationKind::SaveOriginal,
            "s1",
        );
        assert_eq!(status, "s2");
        assert_eq!(tracker.history.len(), 2);
    }

    #[test]
    fn cross_export_save_out_of_order_never_overwrites_newer_main_status() {
        let mut tracker = OperationTracker::default();
        let export = tracker.begin();
        let save = tracker.begin();
        let mut status = "saving";
        complete(
            &mut tracker,
            &mut status,
            save,
            OperationKind::SaveOriginal,
            "saved",
        );
        complete(
            &mut tracker,
            &mut status,
            export,
            OperationKind::Export,
            "exported",
        );
        assert_eq!(status, "saved");
        assert_eq!(tracker.history.len(), 2);
    }

    #[test]
    fn preview_completion_cannot_overwrite_a_newer_export_status() {
        let mut tracker = OperationTracker::default();
        let preview = tracker.begin();
        let export = tracker.begin();
        let mut status = "encoding";
        complete(
            &mut tracker,
            &mut status,
            export,
            OperationKind::Export,
            "exported",
        );
        complete(
            &mut tracker,
            &mut status,
            preview,
            OperationKind::Preview,
            "preview-ready",
        );
        assert_eq!(status, "exported");
        assert_eq!(tracker.history.len(), 2);
    }

    #[test]
    fn preview_started_after_save_owns_the_newer_main_status() {
        let mut tracker = OperationTracker::default();
        let save = tracker.begin();
        let preview = tracker.begin();
        let mut status = "developing-preview";
        complete(
            &mut tracker,
            &mut status,
            save,
            OperationKind::SaveOriginal,
            "saved",
        );
        assert_eq!(status, "developing-preview");
        complete(
            &mut tracker,
            &mut status,
            preview,
            OperationKind::Preview,
            "preview-ready",
        );
        assert_eq!(status, "preview-ready");
        assert_eq!(tracker.history.len(), 2);
    }

    #[test]
    fn published_but_not_durable_is_a_successful_warning_for_both_paths() {
        let (_, warning, status) =
            original_completion(Ok(AtomicOutputOutcome::PublishedButNotDurable), "copy.jpg");
        assert!(warning);
        assert!(status.starts_with("Published with durability warning:"));

        let input = tempfile::NamedTempFile::with_suffix(".jpg").unwrap();
        let image = image::RgbImage::from_pixel(2, 2, image::Rgb([80, 120, 160]));
        image.save(input.path()).unwrap();
        let preview = develop_preview(
            input.path(),
            DevelopSettings::default(),
            false,
            &CancellationToken::new(),
        )
        .unwrap();
        let mut report = preview.report().clone();
        report.outcome = DevelopJobOutcome::PublishedButNotDurable { bytes_written: 1 };
        let (_, warning, status, serialized) = export_completion(Ok(report), "JPEG", "out.jpg");
        assert!(warning);
        assert!(status.starts_with("Published with durability warning:"));
        assert!(serialized.is_some());
    }
}
