import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import io.omacom.grainroom

Item {
    id: panel

    required property var theme
    required property bool photoReady
    property int selectedIndex: 0

    readonly property var controls: [
        { "label": "Brightness", "from": -100, "to": 100 },
        { "label": "Contrast", "from": -100, "to": 100 },
        { "label": "Clarity", "from": -100, "to": 100 },
        { "label": "Highlights", "from": -100, "to": 100 },
        { "label": "Shadows", "from": -100, "to": 100 },
        { "label": "Whites", "from": -100, "to": 100 },
        { "label": "Blacks", "from": -100, "to": 100 },
        { "label": "Saturation", "from": -100, "to": 100 },
        { "label": "Vibrance", "from": -100, "to": 100 },
        { "label": "Temperature", "from": -100, "to": 100 },
        { "label": "Tint", "from": -100, "to": 100 },
        { "label": "Bloom", "from": 0, "to": 100 },
        { "label": "Halation", "from": 0, "to": 100 },
        { "label": "Fade", "from": 0, "to": 100 },
        { "label": "Grain", "from": 0, "to": 100 },
        { "label": "Vignette", "from": -100, "to": 100 },
        { "label": "Sharpness", "from": 0, "to": 100 }
    ]

    function parameterAt(index) {
        return sliderRepeater.itemAt(index)
    }

    function moveSelection(direction) {
        selectedIndex = (selectedIndex + direction + controls.length) % controls.length
        Qt.callLater(ensureSelectionVisible)
    }

    function adjustSelection(direction, coarse) {
        parameterAt(selectedIndex).nudge(direction, coarse)
    }

    function resetSelection() {
        parameterAt(selectedIndex).resetValue()
    }

    function ensureSelectionVisible() {
        const control = parameterAt(selectedIndex)
        if (!control)
            return
        const flickable = adjustmentScroll.flickable
        const point = control.mapToItem(flickable, 0, 0)
        if (point.y < 0)
            flickable.contentY = Math.max(0, flickable.contentY + point.y - 4)
        else if (point.y + control.height > adjustmentScroll.availableHeight)
            flickable.contentY += point.y + control.height
                - adjustmentScroll.availableHeight + 4
    }

    ColumnLayout {
        anchors.fill: parent
        spacing: 8

        RowLayout {
            Layout.fillWidth: true

            Text {
                text: "03 / ADJUSTMENTS"
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

        ScrollView {
            id: adjustmentScroll
            readonly property Flickable flickable: contentItem as Flickable
            Layout.fillWidth: true
            Layout.fillHeight: true
            clip: true
            contentWidth: availableWidth
            ScrollBar.horizontal.policy: ScrollBar.AlwaysOff
            ScrollBar.vertical.policy: ScrollBar.AsNeeded

            ColumnLayout {
                width: adjustmentScroll.availableWidth
                spacing: 4

                Repeater {
                    id: sliderRepeater
                    model: panel.controls

                    delegate: ParameterSlider {
                        required property int index
                        required property var modelData

                        Layout.fillWidth: true
                        theme: panel.theme
                        photoReady: panel.photoReady
                        selectedParameter: panel.selectedIndex
                        parameterIndex: index
                        label: modelData.label
                        from: modelData.from
                        to: modelData.to
                        initialValue: 0
                        stepSize: 1
                        coarseStep: 10
                        onSelectionRequested: requestedIndex => {
                            panel.selectedIndex = requestedIndex
                            panel.ensureSelectionVisible()
                        }
                    }
                }
            }
        }
    }
}
