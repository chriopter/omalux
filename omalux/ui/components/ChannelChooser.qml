import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

// A short row of chips that picks which channel or curve the editor below shows (rgb curve
// R/G/B, color zones lightness/chroma/hue, tone curve L/a/b). Options may carry a colour
// dot. It selects a view, not a value: picking a chip changes no processing parameter.
Item {
    id: root
    required property var theme
    property var options: []        // [{ label, value, color? }]
    property var current
    property bool editable: true
    signal chosen(var value)
    property alias navTarget: navTarget
    // Keyboard: ←/→ pick the previous/next channel (see NavTarget).
    NavTarget {
        id: navTarget
        navId: "channel"
        label: "channel " + (root.currentIndex >= 0 ? root.options[root.currentIndex].label : "")
        kind: "choice"
        resettable: false
        activateLabel: ""
        enabled: root.editable
        onAdjust: steps => {
            const next = Math.max(0, Math.min(root.options.length - 1, root.currentIndex + Math.sign(steps)))
            if (next !== root.currentIndex) root.chosen(root.options[next].value)
        }
    }

    implicitWidth: row.implicitWidth
    implicitHeight: 22
    activeFocusOnTab: true
    readonly property int currentIndex: options.findIndex(o => o.value === current)
    Keys.onLeftPressed: if (currentIndex > 0) chosen(options[currentIndex - 1].value)
    Keys.onRightPressed: if (currentIndex < options.length - 1) chosen(options[currentIndex + 1].value)

    RowLayout {
        id: row
        anchors.fill: parent
        spacing: 4
        Repeater {
            model: root.options
            Button {
                id: chip
                required property var modelData
                required property int index
                readonly property bool selected: root.currentIndex === index
                Layout.fillHeight: true
                implicitWidth: content.implicitWidth + 16
                padding: 0
                hoverEnabled: true
                focusPolicy: Qt.NoFocus
                enabled: root.editable
                onClicked: root.chosen(modelData.value)
                Accessible.role: Accessible.RadioButton
                Accessible.name: modelData.label
                Accessible.checked: selected
                contentItem: Item {
                    RowLayout {
                        id: content
                        anchors.centerIn: parent
                        spacing: 5
                        Rectangle {
                            visible: !!chip.modelData.color
                            width: 6; height: 6; radius: 3
                            color: chip.modelData.color || "transparent"
                            opacity: chip.selected ? 1 : 0.7
                        }
                        Text {
                            text: chip.modelData.label
                            color: chip.selected ? root.theme.accent : chip.hovered ? root.theme.ink : root.theme.muted
                            font: root.theme.textFont
                        }
                    }
                }
                background: Rectangle {
                    radius: 11
                    color: chip.selected || chip.pressed ? root.theme.active : chip.hovered ? root.theme.hover : "transparent"
                    border.width: 1
                    border.color: (root.activeFocus || navTarget.current) && chip.selected ? root.theme.accent
                                : chip.selected ? root.theme.line : "transparent"
                }
            }
        }
        Item { Layout.fillWidth: true }
    }
}
