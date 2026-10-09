import QtQuick
import QtQuick.Controls

// Flat toolbar button: optional muted key hint before an ink label. Hover raises the fill and
// never takes the accent (that marks focus and the chosen tab); `tip` names the action after a
// short rest of the pointer.
AbstractButton {
    id: root
    required property var theme
    property string hint: ""
    property bool grouped: false
    property string tip: ""
    ToolTip.visible: hovered && !pressed && tip !== ""
    ToolTip.delay: 500
    ToolTip.text: tip
    // The keys stay with the editor: a click must not leave the focus ring on the toolbar.
    focusPolicy: Qt.NoFocus
    implicitHeight: 30
    implicitWidth: Math.max(30, label.implicitWidth + (text.length > 1 || hint ? 24 : 0))
    hoverEnabled: true
    Accessible.name: text
    contentItem: Text {
        id: label
        textFormat: Text.StyledText
        text: (root.hint ? "<font color='" + root.theme.muted + "'>" + root.hint + "</font>&nbsp;&nbsp;" : "") + root.text
        color: root.enabled ? root.theme.ink : root.theme.muted
        opacity: root.enabled ? 1 : .55
        font: root.theme.textFont
        horizontalAlignment: Text.AlignHCenter
        verticalAlignment: Text.AlignVCenter
    }
    background: Rectangle {
        radius: 5
        color: !root.enabled ? "transparent" : root.pressed ? root.theme.active : root.hovered ? root.theme.hover : "transparent"
        border.width: root.grouped && !root.visualFocus ? 0 : 1
        border.color: root.visualFocus ? root.theme.accent : root.theme.line
    }
}
