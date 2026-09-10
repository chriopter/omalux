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
    palette.window: editorTheme.background
    palette.windowText: editorTheme.ink
    palette.base: "#181825"
    palette.text: editorTheme.ink
    palette.button: "#313244"
    palette.buttonText: editorTheme.ink
    palette.highlight: editorTheme.accent
    palette.highlightedText: "#11111b"
    palette.mid: editorTheme.line
    palette.dark: editorTheme.line
    color: editorTheme.background
    font: editorTheme.textFont

    property bool photoFullscreen: false
    property string activeControl: "exposure"
    property alias selectedPanel: sidebar.selectedPanel
    function revealControl(id) { sidebar.revealControl(id) }
    function showStyleDetails(id) {
        sidebar.showStyleDetails(id);
    }

    EditorTheme {
        id: editorTheme
    }
    ColumnLayout {
        anchors.fill: parent
        spacing: 0
        EditorToolbar {
            visible: !window.photoFullscreen
            Layout.fillWidth: true
            theme: editorTheme
            zoom: viewport.zoom
            onOpenRequested: dialogs.openImage()
            onSaveRequested: dialogs.exportImage()
            onZoomRequested: factor => viewport.zoomBy(factor)
            onFitRequested: viewport.fit()
            logoSource: assetsRoot + "logo/omalux-logo-light.svg"
            filename: editor.filename
        }
        RowLayout {
            Layout.fillWidth: true
            Layout.fillHeight: true
            spacing: 0
            PhotoViewport {
                id: viewport
                cropping: sidebar.geometry.cropping
                crop: sidebar.geometry.crop
                aspectRatio: sidebar.geometry.aspectRatio
                onCropEdited: rect => sidebar.geometry.crop = rect
                Layout.fillWidth: true
                Layout.fillHeight: true
                theme: editorTheme
                preview: editor.preview
                previewAspectRatio: editor.previewAspectRatio
                textureTransform: editor.previewTextureTransform
                status: editor.status
            }
            EditorSidebar {
                id: sidebar
                visible: !window.photoFullscreen
                activeControl: window.activeControl
                iconsRoot: assetsRoot + "icons/"
                Layout.preferredWidth: 352
                Layout.fillHeight: true
                theme: editorTheme
                backend: editor
                onStyleSaveRequested: dialogs.saveStyle()
                onStyleExportRequested: id => dialogs.exportStyle(id)
                onStyleDeleteRequested: (id, name) => dialogs.deleteStyle(id, name)
                onControlSelected: id => window.activeControl = id
            }
        }
        GpuNotice {
            Layout.fillWidth: true
            theme: editorTheme
            message: editor.gpuWarning
        }
        EditorStatusBar {
            visible: !window.photoFullscreen
            Layout.fillWidth: true
            theme: editorTheme
            activeControl: { const c=editor.controls.find(c => c.id===window.activeControl); return c ? c.label : "" }
            status: editor.status
        }
    }
    Connections { target: sidebar.geometry; function onCroppingChanged() { if(sidebar.geometry.cropping) viewport.fit() } }
    EditorDialogs {
        id: dialogs
        theme: editorTheme
        onOpenRequested: file => { sidebar.geometry.cancel(); viewport.fit(); editor.openPhoto(file) }
        onExportRequested: (file, quality) => editor.exportPhoto(file, quality)
        onStyleSaveRequested: name => editor.saveStyle(name)
        onStyleDeleteRequested: id => editor.deleteStyle(id)
        onStyleExportRequested: (id, directory) => editor.exportStyle(id, directory)
    }
    EditorShortcuts {
        id: shortcuts
        active: !(window.activeFocusItem && typeof window.activeFocusItem.selectAll === "function") && !dialogs.busy && !sidebar.textEditing && !helpDialog.visible && !window.photoFullscreen
        filtersActive: sidebar.selectedPanel === 0 || sidebar.selectedPanel === 2
        onPanelRequested: index => sidebar.selectedPanel = index
        onPanelStepRequested: direction => { const panels=[0,1,2,3,4]; sidebar.selectedPanel=panels[(panels.indexOf(sidebar.selectedPanel)+direction+panels.length)%panels.length] }
        onControlStepRequested: direction => { if(sidebar.selectedPanel === 0) sidebar.navigateControl(direction) }
        onValueStepRequested: steps => editor.adjustControl(window.activeControl, steps)
        onResetRequested: editor.resetControl(window.activeControl)
        onControlRequested: id => sidebar.revealControl(id)
        onGrainDetailsRequested: sidebar.toggleGrainDetails()
        onZoomRequested: factor => viewport.zoomBy(factor)
        onFitRequested: viewport.fit()
        onFullscreenRequested: { sidebar.geometry.cancel(); window.photoFullscreen = true; window.showFullScreen() }
        onOpenRequested: dialogs.openImage()
        onSaveRequested: dialogs.exportImage()
        onHelpRequested: helpDialog.open()
    }
    Shortcut { sequences: ["Return", "Enter"]; enabled: sidebar.geometry.cropping && !dialogs.busy; onActivated: sidebar.geometry.apply() }
    Shortcut { sequence: "Escape"; enabled: sidebar.geometry.cropping && !dialogs.busy; onActivated: sidebar.geometry.cancel() }
    Shortcut { sequences: ["Escape", "F"]; enabled: window.photoFullscreen; onActivated: { window.photoFullscreen = false; window.showNormal() } }
    Dialog {
        id: helpDialog
        title: "Keyboard reference"
        anchors.centerIn: parent
        width: Math.min(window.width - 40, 620)
        height: Math.min(window.height - 40, 650)
        modal: true
        standardButtons: Dialog.Close
        ScrollView {
            anchors.fill: parent
            Column {
                width: parent.width; spacing: 10
                Repeater {
                    model: shortcuts.bindings
                    Text { required property var modelData; text: modelData.keys.join(" / ") + " — " + modelData.label; color: editorTheme.ink; font: editorTheme.textFont }
                }
            }
        }
    }
}
