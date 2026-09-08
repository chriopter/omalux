import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

Rectangle {
    id: root
    required property var theme
    required property url logoSource
    required property string filename
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
        PlaceholderButton {
            theme: root.theme
            label: "[O] OPEN"
        }
        PlaceholderButton {
            theme: root.theme
            label: "[S] SAVE"
        }
        Text {
            text: "ZOOM"
            color: root.theme.muted
            font: root.theme.textFont
        }
        Rectangle {
            Layout.preferredWidth: 90
            height: 2
            color: root.theme.line
        }
        Text {
            text: "FIT"
            color: root.theme.ink
            font: root.theme.textFont
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
