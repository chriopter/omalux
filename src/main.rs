mod backend;

use cxx_qt_lib::{QGuiApplication, QQmlApplicationEngine, QString, QUrl};

fn main() {
    let arguments: Vec<String> = std::env::args().collect();
    if arguments
        .iter()
        .any(|argument| argument == "--help" || argument == "-h")
    {
        print_help();
        return;
    }
    if arguments
        .iter()
        .any(|argument| argument == "--version" || argument == "-V")
    {
        println!("grainroom {}", env!("CARGO_PKG_VERSION"));
        return;
    }
    if arguments.iter().any(|argument| argument == "--headless")
        && std::env::var_os("QT_QPA_PLATFORM").is_none()
    {
        // SAFETY: this runs before Qt or any application threads are created.
        unsafe { std::env::set_var("QT_QPA_PLATFORM", "offscreen") };
    }

    let mut app = QGuiApplication::new();
    let mut engine = QQmlApplicationEngine::new();

    QGuiApplication::set_desktop_file_name(&QString::from("grainroom"));
    if let Some(mut app) = app.as_mut() {
        app.as_mut()
            .set_application_name(&QString::from("grainroom"));
        app.as_mut()
            .set_application_display_name(&QString::from("Grainroom"));
        app.as_mut().set_organization_name(&QString::from("Omacom"));
        app.as_mut()
            .set_organization_domain(&QString::from("omacom.io"));
    }

    if let Some(engine) = engine.as_mut() {
        engine.load(&QUrl::from("qrc:/qt/qml/io/omacom/grainroom/qml/Main.qml"));
    }

    let exit_code = app.as_mut().map_or(1, |app| app.exec());
    drop(engine);
    drop(app);
    std::process::exit(exit_code);
}

fn print_help() {
    println!(
        "Grainroom — Omarchy photo developer\n\n\
Usage:\n  grainroom [OPTIONS]\n\n\
Options:\n  --input PATH         Open JPEG, PNG, BMP, or camera RAW\n  --output PATH        Export without a save dialog\n  --format FORMAT      original, jpeg/jpg, or heic/heif (default: inferred)\n  --quality 1..100     JPEG/HEIC quality (default: 90)\n  --grain 0..100       Grain amount (default: 24)\n  --grain-size ISO     Grain size, 20..6400 (default: 4000)\n  --midtones 0..100    Midtone grain response (default: 100)\n  --headless           Run export without showing a window\n  -h, --help           Show this help\n  -V, --version        Show the version\n\n\
Examples:\n  grainroom --input photo.jpg\n  grainroom --headless --input photo.jpg --output out.heic \\\n    --format heic --quality 90 --grain 24 --grain-size 4000"
    );
}
