import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import io.omacom.grainroom

Item {
    id: panel

    required property var theme
    required property bool photoReady
    property int selectedIndex: 0

    function parameterAt(index) {
        switch (index) {
        case 0: return highlightAmount
        case 1: return highlightColor
        case 2: return shadowAmount
        default: return shadowColor
        }
    }

    function moveSelection(direction) {
        selectedIndex = (selectedIndex + direction + 4) % 4
    }

    function adjustSelection(direction, coarse) {
        parameterAt(selectedIndex).nudge(direction, coarse)
    }

    function resetSelection() {
        parameterAt(selectedIndex).resetValue()
    }

    ColumnLayout {
        anchors.fill: parent
        spacing: 8

        RowLayout {
            Layout.fillWidth: true

            Text {
                text: "04 / TINTING"
                color: panel.theme.inkColor
                font.family: panel.theme.monoFont
                font.pixelSize: 13
                font.bold: true
                font.letterSpacing: 1
            }

            Item { Layout.fillWidth: true }

            Text {
                text: "UI MOCK"
                color: panel.theme.accentColor
                font.family: panel.theme.monoFont
                font.pixelSize: 9
                font.bold: true
            }
        }

        Text {
            text: "HIGHLIGHTS"
            color: panel.theme.accentColor
            font.family: panel.theme.monoFont
            font.pixelSize: 10
            font.bold: true
        }

        ParameterSlider {
            id: highlightAmount
            Layout.fillWidth: true
            theme: panel.theme
            photoReady: panel.photoReady
            selectedParameter: panel.selectedIndex
            parameterIndex: 0
            label: "Amount"
            from: 0
            to: 100
            initialValue: 0
            onSelectionRequested: index => panel.selectedIndex = index
        }

        ParameterSlider {
            id: highlightColor
            Layout.fillWidth: true
            theme: panel.theme
            photoReady: panel.photoReady
            selectedParameter: panel.selectedIndex
            parameterIndex: 1
            label: "Color"
            from: -180
            to: 180
            initialValue: 0
            suffix: "°"
            onSelectionRequested: index => panel.selectedIndex = index
        }

        Rectangle {
            Layout.fillWidth: true
            Layout.preferredHeight: 1
            color: panel.theme.lineColor
        }

        Text {
            text: "SHADOWS"
            color: panel.theme.accentColor
            font.family: panel.theme.monoFont
            font.pixelSize: 10
            font.bold: true
        }

        ParameterSlider {
            id: shadowAmount
            Layout.fillWidth: true
            theme: panel.theme
            photoReady: panel.photoReady
            selectedParameter: panel.selectedIndex
            parameterIndex: 2
            label: "Amount"
            from: 0
            to: 100
            initialValue: 0
            onSelectionRequested: index => panel.selectedIndex = index
        }

        ParameterSlider {
            id: shadowColor
            Layout.fillWidth: true
            theme: panel.theme
            photoReady: panel.photoReady
            selectedParameter: panel.selectedIndex
            parameterIndex: 3
            label: "Color"
            from: -180
            to: 180
            initialValue: 0
            suffix: "°"
            onSelectionRequested: index => panel.selectedIndex = index
        }

        Item { Layout.fillHeight: true }

        Text {
            Layout.fillWidth: true
            text: "HIGHLIGHT / SHADOW COLOR WHEELS\nPROCESSING FOLLOWS AFTER UI VALIDATION"
            color: panel.theme.mutedColor
            font.family: panel.theme.monoFont
            font.pixelSize: 9
            lineHeight: 1.45
        }
    }
}
