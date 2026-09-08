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
        interactive: !root.cropping
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
        WheelHandler { enabled: !root.cropping; onWheel: event => { root.zoomBy(event.angleDelta.y > 0 ? 1.15 : 1 / 1.15); event.accepted = true } }
        PinchHandler {
            enabled: !root.cropping
            target: null
            property real startZoom: 1
            onActiveChanged: if (active) startZoom = root.zoom
            onActiveScaleChanged: root.zoom = Math.max(1, Math.min(16, startZoom * activeScale))
        }
    }
    Text {
        anchors.centerIn: parent; width: parent.width - 40
        visible: root.preview === ""; text: root.status
        color: root.theme.muted; font: root.theme.textFont
        wrapMode: Text.WordWrap; horizontalAlignment: Text.AlignHCenter
    }
}
