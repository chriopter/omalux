import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

Rectangle {
    id: root
    required property var theme
    required property string activeControl
    required property string status
    implicitHeight: 28
    color: "#242438"
    RowLayout {
        anchors.fill: parent
        anchors.leftMargin: 12
        anchors.rightMargin: 12
        Text {
            text: "[←/→] " + root.activeControl + "   [R] RESET VALUE   [?] HELP"
            color: root.theme.muted
            font: root.theme.textFont
        }
        Item {
            Layout.fillWidth: true
        }
        Text {
            text: root.status
            color: root.theme.ink
            font: root.theme.textFont
        }
    }
}
