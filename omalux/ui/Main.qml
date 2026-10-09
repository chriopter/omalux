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
    // The hovered menu item (Basic MenuItem: palette.light) on the raised fill of the pane tabs;
    // the default near-white bar made its pale label unreadable.
    palette.light: editorTheme.active
    palette.dark: editorTheme.line
    color: editorTheme.background
    font: editorTheme.textFont

    property bool photoFullscreen: false
    property string activeControl: "exposure"
    // The pane the active control was chosen in: its tool on the photo shows only with that pane,
    // as darktable hides a module's overlay when the module loses focus.
    property int activeControlPane: 0
    onActiveControlChanged: activeControlPane = sidebar.selectedPanel
    property alias selectedPanel: sidebar.selectedPanel
    property alias filterView: sidebar.filterView
    property alias filterSearch: sidebar.filterSearch
    property alias moduleSearch: sidebar.moduleSearch
    property alias keyHints: keyboard.hintText
    // A file or style dialog is open (native or Qt's own); keys belong to it until it closes.
    readonly property bool dialogOpen: dialogs.busy
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
        if (sidebar.selectedPanel !== window.activeControlPane || window.photoFullscreen) return null
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
            id: toolbar
            visible: !window.photoFullscreen
            iconsRoot: assetsRoot + "icons/"
            busy: editor.styleBusy
            exampleAvailable: window.examplePhotoUrl !== ""
            onExampleRequested: window.openExample()
            Layout.fillWidth: true
            theme: editorTheme
            zoom: viewport.zoom
            minimumZoom: viewport.minimumZoom
            maximumZoom: viewport.maximumZoom
            photoReady: editor.preview !== ""
            comparing: editor.comparing
            compareAvailable: !sidebar.geometry.cropping && !editor.styleBusy
            onCompareRequested: window.toggleCompare()
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
                picker: sidebar.tools.active ? { kind: sidebar.tools.active.kind, box: sidebar.tools.activeBox(),
                                                 chart: sidebar.tools.chartLayout, safety: (sidebar.tools.active.gui || {})["@safety"] } : null
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
                photoMissing: editor.photoMissing
                comparing: editor.comparing
                onOpenRequested: dialogs.openImage()
                iconsRoot: assetsRoot + "icons/"
                exampleAvailable: window.examplePhotoUrl !== ""
                onExampleRequested: window.openExample()
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
            suppressed: window.photoFullscreen
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
    // A dialog in a window of its own (Qt's file dialog, or the platform's) hands the keys back
    // to the editor when it closes: the window is asked to become active again (offscreen and
    // some platforms leave the focus on the closed dialog window) and the navigator takes focus.
    Connections {
        target: dialogs
        function onBusyChanged() { if (!dialogs.busy) { window.requestActivate(); Qt.callLater(keyboard.reclaim) } }
    }
    // Before/after is a view of the photograph as opened; not while the crop frame is edited.
    function toggleCompare() {
        if (editor.comparing) editor.comparing = false
        else if (!sidebar.geometry.cropping) { sidebar.tools.cancel(); editor.comparing = true }
    }
    // The example photograph shipped with the application ("" when the build carries none):
    // main.cpp looks for it beside the other assets.
    readonly property string examplePhotoUrl: typeof examplePhoto === "string" ? examplePhoto : ""
    function openExample() { if (examplePhotoUrl !== "" && !editor.styleBusy) openFile(examplePhotoUrl) }
    // Opening a photograph, from the dialog or a file dropped on the window: tools on the photo
    // end, the view fits the new image.
    function openFile(file) {
        sidebar.geometry.cancel(); sidebar.tools.blendDisplay = null; sidebar.tools.moduleDisplay = null
        viewport.fit(); editor.openPhoto(file)
    }
    // A file dragged from the file manager opens like one chosen in the dialog.
    DropArea {
        id: dropArea
        objectName: "photo-drop"
        anchors.fill: parent
        enabled: !dialogs.busy && !editor.styleBusy
        function fileOf(drop) {
            const urls = drop.urls || []
            for (let i = 0; i < urls.length; ++i)
                if (String(urls[i]).startsWith("file:")) return urls[i]
            return ""
        }
        onEntered: drag => { drag.accepted = fileOf(drag) !== "" }
        onDropped: drop => {
            const file = fileOf(drop)
            if (file === "") return
            drop.accept(Qt.CopyAction)
            window.openFile(file)
        }
        Rectangle {
            objectName: "photo-drop-hint"
            visible: dropArea.containsDrag
            parent: viewport
            anchors.fill: parent
            anchors.margins: 12
            z: 10
            radius: 8
            color: Qt.rgba(0.12, 0.12, 0.18, 0.82)
            border.color: editorTheme.accent
            border.width: 2
            Text {
                anchors.centerIn: parent
                text: "Drop to open the photograph"
                color: editorTheme.ink
                font: editorTheme.settingsFont
            }
        }
    }
    EditorDialogs {
        id: dialogs
        theme: editorTheme
        photoPath: String(editor.metadata.path || "")
        onOpenRequested: file => window.openFile(file)
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
        picking: !!sidebar.tools.active
        comparing: editor.comparing
        onCompareRequested: window.toggleCompare()
        onPickerCancelRequested: sidebar.tools.cancel()
        panelCount: sidebar.keyOrder.length
        // Number keys and Tab both follow the tab strip as shown.
        onPanelRequested: position => sidebar.selectedPanel = sidebar.keyOrder[position]
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
        onOpenMenuRequested: toolbar.openMenu()
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
