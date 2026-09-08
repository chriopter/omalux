import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

Rectangle {
    id: root
    required property var theme
    property string label
    implicitWidth: label.length * 7 + 22
    implicitHeight: 30
    color: "transparent"
    border.color: root.theme.line
    opacity: 0.45
    Text {
        anchors.centerIn: parent
        text: parent.label
        color: root.theme.ink
        font: root.theme.textFont
    }
}
