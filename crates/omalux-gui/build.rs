use cxx_qt_build::{CxxQtBuilder, QmlModule};

#[path = "build/preset_previews.rs"]
mod preset_previews;

fn main() {
    let preview_resources =
        preset_previews::package().expect("could not package stored preset thumbnails");
    let qml_module = QmlModule::new("org.omalux").qml_files([
        "qml/Main.qml",
        "qml/components/ParameterSlider.qml",
        "qml/components/SidebarScrollHandler.qml",
        "qml/components/ToolTabButton.qml",
        "qml/components/TuiButton.qml",
        "qml/tools/effects/EffectsPanel.qml",
        "qml/tools/metadata/MetadataPanel.qml",
        "qml/tools/presets/PresetsPanel.qml",
    ]);

    CxxQtBuilder::new_qml_module(qml_module)
        .qrc("icons.qrc")
        .qrc(preview_resources)
        .include_dir("src/backend")
        .qt_module("Gui")
        .qt_module("Network")
        .qt_module("Quick")
        .qt_module("QuickControls2")
        .file("src/backend/mod.rs")
        .cpp_file("src/backend/theme_watcher.cpp")
        .build();
}
