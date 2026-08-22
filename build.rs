use cxx_qt_build::{CxxQtBuilder, QmlModule};
use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

fn qsb_path() -> PathBuf {
    if let Ok(path) = env::var("QSB") {
        return PathBuf::from(path);
    }

    for qmake in ["qmake6", "qmake"] {
        for query in ["QT_HOST_BINS", "QT_HOST_LIBEXECS"] {
            if let Ok(output) = Command::new(qmake).args(["-query", query]).output() {
                if output.status.success() {
                    let path =
                        PathBuf::from(String::from_utf8_lossy(&output.stdout).trim()).join("qsb");
                    if path.exists() {
                        return path;
                    }
                }
            }
        }
    }

    PathBuf::from("qsb")
}

fn bake_shader(source: &Path, output: &Path) {
    let status = Command::new(qsb_path())
        .args(["--qt6", "-O", "-o"])
        .arg(output)
        .arg(source)
        .status()
        .expect("failed to start Qt Shader Baker (qsb)");

    assert!(status.success(), "qsb failed for {}", source.display());
}

fn main() {
    let shader_source = Path::new("qml/shaders/grain.frag");
    let shader_output = Path::new("qml/shaders/grain.frag.qsb");
    bake_shader(shader_source, shader_output);
    println!("cargo:rerun-if-changed={}", shader_source.display());

    CxxQtBuilder::new_qml_module(QmlModule::new("io.omacom.grainroom").qml_file("qml/Main.qml"))
        .qt_module("Gui")
        .qt_module("Network")
        .qt_module("Quick")
        .qt_module("QuickControls2")
        .file("src/backend.rs")
        .qrc_resources([shader_output])
        .build();
}
