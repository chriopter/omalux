import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import "components"

ApplicationWindow {
    id: window
    width: 1280
    height: 820
    minimumWidth: 780
    minimumHeight: 520
    visible: true
    title: editor.filename + " — Omalux"
    color: editorTheme.background
    font: editorTheme.textFont

    property string activeControl: "brightness"
    property alias selectedPanel: sidebar.selectedPanel
    function showPresetDetails(id) {
        sidebar.showPresetDetails(id);
    }

    EditorTheme {
        id: editorTheme
    }
    ColumnLayout {
        anchors.fill: parent
        spacing: 0
        EditorToolbar {
            Layout.fillWidth: true
            theme: editorTheme
            logoSource: assetsRoot + "logo/omalux-logo-light.svg"
            filename: editor.filename
        }
        RowLayout {
            Layout.fillWidth: true
            Layout.fillHeight: true
            spacing: 0
            PhotoViewport {
                Layout.fillWidth: true
                Layout.fillHeight: true
                theme: editorTheme
                preview: editor.preview
                status: editor.status
            }
            EditorSidebar {
                id: sidebar
                Layout.preferredWidth: 312
                Layout.fillHeight: true
                theme: editorTheme
                backend: editor
                onControlSelected: id => window.activeControl = id
            }
        }
        GpuNotice {
            Layout.fillWidth: true
            theme: editorTheme
            message: editor.gpuWarning
        }
        EditorStatusBar {
            Layout.fillWidth: true
            theme: editorTheme
            activeControl: window.activeControl
            status: editor.status
        }
    }
    Shortcut {
        enabled: !sidebar.textEditing
        sequence: "R"
        onActivated: editor.resetControls()
    }
    Shortcut {
        enabled: !sidebar.textEditing
        sequence: "Left"
        onActivated: editor.adjustControl(window.activeControl, -1)
    }
    Shortcut {
        enabled: !sidebar.textEditing
        sequence: "Right"
        onActivated: editor.adjustControl(window.activeControl, 1)
    }
}
