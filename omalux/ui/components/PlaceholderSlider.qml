import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

Column {
    id: root
    required property var theme
    property string label
    width: parent.width
    spacing: 14
    opacity: 0.38
    RowLayout {
        width: parent.width
        Text {
            text: label
            color: root.theme.ink
            font: root.theme.textFont
            Layout.fillWidth: true
        }
        Text {
            text: "—"
            color: root.theme.muted
            font: root.theme.textFont
        }
    }
    Rectangle {
        width: parent.width
        height: 2
        color: root.theme.muted
        Rectangle {
            anchors.centerIn: parent
            width: 7
            height: 7
            color: root.theme.muted
        }
    }
}
