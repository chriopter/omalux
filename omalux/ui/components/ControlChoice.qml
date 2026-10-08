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
    // Inside a module among sliders the label takes the slider label's font and colour.
    property font labelFont: theme.textFont
    property color labelColor: theme.muted
    signal edited(real value)
    signal resetRequested()
    property alias navTarget: navTarget
    // Keyboard: ←/→ choose the previous/next option, Enter the next one (wrapping).
    NavTarget {
        id: navTarget
        navId: root.label
        label: root.label
        kind: "choice"
        enabled: root.editable
        onAdjust: steps => root.choose(Math.max(0, Math.min(root.options.length - 1, root.current + Math.sign(steps))))
        onActivate: root.choose((root.current + 1) % root.options.length)
        onReset: root.resetRequested()
    }
    function choose(index) {
        if (root.editable && index >= 0 && index !== root.current && index < root.options.length)
            root.edited(root.options[index].value)
    }

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
                color: navTarget.current ? root.theme.accent : root.labelColor
                font: root.labelFont
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
                    onClicked: { navTarget.claim(); root.edited(modelData.value) }
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
        onClicked: { navTarget.claim(); menu.popup(root.width - menu.width, root.height) }
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
