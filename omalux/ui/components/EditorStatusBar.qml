import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

Rectangle {
    id: root
    required property var theme
    // The keys that apply to the current selection, from KeyboardNavigator.
    required property string hints
    required property string status
    implicitHeight: 28
    color: "#242438"
    RowLayout {
        anchors.fill: parent
        anchors.leftMargin: 12
        anchors.rightMargin: 12
        Text {
            objectName: "keyHints"
            Layout.fillWidth: true
            Layout.minimumWidth: 0
            text: root.hints
            elide: Text.ElideRight
            color: root.theme.muted
            font: root.theme.textFont
        }
        Text {
            text: root.status
            color: root.theme.ink
            font: root.theme.textFont
        }
    }
}
