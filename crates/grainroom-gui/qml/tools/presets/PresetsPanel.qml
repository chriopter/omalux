import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

Item {
    id: panel

    required property var theme
    required property bool photoReady
    property int selectedPreset: -1

    readonly property var presets: [
        { "name": "Clean Neutral", "detail": "BALANCED / LOW CONTRAST" },
        { "name": "Soft Film", "detail": "WARM / FADED / GRAIN" },
        { "name": "Night Punch", "detail": "COOL / DEEP BLACKS" }
    ]

    ColumnLayout {
        anchors.fill: parent
        spacing: 10

        Text {
            text: "PRESETS"
            color: panel.theme.inkColor
            font.family: panel.theme.monoFont
            font.pixelSize: 13
            font.bold: true
            font.letterSpacing: 1
        }

        Text {
            text: "3 DEMO LOOKS"
            color: panel.theme.accentColor
            font.family: panel.theme.monoFont
            font.pixelSize: 9
            font.bold: true
        }

        Repeater {
            model: panel.presets

            delegate: Button {
                id: presetButton
                required property int index
                required property var modelData
                Layout.fillWidth: true
                Layout.preferredHeight: 62
                enabled: panel.photoReady
                activeFocusOnTab: false
                onClicked: panel.selectedPreset = index

                contentItem: Column {
                    leftPadding: 14
                    anchors.verticalCenter: parent.verticalCenter
                    spacing: 5

                    Text {
                        text: (presetButton.index + 1) + "  " + presetButton.modelData.name.toUpperCase()
                        color: panel.selectedPreset === presetButton.index
                            ? panel.theme.inkColor : panel.theme.mutedColor
                        font.family: panel.theme.monoFont
                        font.pixelSize: 11
                        font.bold: true
                    }

                    Text {
                        text: presetButton.modelData.detail
                        color: panel.theme.mutedColor
                        font.family: panel.theme.monoFont
                        font.pixelSize: 8
                    }
                }

                background: Rectangle {
                    color: panel.selectedPreset === presetButton.index
                        ? panel.theme.selectionColor
                        : presetButton.hovered ? panel.theme.surfaceColor : "transparent"
                    border.width: 1
                    border.color: panel.selectedPreset === presetButton.index
                        ? panel.theme.accentColor : panel.theme.lineColor
                }
            }
        }

        Item { Layout.fillHeight: true }
    }
}
