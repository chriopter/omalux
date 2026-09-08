import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

Column {
    id: root
    required property var theme
    required property real value
    required property bool editable
    signal selected
    signal edited(real value)
    required property var control
    width: parent.width - 20
    spacing: 8
    RowLayout {
        width: parent.width
        Text {
            text: root.control.label
            color: root.theme.accent
            font: root.theme.textFont
            Layout.fillWidth: true
        }
        Text {
            text: Number(root.value).toFixed(root.control.decimals) + root.control.unit
            color: root.theme.accent
            font: root.theme.textFont
        }
    }
    Slider {
        id: controlSlider
        enabled: root.editable
        width: parent.width
        height: 30
        padding: 0
        from: root.control.minimum
        to: root.control.maximum
        stepSize: root.control.step
        value: root.value
        onPressedChanged: if (pressed)
            root.selected()
        onActiveFocusChanged: if (activeFocus)
            root.selected()
        onMoved: root.edited(value)
        background: Rectangle {
            x: controlSlider.leftPadding
            y: controlSlider.topPadding + controlSlider.availableHeight / 2
            width: controlSlider.availableWidth
            height: 2
            color: root.theme.muted
        }
        handle: Rectangle {
            x: controlSlider.leftPadding + controlSlider.visualPosition * (controlSlider.availableWidth - width)
            y: controlSlider.topPadding + controlSlider.availableHeight / 2 - height / 2
            width: 8
            height: 8
            color: controlSlider.pressed ? "#ffffff" : root.theme.accent
        }
        Accessible.name: root.control.label
    }
}
