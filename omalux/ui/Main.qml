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
    // The tool drawn on the photo follows the selected control, as darktable shows the
    // overlay of the focused module: a row of vignetting, graduated density, rotate and
    // perspective, liquify, retouch or spot removal, or a drawn-mask row ("op/instance/@shapes").
    // History and Info show none.
    readonly property var canvasKinds: ({ vignette: "vignette", graduatednd: "line", ashift: "ashift", liquify: "liquify",
                                          retouch: "shapes", spots: "shapes" })
    readonly property var canvasTitles: ({ vignette: "vignetting", graduatednd: "graduated density",
                                           ashift: "rotate and perspective", liquify: "liquify", retouch: "retouch",
                                           spots: "spot removal" })
    readonly property var canvasTool: {
        const id = window.activeControl || ""
        if ([3, 4].indexOf(sidebar.selectedPanel) >= 0 || window.photoFullscreen) return null
        let operation = "", instance = 0, path = ""
        if (id.indexOf("/") >= 0) {
            const parts = id.split("/")
            operation = parts[0]; instance = Number(parts[1]) || 0; path = parts.slice(2).join("/")
        } else {
            const control = editor.controls.find(c => c.id === id)
            if (control) operation = control.module
        }
        const kind = path === "@shapes" ? "shapes" : canvasKinds[operation] || ""
        if (!kind) return null
        return { operation: operation, instance: instance, kind: kind,
                 title: path === "@shapes" ? "drawn mask" : canvasTitles[operation] }
    }
    function requestDrawnShape(operation, instance, shape) {
        window.activeControl = operation + "/" + instance + "/@shapes"
        const name = ({ 1: "circle", 2: "path", 16: "gradient", 32: "ellipse", 64: "brush" })[shape & 115]
        if (name) Qt.callLater(() => viewport.startCanvasTool("shape:" + name))
    }
    // One interaction on the photo at a time: a module colour picker hides the drawn tool while
    // it runs (PhotoViewport.canvasShown), and showing another module's tool ends the picker.
    readonly property string canvasKey: canvasTool ? canvasTool.operation + "/" + canvasTool.instance + "/" + canvasTool.kind : ""
    onCanvasKeyChanged: if (canvasKey && sidebar.tools.active) sidebar.tools.cancel()
    onCanvasToolChanged: editor.setCanvasModule(canvasTool && canvasTool.kind !== "vignette" ? canvasTool.operation : "",
                                                canvasTool ? canvasTool.instance : 0)
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
                canvasTool: window.canvasTool
                canvasOverlay: editor.canvasOverlay
                canvasState: window.canvasTool ? sidebar.moduleStates[window.canvasTool.operation + "/" + window.canvasTool.instance] : undefined
                canvasEditable: !editor.styleBusy
                onCanvasEdited: (operation, instance, gesture) => editor.editCanvas(operation, instance, gesture)
                onCanvasParametersEdited: (operation, instance, changes) => sidebar.changeParameters(operation, instance, changes)
                onCanvasInteractionChanged: active => editor.setInteractive(active)
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
                // A shape button of a blend section: show the module's drawn mask on the photo with
                // that shape picked (DT_MASKS_CIRCLE 1, PATH 2, GRADIENT 16, ELLIPSE 32, BRUSH 64).
                onDrawnShapeRequested: (operation, instance, shape) => window.requestDrawnShape(operation, instance, shape)
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
        onOpenRequested: file => { sidebar.geometry.cancel(); sidebar.tools.blendDisplay = null; viewport.fit(); editor.openPhoto(file) }
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
        canvasDrawing: viewport.canvasCapturing
        onCanvasCancelRequested: viewport.cancelCanvasTool()
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
