mod cli;

use cxx_qt_lib::{QGuiApplication, QQmlApplicationEngine, QString, QUrl};

fn main() {
    omalux_gui::initialize_backend_types();
    if !cli::prepare() {
        return;
    }

    let mut app = QGuiApplication::new();
    let mut engine = QQmlApplicationEngine::new();

    QGuiApplication::set_desktop_file_name(&QString::from("omalux"));
    if let Some(mut app) = app.as_mut() {
        app.as_mut().set_application_name(&QString::from("omalux"));
        app.as_mut()
            .set_application_display_name(&QString::from("Omalux"));
        app.as_mut().set_organization_name(&QString::from("Omacom"));
        app.as_mut()
            .set_organization_domain(&QString::from("omalux.org"));
    }

    if let Some(engine) = engine.as_mut() {
        engine.load(&QUrl::from("qrc:/qt/qml/org/omalux/qml/Main.qml"));
    }

    let exit_code = app.as_mut().map_or(1, |app| app.exec());
    drop(engine);
    drop(app);
    std::process::exit(exit_code);
}
