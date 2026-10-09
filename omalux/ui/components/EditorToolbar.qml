import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

Rectangle {
    id: root
    required property var theme
    required property url logoSource
    required property string filename
    property real zoom: 1
    property real minimumZoom: 1
    property real maximumZoom: 16
    // Nothing to export or zoom before the first preview.
    property bool photoReady: true
    signal openRequested()
    signal saveRequested()
    signal fitRequested()
    signal zoomRequested(real factor)
    implicitHeight: 48
    color: root.theme.background
    RowLayout {
        anchors.fill: parent
        anchors.leftMargin: 18
        anchors.rightMargin: 18
        spacing: 8
        Image {
            id: logo
            source: root.logoSource
            Layout.preferredWidth: 92
            Layout.preferredHeight: 26
            fillMode: Image.PreserveAspectFit
        }
        Item {
            Layout.fillWidth: true
        }
        ToolbarButton { objectName: "toolbar-open"; theme: root.theme; hint: "[O]"; text: "OPEN"; tip: "Open a photograph"; onClicked: root.openRequested() }
        ToolbarButton { objectName: "toolbar-export"; theme: root.theme; hint: "[Ctrl+S]"; text: "EXPORT"; tip: "Export as JPEG or PNG"; enabled: root.photoReady; onClicked: root.saveRequested() }
        Item { implicitWidth: 6 }
        // Zoom: step out, the current level (click or [0] fits the photograph), step in.
        Rectangle {
            implicitWidth: zoomRow.implicitWidth
            implicitHeight: 30
            radius: 5
            color: "transparent"
            border.color: root.theme.line
            Row {
                id: zoomRow
                ToolbarButton {
                    objectName: "toolbar-zoom-out"
                    theme: root.theme; grouped: true; text: "−"; tip: "Zoom out  [−]"
                    enabled: root.photoReady && root.zoom > root.minimumZoom + .001
                    onClicked: root.zoomRequested(.8); Accessible.name: "Zoom out"
                }
                ToolbarButton {
                    objectName: "toolbar-fit"
                    theme: root.theme; grouped: true
                    width: 96
                    hint: "[0]"
                    text: Math.abs(root.zoom - 1) < .001 ? "FIT" : root.zoom.toFixed(1) + "×"
                    tip: Math.abs(root.zoom - 1) < .001 ? "The photograph fits the window" : "Fit the photograph to the window"
                    onClicked: root.fitRequested()
                    Accessible.name: "Fit photograph"
                }
                ToolbarButton {
                    objectName: "toolbar-zoom-in"
                    theme: root.theme; grouped: true; text: "+"; tip: "Zoom in  [+]"
                    enabled: root.photoReady && root.zoom < root.maximumZoom - .001
                    onClicked: root.zoomRequested(1.25); Accessible.name: "Zoom in"
                }
            }
        }
        Item {
            Layout.fillWidth: true
        }
        Text {
            objectName: "toolbar-filename"
            text: root.filename
            color: root.theme.muted
            font: root.theme.textFont
            elide: Text.ElideMiddle
            Layout.maximumWidth: 170
            // The whole name when it does not fit.
            HoverHandler { id: nameHover }
            ToolTip.visible: nameHover.hovered && truncated
            ToolTip.delay: 500
            ToolTip.text: root.filename
        }
    }
    Rectangle {
        anchors.bottom: parent.bottom
        width: parent.width
        height: 1
        color: root.theme.line
    }
}
