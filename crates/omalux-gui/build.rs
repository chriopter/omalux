use cxx_qt_build::{CxxQtBuilder, QmlModule};

fn main() {
    let qml_module = QmlModule::new("io.omacom.omalux").qml_files([
        "qml/Main.qml",
        "qml/components/ParameterSlider.qml",
        "qml/components/ToolTabButton.qml",
        "qml/components/TuiButton.qml",
        "qml/tools/grain/GrainPanel.qml",
        "qml/tools/metadata/MetadataPanel.qml",
        "qml/tools/presets/PresetsPanel.qml",
    ]);

    CxxQtBuilder::new_qml_module(qml_module)
        .qt_module("Gui")
        .qt_module("Network")
        .qt_module("Quick")
        .qt_module("QuickControls2")
        .file("src/backend/mod.rs")
        .build();
}
