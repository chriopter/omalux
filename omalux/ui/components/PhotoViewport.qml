import QtQuick
import QtQuick.Controls

Rectangle {
    id: root
    required property var theme
    required property string preview
    required property real previewAspectRatio
    required property vector4d textureTransform
    required property string status
    property bool cropping: false
    property var crop: ({x:0,y:0,width:1,height:1})
    property real aspectRatio: 0
    signal cropEdited(var rect)
    // The active module colour picker ({ kind: "area"|"point", box }) or null (ModuleTools).
    property var picker: null
    signal pickerEdited(var box, int modifiers)
    // Drawing on the photo: the module whose tool is shown ({ operation, instance, kind } or
    // null), what the engine reports for it, and its catalog state (see CanvasOverlay).
    property var canvasTool: null
    property string canvasOverlay: ""
    property var canvasState
    property bool canvasEditable: true
    signal canvasEdited(string operation, int instance, var gesture)
    signal canvasParametersEdited(string operation, int instance, var changes)
    signal canvasInteractionChanged(bool active)
    // A module colour picker takes the photo; the drawn tool waits until it ends.
    readonly property bool canvasShown: !!canvasTool && !cropping && !picker && preview !== ""
    readonly property bool canvasCapturing: canvasShown && canvas.capturing
    function cancelCanvasTool() { canvas.cancel() }
    // Pick one of the shown tool's toolbar entries, e.g. "shape:circle".
    function startCanvasTool(key) { canvas.toolClicked(key, 0) }
    property real zoom: 1
    readonly property real fitWidth: Math.max(1, width - 40)
    readonly property real fitHeight: Math.max(1, height - 40)
    readonly property real ratio: root.previewAspectRatio > 0 ? root.previewAspectRatio : 1
    readonly property real imageWidth: Math.min(fitWidth, fitHeight * ratio) * zoom
    readonly property real imageHeight: imageWidth / ratio
    function fit() { zoom = 1; flick.contentX = 0; flick.contentY = 0 }
    function zoomBy(factor) { zoom = Math.max(1, Math.min(16, zoom * factor)) }
    color: "#0b0b0d"
    clip: true
    Flickable {
        id: flick
        anchors.fill: parent
        contentWidth: Math.max(width, root.imageWidth + 40)
        contentHeight: Math.max(height, root.imageHeight + 40)
        interactive: !root.cropping && !root.picker && !(root.canvasShown && canvas.capturing)
        boundsBehavior: Flickable.StopAtBounds
        Image {
            id: photo
            x: (flick.contentWidth - width) / 2
            y: (flick.contentHeight - height) / 2
            width: root.imageWidth; height: root.imageHeight
            source: root.preview
            // Fit the logical image bounds above, independent of rounded raster dimensions.
            fillMode: Image.Stretch
            cache: false; asynchronous: false
            visible: GraphicsInfo.api === GraphicsInfo.Software
        }
        ShaderEffect {
            visible: GraphicsInfo.api !== GraphicsInfo.Software
            x: photo.x; y: photo.y; width: photo.width; height: photo.height
            property variant source: photo
            property vector4d textureTransform: root.textureTransform
            fragmentShader: "../../build/shaders/preview.frag.qsb"
        }
        CropOverlay {
            x: photo.x; y: photo.y; width: photo.width; height: photo.height
            visible: root.cropping
            crop: root.crop; aspectRatio: root.aspectRatio
            onCropChangedByUser: rect => root.cropEdited(rect)
        }
        // area E: color calibration's colour checker (kind "chart": four corners, ChartOverlay)
        ChartOverlay {
            objectName: "chart-overlay"
            x: photo.x; y: photo.y; width: photo.width; height: photo.height
            visible: !!root.picker && root.picker.kind === "chart" && !root.cropping
            box: root.picker && root.picker.kind === "chart" ? root.picker.box : [0.01, 0.01, 0.99, 0.01, 0.99, 0.99, 0.01, 0.99]
            chart: root.picker ? root.picker.chart || null : null
            safety: root.picker && root.picker.safety !== undefined ? root.picker.safety : 0.5
            onBoxEdited: box => root.pickerEdited(box, 0)
        }
        PickerOverlay {
            x: photo.x; y: photo.y; width: photo.width; height: photo.height
            visible: !!root.picker && root.picker.kind !== "chart" && !root.cropping
            kind: root.picker ? root.picker.kind : "area"
            box: root.picker ? root.picker.box : [0.02, 0.02, 0.98, 0.98]
            onBoxEdited: (box, modifiers) => root.pickerEdited(box, modifiers)
        }
        CanvasOverlay {
            id: canvas
            objectName: "canvas-overlay"
            x: photo.x; y: photo.y; width: photo.width; height: photo.height
            visible: root.canvasShown
            theme: root.theme
            tool: root.canvasShown ? root.canvasTool : null
            overlayJson: root.canvasOverlay
            moduleState: root.canvasState
            editable: root.canvasEditable
            viewScale: Math.min(flick.width, flick.height) / Math.max(1, Math.min(root.imageWidth, root.imageHeight))
            onEdited: gesture => root.canvasEdited(root.canvasTool.operation, root.canvasTool.instance, gesture)
            onParametersEdited: changes => root.canvasParametersEdited(root.canvasTool.operation, root.canvasTool.instance, changes)
            onInteractionChanged: active => root.canvasInteractionChanged(active)
        }
        WheelHandler { enabled: !root.cropping; onWheel: event => { root.zoomBy(event.angleDelta.y > 0 ? 1.15 : 1 / 1.15); event.accepted = true } }
        PinchHandler {
            enabled: !root.cropping
            target: null
            property real startZoom: 1
            onActiveChanged: if (active) startZoom = root.zoom
            onActiveScaleChanged: root.zoom = Math.max(1, Math.min(16, startZoom * activeScale))
        }
    }
    CanvasToolbar {
        objectName: "canvas-toolbar"
        anchors.top: parent.top; anchors.left: parent.left; anchors.margins: 10
        visible: root.canvasShown && (canvas.tools.length > 0 || title !== "")
        theme: root.theme
        title: root.canvasTool ? (root.canvasTool.title || "") : ""
        tools: canvas.tools
        onToolClicked: (key, modifiers) => canvas.toolClicked(key, modifiers)
    }
    Text {
        anchors.centerIn: parent; width: parent.width - 40
        visible: root.preview === ""; text: root.status
        color: root.theme.muted; font: root.theme.textFont
        wrapMode: Text.WordWrap; horizontalAlignment: Text.AlignHCenter
    }
}
