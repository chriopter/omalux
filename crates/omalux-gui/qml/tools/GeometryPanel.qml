import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import org.omalux

ScrollView {
    id: panel
    required property var theme
    required property bool photoReady
    required property string settingsJson
    property bool editing: false
    property real imageAspect: 1
    property var geometry: ({})
    property var crop: ({ x:0, y:0, width:1, height:1 })
    property real draftAngle: 0
    property string originalGeometry: ""
    property int selectedParameter: 0
    property bool finishing: false
    readonly property real aspectRatio: ratios.currentIndex === 1 ? imageAspect
        : [0, 0, 1, 3/2, 4/3, 4/5, 16/9][ratios.currentIndex] * (portrait.checked ? -1 : 1)
    readonly property real lockedRatio: aspectRatio < 0 ? -1/aspectRatio : aspectRatio
    signal geometryCommitted(string json)
    signal finished()
    contentWidth: availableWidth
    clip: true

    function synchronize() {
        geometry = JSON.parse(settingsJson).geometry
        crop = geometry.crop || { x:0, y:0, width:1, height:1 }
        draftAngle = geometry.straighten_degrees
        angle.value = draftAngle
    }
    onSettingsJsonChanged: synchronize()
    Component.onCompleted: synchronize()
    onEditingChanged: {
        if (editing) {
            originalGeometry = JSON.stringify(JSON.parse(settingsJson).geometry)
            ratios.currentIndex = 0
            synchronize()
        } else if (!finishing && originalGeometry) {
            applyGeometry()
        }
        finishing = false
    }
    function applyGeometry() {
        // Quantize edges once, when committing, to remain exact in f32 settings.
        const unit = 1048576
        const l = Math.round(crop.x*unit)/unit, t = Math.round(crop.y*unit)/unit
        const r = Math.round((crop.x+crop.width)*unit)/unit
        const b = Math.round((crop.y+crop.height)*unit)/unit
        geometry.crop = l === 0 && t === 0 && r === 1 && b === 1 ? null
            : { x:l, y:t, width:r-l, height:b-t }
        geometry.straighten_degrees = draftAngle
        geometryCommitted(JSON.stringify(geometry))
    }
    function applyCrop() { applyGeometry(); finishing = true; finished() }
    function cancel() { finishing = true; geometryCommitted(originalGeometry); finished() }
    function rotate(direction) {
        const r = crop
        crop = direction > 0 ? { x:1-r.y-r.height, y:r.x, width:r.height, height:r.width }
            : { x:r.y, y:1-r.x-r.width, width:r.height, height:r.width }
        geometry.quarter_turns_clockwise = (geometry.quarter_turns_clockwise+direction+4)%4
        ratios.currentIndex = 0
        applyGeometry()
    }
    function constrainAspect() {
        if (lockedRatio <= 0) return
        const ratio = lockedRatio/imageAspect
        let w = crop.width, h = crop.height
        if (w/h > ratio) w = h*ratio
        else h = w/ratio
        crop = { x:crop.x+(crop.width-w)/2, y:crop.y+(crop.height-h)/2, width:w, height:h }
    }
    function moveSelection(direction) { selectedParameter = 0 }
    function adjust(direction, coarse) { angle.nudge(direction, coarse) }
    function resetSelected() { angle.resetValue() }

    ColumnLayout {
        width: panel.availableWidth
        spacing: 20
        Text {
            text: "CROP & ROTATE"
            color: panel.theme.inkColor
            font.family: panel.theme.monoFont
            font.pixelSize: 13; font.bold: true
        }
        Text {
            Layout.fillWidth: true
            text: "Drag the frame or its handles in the photograph."
            wrapMode: Text.Wrap
            color: panel.theme.mutedColor
            font.family: panel.theme.monoFont; font.pixelSize: 11
        }
        RowLayout {
            Layout.fillWidth: true
            ComboBox {
                id: ratios
                objectName: "cropAspect"
                Layout.fillWidth: true
                model: ["Free", "Original", "1:1", "3:2", "4:3", "4:5", "16:9"]
                enabled: panel.photoReady
                onActivated: panel.constrainAspect()
                Accessible.name: "Crop aspect ratio"
            }
            TuiButton {
                id: portrait
                theme: panel.theme; text: "⇄"
                checkable: true
                enabled: panel.photoReady && ratios.currentIndex > 1
                onClicked: panel.constrainAspect()
                Accessible.name: "Swap aspect ratio orientation"
            }
        }
        RowLayout {
            Layout.fillWidth: true
            TuiButton {
                Layout.fillWidth: true
                theme: panel.theme; text: "↶ 90°"
                enabled: panel.photoReady
                onClicked: panel.rotate(-1)
                Accessible.name: "Rotate counterclockwise"
            }
            TuiButton {
                Layout.fillWidth: true
                theme: panel.theme; text: "↷ 90°"
                enabled: panel.photoReady
                onClicked: panel.rotate(1)
                Accessible.name: "Rotate clockwise"
            }
        }
        ParameterSlider {
            id: angle
            objectName: "straightenSlider"
            Layout.fillWidth: true
            theme: panel.theme; photoReady: panel.photoReady
            label: "Straighten"; parameterIndex: 0
            selectedParameter: panel.selectedParameter
            from: -45; to: 45
            stepSize: 0.1; coarseStep: 5
            decimalPlaces: 1; suffix: "°"; initialValue: 0
            onValueCommitted: value => panel.draftAngle = value
        }
        Text {
            Layout.fillWidth: true
            text: "Drag to rotate · ← / → 0.1°\nShift + ← / → 5°"
            wrapMode: Text.Wrap
            color: panel.theme.mutedColor
            font.family: panel.theme.monoFont; font.pixelSize: 10
        }
        RowLayout {
            Layout.fillWidth: true
            TuiButton {
                Layout.fillWidth: true
                theme: panel.theme; text: "APPLY"; primary: true
                enabled: panel.photoReady
                onClicked: panel.applyCrop()
            }
            TuiButton {
                Layout.fillWidth: true
                theme: panel.theme; text: "CANCEL"
                enabled: panel.photoReady
                onClicked: panel.cancel()
            }
        }
        ResetButton {
            theme: panel.theme; text: "RESET CROP & ROTATION"
            enabled: panel.photoReady
            onClicked: {
                panel.crop = { x:0, y:0, width:1, height:1 }
                panel.draftAngle = 0; angle.value = 0
                panel.geometry.quarter_turns_clockwise = 0
                ratios.currentIndex = 0
                panel.applyGeometry()
            }
        }
    }
}
