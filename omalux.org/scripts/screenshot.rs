//! Loads the real application with a small capture controller, using Qt offscreen.
use cxx_qt_lib::{QByteArray, QGuiApplication, QQmlApplicationEngine, QUrl};

fn main() {
    omalux_gui::initialize_backend_types();
    let mut app = QGuiApplication::new();
    let mut engine = QQmlApplicationEngine::new();
    engine.pin_mut().load_data(
        &QByteArray::from(include_str!("screenshot.qml")),
        &QUrl::from("qrc:/website-screenshot.qml"),
    );
    let code = app.pin_mut().exec();
    drop(engine);
    drop(app);
    std::process::exit(code);
}
