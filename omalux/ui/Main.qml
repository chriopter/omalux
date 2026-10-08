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
    property alias filterView: sidebar.filterView
    property alias filterSearch: sidebar.filterSearch
    property alias moduleSearch: sidebar.moduleSearch
    property alias keyHints: keyboard.hintText
    // Curated ids open their Filters row; generated ids ("operation/instance/path") are
    // selected in the visible pane.
    function revealControl(id) { if (id.indexOf("/") < 0) sidebar.revealControl(id); keyboard.selectId(id) }
    function showStyleDetails(id) {
        sidebar.showStyleDetails(id);
    }

    EditorTheme {
        id: editorTheme
    }
    // Holds keyboard focus; every key goes through it (see KeyboardNavigator).
    KeyboardNavigator {
        id: keyboard
        scope: sidebar
        pane: sidebar.selectedPanel + "/" + sidebar.filterView
        shortcuts: shortcuts
    }
    ColumnLayout {
        anchors.fill: parent
        spacing: 0
        // Keys a text field or a clicked control did not use.
        Keys.onPressed: event => keyboard.handleKey(event)
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
                picker: sidebar.tools.active ? { kind: sidebar.tools.active.kind, box: sidebar.tools.activeBox() } : null
                onPickerEdited: (box, modifiers) => sidebar.tools.setBox(box, modifiers)
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
            hints: keyboard.hintText
            status: editor.status
        }
    }
    Connections { target: sidebar.geometry; function onCroppingChanged() { if(sidebar.geometry.cropping) { sidebar.tools.cancel(); viewport.fit() } } }
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
        navigator: keyboard
        cropping: sidebar.geometry.cropping
        fullscreen: window.photoFullscreen
        panelCount: sidebar.paneOrder.length
        // Number keys and Tab both follow the tab strip as shown.
        onPanelRequested: position => sidebar.selectedPanel = sidebar.paneOrder[position]
        onPanelStepRequested: direction => { const panels=sidebar.paneOrder; sidebar.selectedPanel=panels[(panels.indexOf(sidebar.selectedPanel)+direction+panels.length)%panels.length] }
        onControlRequested: id => window.revealControl(id)
        onGrainDetailsRequested: { sidebar.selectedPanel = 0; sidebar.filterView = 0; sidebar.toggleGrainDetails() }
        onZoomRequested: factor => viewport.zoomBy(factor)
        onFitRequested: viewport.fit()
        onFullscreenRequested: { sidebar.geometry.cancel(); window.photoFullscreen = true; window.showFullScreen() }
        onFullscreenExitRequested: { window.photoFullscreen = false; window.showNormal() }
        onCropApplyRequested: sidebar.geometry.apply()
        onCropCancelRequested: sidebar.geometry.cancel()
        onOpenRequested: dialogs.openImage()
        onSaveRequested: dialogs.exportImage()
        onHelpRequested: helpDialog.open()
    }
    KeyboardHelp {
        id: helpDialog
        theme: editorTheme
        shortcuts: shortcuts
        anchors.centerIn: parent
        width: Math.min(window.width - 40, 680)
        height: Math.min(window.height - 40, 720)
    }
}
