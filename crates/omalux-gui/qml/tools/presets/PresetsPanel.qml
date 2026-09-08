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

        ListView {
            id: presetList
            objectName: "presetList"
            Layout.fillWidth: true
            Layout.fillHeight: true
            clip: true
            spacing: 8
            model: panel.presets
            boundsBehavior: Flickable.StopAtBounds
            ScrollBar.vertical: ScrollBar {}

            delegate: Button {
                id: presetButton
                required property int index
                required property var modelData
                width: presetList.width
                height: 112
                padding: 8
                enabled: panel.photoReady
                activeFocusOnTab: false
                onClicked: panel.presetRequested(modelData.id)

                Accessible.name: modelData.name

                contentItem: RowLayout {
                    spacing: 10

                    Image {
                        objectName: "presetPreview-" + presetButton.modelData.id
                        Layout.preferredWidth: 144
                        Layout.preferredHeight: 96
                        source: presetButton.modelData.previewUrl
                        fillMode: Image.PreserveAspectFit
                        asynchronous: true
                        smooth: true
                    }

                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: 6

                        Text {
                            Layout.fillWidth: true
                            text: presetButton.modelData.name.toUpperCase()
                            color: panel.selectedPresetId === presetButton.modelData.id
                                ? panel.theme.inkColor : panel.theme.mutedColor
                            font.family: panel.theme.monoFont
                            font.pixelSize: 11
                            font.bold: true
                            wrapMode: Text.Wrap
                            maximumLineCount: 3
                            elide: Text.ElideRight
                        }

                        Text {
                            Layout.fillWidth: true
                            text: panel.selectedPresetId === presetButton.modelData.id
                                ? "SELECTED" : (presetButton.index + 1) + " / " + panel.presets.length
                            color: panel.theme.mutedColor
                            font.family: panel.theme.monoFont
                            font.pixelSize: 8
                        }
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
    }
}
