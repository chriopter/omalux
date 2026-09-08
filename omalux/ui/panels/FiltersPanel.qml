import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

import "../components"

ScrollView {
    id: root
    required property var theme
    required property var controls
    required property var values
    required property bool editable
    signal controlSelected(string id)
    signal controlEdited(string id, real value)
    clip: true
    contentWidth: availableWidth
    Column {
        width: parent.width
        spacing: 28
        padding: 10
        Text {
            text: "FILTERS"
            color: root.theme.ink
            font.bold: true
            font.letterSpacing: 2
        }
        Text {
            text: "01 / BASICS"
            color: root.theme.accent
            font.bold: true
            font.pixelSize: 15
            font.letterSpacing: 1
        }
        PlaceholderSlider {
            theme: root.theme
            width: parent.width - 20
            label: "EXPOSURE"
        }
        Repeater {
            model: root.controls
            delegate: ControlSlider {
                required property var modelData
                width: parent.width - 20
                theme: root.theme
                control: modelData
                value: root.values[modelData.id]
                editable: root.editable
                onSelected: root.controlSelected(modelData.id)
                onEdited: value => root.controlEdited(modelData.id, value)
            }
        }
        PlaceholderSlider {
            theme: root.theme
            width: parent.width - 20
            label: "CLARITY"
        }
        PlaceholderSlider {
            theme: root.theme
            width: parent.width - 20
            label: "HIGHLIGHTS"
        }
        PlaceholderSlider {
            theme: root.theme
            width: parent.width - 20
            label: "SHADOWS"
        }
        PlaceholderSlider {
            theme: root.theme
            width: parent.width - 20
            label: "WHITES"
        }
        PlaceholderSlider {
            theme: root.theme
            width: parent.width - 20
            label: "BLACKS"
        }
    }
}
