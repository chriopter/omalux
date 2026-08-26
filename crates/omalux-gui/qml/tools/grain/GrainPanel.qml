import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import io.omacom.omalux

Item {
    id: panel

    required property var theme
    required property bool photoReady
    required property int selectedParameter
    required property bool advancedExpanded
    required property string settingsJson
    required property string supportedParametersJson

    readonly property int parameterCount: 24
    property alias grainValue: grainControl.value
    property alias grainSizeValue: grainSizeControl.value
    property alias midtonesValue: midtonesControl.value

    signal selectionRequested(int index)
    signal advancedToggleRequested
    signal parameterCommitted(string id, real value)

    readonly property var supportedParameterIds: {
        try {
            return JSON.parse(supportedParametersJson)
        } catch (error) {
            return []
        }
    }

    function parameterSupported(id) {
        return supportedParameterIds.indexOf(id) >= 0
    }

    function synchronizeSettings() {
        var settings
        try {
            settings = JSON.parse(settingsJson)
        } catch (error) {
            return
        }
        var basicsValues = [settings.basics.exposure_ev,
            settings.basics.brightness, settings.basics.contrast,
            settings.basics.clarity, settings.basics.highlights, settings.basics.shadows,
            settings.basics.whites, settings.basics.blacks]
        for (var basicIndex = 0; basicIndex < basicsValues.length; ++basicIndex) {
            var basic = basicsRepeater.itemAt(basicIndex)
            if (basic)
                basic.value = basicsValues[basicIndex]
        }
        var highlightHue = settings.color_grading.highlights.hue_degrees
        var shadowHue = settings.color_grading.shadows.hue_degrees
        var colorValues = [settings.basics.saturation, settings.basics.vibrance,
            settings.basics.temperature, settings.basics.tint,
            settings.color_grading.highlights.saturation,
            highlightHue > 180 ? highlightHue - 360 : highlightHue,
            settings.color_grading.shadows.saturation,
            shadowHue > 180 ? shadowHue - 360 : shadowHue]
        for (var colorIndex = 0; colorIndex < colorValues.length; ++colorIndex) {
            var color = colorRepeater.itemAt(colorIndex)
            if (color)
                color.value = colorValues[colorIndex]
        }
        bloomControl.value = settings.effects.bloom
        halationControl.value = settings.effects.halation
        fadeControl.value = settings.effects.fade
        grainControl.value = settings.effects.grain.amount
        grainSizeControl.value = settings.effects.grain.size_iso
        midtonesControl.value = settings.effects.grain.midtone_response
        vignetteControl.value = settings.effects.vignette
        sharpnessControl.value = settings.effects.sharpness
    }

    onSettingsJsonChanged: synchronizeSettings()
    Component.onCompleted: synchronizeSettings()

    readonly property var luminanceTrack: ["#17171c", "#eceaf2"]
    readonly property var basics: [
        { "label": "Exposure", "from": -5, "to": 5, "suffix": " EV", "track": luminanceTrack },
        { "label": "Brightness", "from": -300, "to": 300, "track": luminanceTrack },
        { "label": "Contrast", "from": -200, "to": 200 },
        { "label": "Clarity", "from": -200, "to": 200 },
        { "label": "Highlights", "from": -150, "to": 150, "track": ["#55555e", "#eceaf2"] },
        { "label": "Shadows", "from": -150, "to": 150, "track": ["#17171c", "#9a9aa4"] },
        { "label": "Whites", "from": -150, "to": 150, "track": ["#55555e", "#ffffff"] },
        { "label": "Blacks", "from": -150, "to": 150, "track": ["#000000", "#9a9aa4"] }
    ]

    readonly property var hueTrack: ["#4fc3c3", "#5a6fd0", "#c05ad0", "#d05a5a", "#d0b05a", "#5ac06a", "#4fc3c3"]
    readonly property var colors: [
        { "label": "Saturation", "from": -100, "to": 200, "suffix": "", "track": ["#8a8a92", "#e05555"] },
        { "label": "Vibrance", "from": -100, "to": 200, "suffix": "", "track": ["#8a8a92", "#e07555"] },
        { "label": "Temperature", "from": -150, "to": 150, "suffix": "", "track": ["#5a8ad0", "#e0954a"] },
        { "label": "Tint", "from": -150, "to": 150, "suffix": "", "track": ["#5ac06a", "#d05ad0"] },
        { "label": "Highlight amount", "from": 0, "to": 200, "suffix": "", "track": ["#8a8a92", "#e0a555"] },
        { "label": "Highlight color", "from": -180, "to": 180, "suffix": "°", "track": hueTrack },
        { "label": "Shadow amount", "from": 0, "to": 200, "suffix": "", "track": ["#8a8a92", "#e0a555"] },
        { "label": "Shadow color", "from": -180, "to": 180, "suffix": "°", "track": hueTrack }
    ]

    function parameterAt(index) {
        if (index < 8)
            return basicsRepeater.itemAt(index)
        if (index < 16)
            return colorRepeater.itemAt(index - 8)
        switch (index) {
        case 16: return bloomControl
        case 17: return halationControl
        case 18: return fadeControl
        case 19: return grainControl
        case 20: return grainSizeControl
        case 21: return midtonesControl
        case 22: return vignetteControl
        default: return sharpnessControl
        }
    }

    function navigationOrder() {
        const result = []
        for (let index = 0; index < parameterCount; ++index) {
            if (advancedExpanded || (index !== 20 && index !== 21))
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
        Layout.topMargin: 14
        Layout.bottomMargin: 4
        color: panel.theme.accentColor
        font.family: panel.theme.monoFont
        font.pixelSize: 15
        font.bold: true
        font.letterSpacing: 1.2
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
                    suffix: modelData.suffix || ""
                    trackColors: modelData.track || []
                    initialValue: 0
                    supported: panel.parameterSupported([
                        "basics.exposure_ev", "basics.brightness", "basics.contrast", "basics.clarity",
                        "basics.highlights", "basics.shadows", "basics.whites",
                        "basics.blacks"
                    ][index])
                    onSelectionRequested: requestedIndex => panel.selectionRequested(requestedIndex)
                    onValueCommitted: value => panel.parameterCommitted([
                        "basics.exposure_ev", "basics.brightness", "basics.contrast", "basics.clarity",
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
                    parameterIndex: index + 8
                    label: modelData.label
                    from: modelData.from
                    to: modelData.to
                    suffix: modelData.suffix
                    trackColors: modelData.track || []
                    initialValue: 0
                    supported: panel.parameterSupported([
                        "basics.saturation", "basics.vibrance", "basics.temperature",
                        "basics.tint", "color_grading.highlights.saturation",
                        "color_grading.highlights.hue_degrees",
                        "color_grading.shadows.saturation",
                        "color_grading.shadows.hue_degrees"
                    ][index])
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
                parameterIndex: 16
                label: "Bloom"
                from: 0
                to: 200
                initialValue: 0
                supported: panel.parameterSupported("effects.bloom")
                onSelectionRequested: index => panel.selectionRequested(index)
                onValueCommitted: value => panel.parameterCommitted("effects.bloom", value)
            }

            ParameterSlider {
                id: halationControl
                Layout.fillWidth: true
                theme: panel.theme
                photoReady: panel.photoReady
                selectedParameter: panel.selectedParameter
                parameterIndex: 17
                label: "Halation"
                from: 0
                to: 200
                initialValue: 0
                supported: panel.parameterSupported("effects.halation")
                onSelectionRequested: index => panel.selectionRequested(index)
                onValueCommitted: value => panel.parameterCommitted("effects.halation", value)
            }

            ParameterSlider {
                id: fadeControl
                Layout.fillWidth: true
                theme: panel.theme
                photoReady: panel.photoReady
                selectedParameter: panel.selectedParameter
                parameterIndex: 18
                label: "Fade"
                from: 0
                to: 200
                initialValue: 0
                supported: panel.parameterSupported("effects.fade")
                onSelectionRequested: index => panel.selectionRequested(index)
                onValueCommitted: value => panel.parameterCommitted("effects.fade", value)
            }

            ParameterSlider {
                id: grainControl
                Layout.fillWidth: true
                theme: panel.theme
                photoReady: panel.photoReady
                selectedParameter: panel.selectedParameter
                parameterIndex: 19
                label: "Grain"
                from: 0
                to: 150
                initialValue: 24
                supported: panel.parameterSupported("effects.grain.amount")
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
                        parameterIndex: 20
                        label: "Size"
                        from: 20
                        to: 12800
                        initialValue: 4000
                        stepSize: 100
                        coarseStep: 500
                        suffix: " ISO"
                        supported: panel.parameterSupported("effects.grain.size_iso")
                        onSelectionRequested: index => panel.selectionRequested(index)
                        onValueCommitted: value => panel.parameterCommitted("effects.grain.size_iso", value)
                    }

                    ParameterSlider {
                        id: midtonesControl
                        Layout.fillWidth: true
                        theme: panel.theme
                        photoReady: panel.photoReady
                        selectedParameter: panel.selectedParameter
                        parameterIndex: 21
                        label: "Midtones"
                        from: 0
                        to: 100
                        initialValue: 100
                        supported: panel.parameterSupported("effects.grain.midtone_response")
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
                parameterIndex: 22
                label: "Vignette"
                from: -150
                to: 150
                initialValue: 0
                supported: panel.parameterSupported("effects.vignette")
                onSelectionRequested: index => panel.selectionRequested(index)
                onValueCommitted: value => panel.parameterCommitted("effects.vignette", value)
            }

            ParameterSlider {
                id: sharpnessControl
                Layout.fillWidth: true
                theme: panel.theme
                photoReady: panel.photoReady
                selectedParameter: panel.selectedParameter
                parameterIndex: 23
                label: "Sharpness"
                from: 0
                to: 150
                initialValue: 0
                supported: panel.parameterSupported("effects.sharpness")
                onSelectionRequested: index => panel.selectionRequested(index)
                onValueCommitted: value => panel.parameterCommitted("effects.sharpness", value)
            }
        }
    }
}
