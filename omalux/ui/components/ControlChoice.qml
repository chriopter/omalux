import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

// One of a fixed set of options, as darktable describes them. Few options are laid out side
// by side so the whole choice is readable at a glance; longer lists fall back to a menu.
Item {
    id: root
    required property var theme
    required property string label
    required property var options   // [{ value, label }]
    required property real value
    required property bool editable
    signal edited(real value)

    readonly property int current: options.findIndex(option => option.value === Math.round(value))
    readonly property bool inline: options.length > 0 && options.length <= 3
                                   && options.every(option => option.label.length <= 12)
    implicitHeight: inline ? 46 : 30

    Column {
        anchors.fill: parent
        anchors.leftMargin: 8
        anchors.rightMargin: 8
        spacing: 4

        RowLayout {
            width: parent.width
            height: 22
            Text {
                Layout.fillWidth: true
                text: root.label
                color: root.theme.muted
                font: root.theme.textFont
                elide: Text.ElideRight
            }
            Text {
                visible: !root.inline
                text: root.current >= 0 ? root.options[root.current].label : root.value
                color: root.editable ? root.theme.ink : root.theme.muted
                font: root.theme.textFont
            }
            // Says that the value opens a list, which plain text next to a label does not.
            Text {
                visible: !root.inline
                text: "\u25be"
                color: root.theme.muted
                font: root.theme.textFont
            }
        }

        RowLayout {
            visible: root.inline
            width: parent.width
            height: 20
            spacing: 4
            Repeater {
                model: root.inline ? root.options : []
                Button {
                    id: option
                    required property var modelData
                    required property int index
                    Layout.fillWidth: true
                    implicitHeight: 20
                    padding: 0
                    enabled: root.editable
                    onClicked: root.edited(modelData.value)
                    contentItem: Text {
                        text: option.modelData.label
                        color: root.current === option.index ? root.theme.accent : root.theme.muted
                        font.family: root.theme.textFont.family
                        font.pixelSize: root.theme.textFont.pixelSize
                        horizontalAlignment: Text.AlignHCenter
                        verticalAlignment: Text.AlignVCenter
                        elide: Text.ElideRight
                    }
                    background: Rectangle {
                        color: root.current === option.index
                               ? Qt.lighter(root.theme.background, 1.2) : "transparent"
                        border.color: root.current === option.index ? root.theme.line : "transparent"
                        radius: 3
                    }
                }
            }
        }
    }

    MouseArea {
        anchors.fill: parent
        enabled: root.editable && !root.inline
        onClicked: menu.popup(root.width - menu.width, root.height)
        Menu {
            id: menu
            Repeater {
                model: root.options
                MenuItem {
                    required property var modelData
                    text: modelData.label
                    onTriggered: root.edited(modelData.value)
                }
            }
        }
    }
}
