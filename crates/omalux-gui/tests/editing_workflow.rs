use cxx_qt_lib::{QByteArray, QGuiApplication, QQmlApplicationEngine, QUrl};
use std::{
    process::Command,
    thread,
    time::{Duration, Instant},
};

fn main() {
    if std::env::var_os("OMALUX_EDITING_TEST_CHILD").is_none() {
        let data = tempfile::tempdir().unwrap();
        let mut child = Command::new(std::env::current_exe().unwrap())
            .env("OMALUX_EDITING_TEST_CHILD", "1")
            .env("XDG_DATA_HOME", data.path())
            .env("QT_QPA_PLATFORM", "offscreen")
            .env("QT_QPA_PLATFORMTHEME", "")
            .env("QT_QUICK_BACKEND", "software")
            .env("QT_FORCE_STDERR_LOGGING", "1")
            .spawn()
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(90);
        loop {
            if let Some(status) = child.try_wait().unwrap() {
                assert!(status.success(), "Editing workflow failed: {status}");
                let exported =
                    omalux::preset::load_preset_file(&data.path().join("exported.json")).unwrap();
                assert_eq!(exported.settings.geometry, Default::default());
                assert!((exported.settings.basics.exposure_ev - 0.7).abs() < 0.001);
                return;
            }
            if Instant::now() >= deadline {
                child.kill().unwrap();
                child.wait().unwrap();
                panic!("Editing workflow timed out");
            }
            thread::sleep(Duration::from_millis(50));
        }
    }
    omalux_gui::initialize_backend_types();
    let mut app = QGuiApplication::new();
    let mut engine = QQmlApplicationEngine::new();
    let input =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../reference pictures/main.jpg");
    let qml = include_str!("qml/editing_workflow.qml")
        .replace(
            "CAPTURE_PATH",
            &serde_json::to_string(&std::env::var("OMALUX_WORKFLOW_CAPTURE").unwrap_or_default())
                .unwrap(),
        )
        .replace(
            "INPUT_PATH",
            &serde_json::to_string(&input.to_string_lossy()).unwrap(),
        )
        .replace(
            "EXPORT_PATH",
            &serde_json::to_string(
                &std::path::PathBuf::from(std::env::var_os("XDG_DATA_HOME").unwrap())
                    .join("exported.json")
                    .to_string_lossy(),
            )
            .unwrap(),
        );
    engine.pin_mut().load_data(
        &QByteArray::from(&qml),
        &QUrl::from("qrc:/editing-workflow.qml"),
    );
    let code = app.pin_mut().exec();
    drop(engine);
    drop(app);
    std::process::exit(code);
}
