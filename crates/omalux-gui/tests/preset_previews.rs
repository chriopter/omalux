//! Run Qt on a dedicated process's main thread, without touching the desktop.
use cxx_qt_lib::{QByteArray, QGuiApplication, QQmlApplicationEngine, QUrl};
use std::{
    process::Command,
    thread,
    time::{Duration, Instant},
};

fn main() {
    if std::env::var_os("OMALUX_PRESET_PREVIEW_TEST_CHILD").is_none() {
        let mut child = Command::new(std::env::current_exe().unwrap())
            .env("OMALUX_PRESET_PREVIEW_TEST_CHILD", "1")
            .env("QT_QPA_PLATFORM", "offscreen")
            .env("QT_QUICK_BACKEND", "software")
            .env("QT_FORCE_STDERR_LOGGING", "1")
            .env(
                "QT_LOGGING_RULES",
                "*.warning=true;*.critical=true;qml.debug=true",
            )
            .spawn()
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(45);
        loop {
            if let Some(status) = child.try_wait().unwrap() {
                assert!(status.success(), "preset preview UI test failed: {status}");
                return;
            }
            if Instant::now() >= deadline {
                child.kill().unwrap();
                child.wait().unwrap();
                panic!("preset preview UI test timed out");
            }
            thread::sleep(Duration::from_millis(50));
        }
    }

    omalux_gui::initialize_backend_types();
    let mut app = QGuiApplication::new();
    let mut engine = QQmlApplicationEngine::new();
    let capture = std::env::var("OMALUX_PRESET_PREVIEW_CAPTURE").unwrap_or_default();
    let qml = include_str!("qml/preset_previews.qml").replace(
        "property string capturePath: \"\"",
        &format!(
            "property string capturePath: {}",
            serde_json::to_string(&capture).unwrap()
        ),
    );
    engine.pin_mut().load_data(
        &QByteArray::from(&qml),
        &QUrl::from("qrc:/preset-preview-test.qml"),
    );
    let code = app.pin_mut().exec();
    drop(engine);
    drop(app);
    std::process::exit(code);
}
