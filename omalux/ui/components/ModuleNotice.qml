import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

// One muted line inside a module for what Omalux cannot edit yet (for example parameters
// that darktable draws on the image). Long text wraps to three lines, beyond that it is
// elided with the full text as tooltip.
Item {
    id: root
    required property var theme
    property string text: ""

    implicitWidth: 260
    implicitHeight: Math.max(22, label.implicitHeight + 4)
    Accessible.role: Accessible.StaticText
    Accessible.name: text

    RowLayout {
        anchors.fill: parent
        spacing: 7
        Rectangle {
            Layout.alignment: Qt.AlignTop
            Layout.topMargin: 4
            width: 13; height: 13; radius: 6.5
            color: "transparent"
            border.color: root.theme.line
            Text {
                anchors.centerIn: parent
                text: "i"
                color: root.theme.muted
                font.family: root.theme.textFont.family
                font.pixelSize: 9
            }
        }
        Text {
            id: label
            Layout.fillWidth: true
            text: root.text
            color: root.theme.muted
            font: root.theme.textFont
            wrapMode: Text.WordWrap
            maximumLineCount: 3
            elide: Text.ElideRight
            HoverHandler { id: hover }
            ToolTip.visible: hover.hovered && label.truncated
            ToolTip.text: root.text
            ToolTip.delay: 400
        }
    }
}
