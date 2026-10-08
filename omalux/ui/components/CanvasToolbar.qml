import QtQuick
import QtQuick.Controls

// The tools of the on-canvas tool shown on the photo: the module's name and flat buttons
// in the image toolbar's style. A checked tool is drawn in the accent colour; `separated`
// starts a new group. Clicks report the keyboard modifiers (Ctrl adds several shapes).
Rectangle {
    id: root
    required property var theme
    property string title: ""
    property var tools: []
    signal toolClicked(string key, int modifiers)
    implicitWidth: row.implicitWidth + 16
    implicitHeight: 32
    radius: 6
    color: Qt.rgba(root.theme.background.r, root.theme.background.g, root.theme.background.b, 0.88)
    border.color: root.theme.line
    Row {
        id: row
        anchors.verticalCenter: parent.verticalCenter
        x: 8
        spacing: 2
        Text {
            anchors.verticalCenter: parent.verticalCenter
            rightPadding: 8
            text: root.title
            color: root.theme.muted
            font: root.theme.textFont
        }
        Repeater {
            model: root.tools
            Item {
                id: tool
                required property var modelData
                objectName: "canvas-tool-" + modelData.key
                readonly property bool enabledTool: modelData.enabled !== false
                width: label.implicitWidth + 14 + (modelData.separated ? 10 : 0)
                height: 24
                Rectangle {
                    visible: !!tool.modelData.separated
                    x: 4; width: 1; height: 16; anchors.verticalCenter: parent.verticalCenter
                    color: root.theme.line
                }
                Rectangle {
                    x: tool.modelData.separated ? 10 : 0
                    width: parent.width - x; height: parent.height
                    radius: 4
                    color: tool.modelData.checked ? root.theme.active : mouse.containsMouse && tool.enabledTool ? root.theme.hover : "transparent"
                    border.width: tool.modelData.checked ? 1 : 0
                    border.color: root.theme.accent
                    Text {
                        id: label
                        anchors.centerIn: parent
                        text: tool.modelData.label
                        color: !tool.enabledTool ? root.theme.muted : tool.modelData.checked || mouse.containsMouse ? root.theme.accent : root.theme.ink
                        font: root.theme.textFont
                    }
                    MouseArea {
                        id: mouse
                        anchors.fill: parent
                        hoverEnabled: true
                        enabled: tool.enabledTool
                        onClicked: event => root.toolClicked(tool.modelData.key, event.modifiers)
                        ToolTip.visible: containsMouse && !!tool.modelData.tooltip
                        ToolTip.delay: 600
                        ToolTip.text: tool.modelData.tooltip || ""
                    }
                    Accessible.role: Accessible.Button
                    Accessible.name: tool.modelData.label
                }
            }
        }
    }
}
