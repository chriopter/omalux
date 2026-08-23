import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import io.omacom.grainroom

Item {
    id: panel

    required property var theme
    required property bool photoReady
    required property int selectedParameter
    required property bool advancedExpanded

    readonly property int parameterCount: 23
    property alias grainValue: grainControl.value
    property alias grainSizeValue: grainSizeControl.value
    property alias midtonesValue: midtonesControl.value

    signal selectionRequested(int index)
    signal advancedToggleRequested
    signal parameterCommitted(string id, real value)

    readonly property var basics: [
        { "label": "Brightness", "from": -100, "to": 100 },
        { "label": "Contrast", "from": -100, "to": 100 },
        { "label": "Clarity", "from": -100, "to": 100 },
        { "label": "Highlights", "from": -100, "to": 100 },
        { "label": "Shadows", "from": -100, "to": 100 },
        { "label": "Whites", "from": -100, "to": 100 },
        { "label": "Blacks", "from": -100, "to": 100 }
    ]

    readonly property var colors: [
        { "label": "Saturation", "from": -100, "to": 100, "suffix": "" },
        { "label": "Vibrance", "from": -100, "to": 100, "suffix": "" },
        { "label": "Temperature", "from": -100, "to": 100, "suffix": "" },
        { "label": "Tint", "from": -100, "to": 100, "suffix": "" },
        { "label": "Highlight amount", "from": 0, "to": 100, "suffix": "" },
        { "label": "Highlight color", "from": -180, "to": 180, "suffix": "°" },
        { "label": "Shadow amount", "from": 0, "to": 100, "suffix": "" },
        { "label": "Shadow color", "from": -180, "to": 180, "suffix": "°" }
    ]

    function parameterAt(index) {
        if (index < 7)
            return basicsRepeater.itemAt(index)
        if (index < 15)
            return colorRepeater.itemAt(index - 7)
        switch (index) {
        case 15: return bloomControl
        case 16: return halationControl
        case 17: return fadeControl
        case 18: return grainControl
        case 19: return grainSizeControl
        case 20: return midtonesControl
        case 21: return vignetteControl
        default: return sharpnessControl
        }
    }

    function navigationOrder() {
        const result = []
        for (let index = 0; index < parameterCount; ++index) {
            if (advancedExpanded || (index !== 19 && index !== 20))
                result.push(index)
        }
        return result
    }

    function ensureVisible(control) {
        if (!control)
            return
        const flickable = editScroll.flickable
        const point = control.mapToItem(flickable, 0, 0)
        if (point.y < 0)
            flickable.contentY = Math.max(0, flickable.contentY + point.y - 4)
        else if (point.y + control.height > editScroll.availableHeight)
            flickable.contentY += point.y + control.height
                - editScroll.availableHeight + 4
    }

    component GroupHeading: Text {
        Layout.fillWidth: true
        Layout.topMargin: 8
        Layout.bottomMargin: 2
        color: panel.theme.accentColor
        font.family: panel.theme.monoFont
        font.pixelSize: 10
        font.bold: true
        font.letterSpacing: 1
    }

    ScrollView {
        id: editScroll
        readonly property Flickable flickable: contentItem as Flickable
        anchors.fill: parent
        clip: true
        contentWidth: availableWidth
        ScrollBar.horizontal.policy: ScrollBar.AlwaysOff
        ScrollBar.vertical.policy: ScrollBar.AsNeeded

        ColumnLayout {
            width: editScroll.availableWidth
            spacing: 4

            GroupHeading { text: "01 / BASICS" }

            Repeater {
                id: basicsRepeater
                model: panel.basics

                delegate: ParameterSlider {
                    required property int index
                    required property var modelData
                    Layout.fillWidth: true
                    theme: panel.theme
                    photoReady: panel.photoReady
                    selectedParameter: panel.selectedParameter
                    parameterIndex: index
                    label: modelData.label
                    from: modelData.from
                    to: modelData.to
                    initialValue: 0
                    onSelectionRequested: requestedIndex => panel.selectionRequested(requestedIndex)
                    onValueCommitted: value => panel.parameterCommitted([
                        "basics.brightness", "basics.contrast", "basics.clarity",
                        "basics.highlights", "basics.shadows", "basics.whites",
                        "basics.blacks"
                    ][index], value)
                }
            }

            GroupHeading { text: "02 / COLOR" }

            Repeater {
                id: colorRepeater
                model: panel.colors

                delegate: ParameterSlider {
                    required property int index
                    required property var modelData
                    Layout.fillWidth: true
                    theme: panel.theme
                    photoReady: panel.photoReady
                    selectedParameter: panel.selectedParameter
                    parameterIndex: index + 7
                    label: modelData.label
                    from: modelData.from
                    to: modelData.to
                    suffix: modelData.suffix
                    initialValue: 0
                    onSelectionRequested: requestedIndex => panel.selectionRequested(requestedIndex)
                    onValueCommitted: value => panel.parameterCommitted([
                        "basics.saturation", "basics.vibrance", "basics.temperature",
                        "basics.tint", "color_grading.highlights.saturation",
                        "color_grading.highlights.hue_degrees",
                        "color_grading.shadows.saturation",
                        "color_grading.shadows.hue_degrees"
                    ][index], value < 0 && (index === 5 || index === 7) ? value + 360 : value)
                }
            }

            GroupHeading { text: "03 / EFFECTS" }

            ParameterSlider {
                id: bloomControl
                Layout.fillWidth: true
                theme: panel.theme
                photoReady: panel.photoReady
                selectedParameter: panel.selectedParameter
                parameterIndex: 15
                label: "Bloom"
                from: 0
                to: 100
                initialValue: 0
                onSelectionRequested: index => panel.selectionRequested(index)
                onValueCommitted: value => panel.parameterCommitted("effects.bloom", value)
            }

            ParameterSlider {
                id: halationControl
                Layout.fillWidth: true
                theme: panel.theme
                photoReady: panel.photoReady
                selectedParameter: panel.selectedParameter
                parameterIndex: 16
                label: "Halation"
                from: 0
                to: 100
                initialValue: 0
                onSelectionRequested: index => panel.selectionRequested(index)
                onValueCommitted: value => panel.parameterCommitted("effects.halation", value)
            }

            ParameterSlider {
                id: fadeControl
                Layout.fillWidth: true
                theme: panel.theme
                photoReady: panel.photoReady
                selectedParameter: panel.selectedParameter
                parameterIndex: 17
                label: "Fade"
                from: 0
                to: 100
                initialValue: 0
                onSelectionRequested: index => panel.selectionRequested(index)
                onValueCommitted: value => panel.parameterCommitted("effects.fade", value)
            }

            ParameterSlider {
                id: grainControl
                Layout.fillWidth: true
                theme: panel.theme
                photoReady: panel.photoReady
                selectedParameter: panel.selectedParameter
                parameterIndex: 18
                label: "Grain"
                from: 0
                to: 100
                initialValue: 24
                expandable: true
                expanded: panel.advancedExpanded
                onSelectionRequested: index => panel.selectionRequested(index)
                onExpansionRequested: panel.advancedToggleRequested()
                onValueCommitted: value => panel.parameterCommitted("effects.grain.amount", value)
            }

            Rectangle {
                Layout.fillWidth: true
                implicitHeight: grainSubparameters.implicitHeight + 16
                visible: panel.advancedExpanded
                color: "transparent"
                border.width: 1
                border.color: panel.theme.lineColor

                ColumnLayout {
                    id: grainSubparameters
                    anchors.fill: parent
                    anchors.margins: 8
                    spacing: 6

                    ParameterSlider {
                        id: grainSizeControl
                        Layout.fillWidth: true
                        theme: panel.theme
                        photoReady: panel.photoReady
                        selectedParameter: panel.selectedParameter
                        parameterIndex: 19
                        label: "Size"
                        from: 20
                        to: 6400
                        initialValue: 4000
                        stepSize: 100
                        coarseStep: 500
                        suffix: " ISO"
                        onSelectionRequested: index => panel.selectionRequested(index)
                        onValueCommitted: value => panel.parameterCommitted("effects.grain.size_iso", value)
                    }

                    ParameterSlider {
                        id: midtonesControl
                        Layout.fillWidth: true
                        theme: panel.theme
                        photoReady: panel.photoReady
                        selectedParameter: panel.selectedParameter
                        parameterIndex: 20
                        label: "Midtones"
                        from: 0
                        to: 100
                        initialValue: 100
                        onSelectionRequested: index => panel.selectionRequested(index)
                        onValueCommitted: value => panel.parameterCommitted("effects.grain.midtone_response", value)
                    }
                }
            }

            ParameterSlider {
                id: vignetteControl
                Layout.fillWidth: true
                theme: panel.theme
                photoReady: panel.photoReady
                selectedParameter: panel.selectedParameter
                parameterIndex: 21
                label: "Vignette"
                from: -100
                to: 100
                initialValue: 0
                onSelectionRequested: index => panel.selectionRequested(index)
                onValueCommitted: value => panel.parameterCommitted("effects.vignette", value)
            }

            ParameterSlider {
                id: sharpnessControl
                Layout.fillWidth: true
                theme: panel.theme
                photoReady: panel.photoReady
                selectedParameter: panel.selectedParameter
                parameterIndex: 22
                label: "Sharpness"
                from: 0
                to: 100
                initialValue: 0
                onSelectionRequested: index => panel.selectionRequested(index)
                onValueCommitted: value => panel.parameterCommitted("effects.sharpness", value)
            }
        }
    }
}
