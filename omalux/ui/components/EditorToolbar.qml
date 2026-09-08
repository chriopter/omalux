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
    color: "#1e1e2e"
    RowLayout {
        anchors.fill: parent
        anchors.leftMargin: 18
        anchors.rightMargin: 18
        spacing: 12
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
        Button { text: "[O] OPEN"; onClicked: root.openRequested() }
        Button { text: "[Ctrl+S] EXPORT"; onClicked: root.saveRequested() }
        ToolButton { text: "−"; onClicked: root.zoomRequested(.8); Accessible.name: "Zoom out" }
        Text { text: root.zoom === 1 ? "FIT" : root.zoom.toFixed(1) + "× FIT"; color: root.theme.ink; font: root.theme.textFont }
        ToolButton { text: "+"; onClicked: root.zoomRequested(1.25); Accessible.name: "Zoom in" }
        Button { text: "[0] FIT"; onClicked: root.fitRequested() }
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
