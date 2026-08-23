import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

Item {
    id: panel

    required property var theme
    required property bool photoReady
    required property string catalogJson
    required property string selectedPresetId
    signal presetRequested(string id)

    readonly property var presets: {
        try {
            return JSON.parse(catalogJson).presets || []
        } catch (error) {
            return []
        }
    }

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
            text: panel.presets.length + " CORE PRESET" + (panel.presets.length === 1 ? "" : "S")
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
                onClicked: panel.presetRequested(modelData.id)

                contentItem: Column {
                    leftPadding: 14
                    anchors.verticalCenter: parent.verticalCenter
                    spacing: 5

                    Text {
                        text: (presetButton.index + 1) + "  " + presetButton.modelData.name.toUpperCase()
                        color: panel.selectedPresetId === presetButton.modelData.id
                            ? panel.theme.inkColor : panel.theme.mutedColor
                        font.family: panel.theme.monoFont
                        font.pixelSize: 11
                        font.bold: true
                    }

                    Text {
                        text: presetButton.modelData.id.toUpperCase()
                        color: panel.theme.mutedColor
                        font.family: panel.theme.monoFont
                        font.pixelSize: 8
                    }
                }

                background: Rectangle {
                    color: panel.selectedPresetId === presetButton.modelData.id
                        ? panel.theme.selectionColor
                        : presetButton.hovered ? panel.theme.surfaceColor : "transparent"
                    border.width: 1
                    border.color: panel.selectedPresetId === presetButton.modelData.id
                        ? panel.theme.accentColor : panel.theme.lineColor
                }
            }
        }

        Item { Layout.fillHeight: true }
    }
}
