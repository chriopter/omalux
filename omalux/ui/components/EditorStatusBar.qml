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
        spacing: 16
        // A long message (the path of an export) is cut in the middle, never pushed out of the
        // window; resting the pointer on it shows all of it.
        Text {
            id: statusText
            objectName: "statusText"
            Layout.maximumWidth: Math.max(120, root.width * .6)
            text: root.status
            elide: Text.ElideMiddle
            color: root.theme.ink
            font: root.theme.textFont
            HoverHandler { id: statusHover }
            ToolTip.visible: statusHover.hovered && statusText.truncated
            ToolTip.delay: 500
            ToolTip.text: root.status
        }
    }
}
