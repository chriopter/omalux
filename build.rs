use cxx_qt_build::{CxxQtBuilder, QmlModule};
use std::env;
use std::fs;
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
    let output_directory = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR is set by Cargo"));
    let shader_source = Path::new("qml/shaders/film_grain.frag");
    let shader_output = output_directory.join("film_grain.frag.qsb");
    bake_shader(shader_source, &shader_output);
    println!("cargo:rerun-if-changed={}", shader_source.display());

    let resource_file = output_directory.join("grainroom_shaders.qrc");
    let resource_xml = format!(
        "<RCC><qresource prefix=\"/qt/qml/io/omacom/grainroom\"><file alias=\"qml/shaders/film_grain.frag.qsb\">{}</file></qresource></RCC>",
        shader_output.display()
    );
    fs::write(&resource_file, resource_xml).expect("failed to write shader resource file");

    let qml_module = QmlModule::new("io.omacom.grainroom").qml_files([
        "qml/Main.qml",
        "qml/components/MockParameterSlider.qml",
        "qml/components/ParameterSlider.qml",
        "qml/components/ToolTabButton.qml",
        "qml/components/TuiButton.qml",
        "qml/tools/crop/CropPanel.qml",
        "qml/tools/grain/GrainEffect.qml",
        "qml/tools/grain/GrainPanel.qml",
        "qml/tools/metadata/MetadataPanel.qml",
    ]);

    CxxQtBuilder::new_qml_module(qml_module)
        .qt_module("Gui")
        .qt_module("Network")
        .qt_module("Quick")
        .qt_module("QuickControls2")
        .file("src/backend/mod.rs")
        .qrc(&resource_file)
        .build();
}
