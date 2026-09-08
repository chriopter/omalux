import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import org.omalux

ScrollView {
    id: panel
    required property var theme
    required property bool photoReady
    required property string settingsJson
    signal geometryCommitted(string json)
    property var geometry: ({})
    property int selectedParameter: 0
    property var draftMargins: [0, 0, 0, 0]
    readonly property bool cropValid: draftMargins[0] + draftMargins[2] < 100
        && draftMargins[1] + draftMargins[3] < 100
    contentWidth: availableWidth
    clip: true

    function synchronize() {
        const settings = JSON.parse(settingsJson)
        geometry = settings.geometry
        const crop = geometry.crop || { x: 0, y: 0, width: 1, height: 1 }
        const values = [crop.x, crop.y, 1 - crop.x - crop.width, 1 - crop.y - crop.height]
        for (let i = 0; i < 4; ++i) {
            const control = margins.itemAt(i)
            if (control) control.value = Math.round(values[i] * 100)
        }
        angle.value = geometry.straighten_degrees
    }
    onSettingsJsonChanged: synchronize()
    Component.onCompleted: synchronize()

    function commit() { geometryCommitted(JSON.stringify(geometry)) }
    function rotate(direction) {
        geometry.quarter_turns_clockwise = (geometry.quarter_turns_clockwise + direction + 4) % 4
        commit()
    }
    function applyCrop() {
        if (!cropValid) return
        // Binary fractions keep opposite crop edges exact in the core's f32 settings.
        const unit = 1048576
        const left = Math.round(margins.itemAt(0).value / 100 * unit) / unit
        const top = Math.round(margins.itemAt(1).value / 100 * unit) / unit
        const right = Math.round(margins.itemAt(2).value / 100 * unit) / unit
        const bottom = Math.round(margins.itemAt(3).value / 100 * unit) / unit
        geometry.crop = { x: left, y: top, width: 1 - left - right, height: 1 - top - bottom }
        commit()
    }
    function selectedControl() {
        return selectedParameter === 4 ? angle : margins.itemAt(selectedParameter)
    }
    function moveSelection(direction) {
        const order = [4, 0, 1, 2, 3]
        selectedParameter = order[(order.indexOf(selectedParameter) + direction + 5) % 5]
    }
    function adjust(direction, coarse) { selectedControl().nudge(direction, coarse) }
    function resetSelected() { selectedControl().resetValue() }

    ColumnLayout {
        width: panel.availableWidth
        spacing: 12
        Text {
            text: "CROP & ROTATE"
            color: panel.theme.inkColor
            font.family: panel.theme.monoFont
            font.pixelSize: 13
            font.bold: true
        }
        RowLayout {
            TuiButton {
                theme: panel.theme
                text: "↶ 90°"
                enabled: panel.photoReady
                onClicked: panel.rotate(-1)
                Accessible.name: "Rotate counterclockwise"
            }
            TuiButton {
                theme: panel.theme
                text: "↷ 90°"
                enabled: panel.photoReady
                onClicked: panel.rotate(1)
                Accessible.name: "Rotate clockwise"
            }
        }
        ParameterSlider {
            id: angle
            Layout.fillWidth: true
            theme: panel.theme
            photoReady: panel.photoReady
            label: "Straighten"
            parameterIndex: 4
            selectedParameter: panel.selectedParameter
            from: -45
            to: 45
            stepSize: 0.1
            decimalPlaces: 1
            suffix: "°"
            initialValue: 0
            onSelectionRequested: index => panel.selectedParameter = index
            onValueCommitted: value => {
                panel.geometry.straighten_degrees = value
                panel.commit()
            }
        }
        Text {
            text: "CROP MARGINS"
            color: panel.theme.accentColor
            font.family: panel.theme.monoFont
            font.bold: true
        }
        Repeater {
            id: margins
            model: ["Left", "Top", "Right", "Bottom"]
            delegate: ParameterSlider {
                required property int index
                required property string modelData
                Layout.fillWidth: true
                theme: panel.theme
                photoReady: panel.photoReady
                label: modelData
                parameterIndex: index
                selectedParameter: panel.selectedParameter
                from: 0
                to: 99
                suffix: "%"
                initialValue: 0
                onSelectionRequested: index => panel.selectedParameter = index
                onValueChanged: {
                    const values = panel.draftMargins.slice()
                    values[index] = value
                    panel.draftMargins = values
                }
            }
        }
        TuiButton {
            theme: panel.theme
            text: "APPLY CROP"
            enabled: panel.photoReady && panel.cropValid
            onClicked: panel.applyCrop()
        }
        Label {
            visible: !panel.cropValid
            Layout.fillWidth: true
            text: "Opposite margins must total less than 100%."
            wrapMode: Text.Wrap
        }
        ResetButton {
            theme: panel.theme
            text: "RESET CROP & ROTATION"
            enabled: panel.photoReady
            onClicked: {
                panel.geometry.crop = null
                panel.geometry.quarter_turns_clockwise = 0
                panel.geometry.straighten_degrees = 0
                panel.commit()
            }
        }
    }
}
