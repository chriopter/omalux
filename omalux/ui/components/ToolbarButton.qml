import QtQuick
import QtQuick.Controls

// Flat toolbar button: optional muted key hint before an ink label.
AbstractButton {
    id: root
    required property var theme
    property string hint: ""
    property bool grouped: false
    implicitHeight: 30
    implicitWidth: Math.max(30, label.implicitWidth + (text.length > 1 || hint ? 24 : 0))
    hoverEnabled: true
    Accessible.name: text
    contentItem: Text {
        id: label
        textFormat: Text.StyledText
        text: (root.hint ? "<font color='" + root.theme.muted + "'>" + root.hint + "</font>&nbsp;&nbsp;" : "") + root.text
        color: root.enabled ? (root.hovered ? root.theme.accent : root.theme.ink) : root.theme.muted
        font: root.theme.textFont
        horizontalAlignment: Text.AlignHCenter
        verticalAlignment: Text.AlignVCenter
    }
    background: Rectangle {
        radius: 5
        color: root.pressed ? root.theme.active : root.hovered ? root.theme.hover : "transparent"
        border.width: root.grouped && !root.visualFocus ? 0 : 1
        border.color: root.visualFocus ? root.theme.accent : root.theme.line
    }
}
