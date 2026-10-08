import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

Rectangle {
    id: root
    required property var theme
    required property url logoSource
    required property string filename
    property real zoom: 1
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
        ToolbarButton { theme: root.theme; hint: "[O]"; text: "OPEN"; onClicked: root.openRequested() }
        ToolbarButton { theme: root.theme; hint: "[Ctrl+S]"; text: "EXPORT"; onClicked: root.saveRequested() }
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
                ToolbarButton { theme: root.theme; grouped: true; text: "−"; onClicked: root.zoomRequested(.8); Accessible.name: "Zoom out" }
                ToolbarButton {
                    objectName: "toolbar-fit"
                    theme: root.theme; grouped: true
                    width: 96
                    hint: "[0]"
                    text: Math.abs(root.zoom - 1) < .001 ? "FIT" : root.zoom.toFixed(1) + "×"
                    onClicked: root.fitRequested()
                    Accessible.name: "Fit photograph"
                }
                ToolbarButton { theme: root.theme; grouped: true; text: "+"; onClicked: root.zoomRequested(1.25); Accessible.name: "Zoom in" }
            }
        }
        Item {
            Layout.fillWidth: true
        }
        Text {
            text: root.filename
            color: root.theme.muted
            font: root.theme.textFont
            elide: Text.ElideMiddle
            Layout.maximumWidth: 170
        }
    }
    Rectangle {
        anchors.bottom: parent.bottom
        width: parent.width
        height: 1
        color: root.theme.line
    }
}
