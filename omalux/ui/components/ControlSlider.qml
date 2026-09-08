import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

Item {
    id: root
    required property var theme
    required property var control
    required property real value
    required property bool editable
    property bool selected: false
    signal selectedRequested()
    signal edited(real value)
    signal resetRequested()
    implicitHeight: 52
    readonly property var colors: {
        switch (control.colors) {
        case "light": return ["#17171c", "#eceaf2"]
        case "saturation": return ["#8a8a92", "#e05555"]
        case "temperature": return ["#5a8ad0", "#e0954a"]
        case "tint": return ["#d05ad0", "#5ac06a"]
        case "hue": return ["#d05a5a", "#d0b05a", "#5ac06a", "#4fc3c3", "#5a6fd0", "#c05ad0", "#d05a5a"]
        default: return [theme.line, theme.muted]
        }
    }
    Popup {
        id: numberPopup
        x: root.width-width; y: 0
        TextField {
            id: numberInput
            width: 110
            inputMethodHints: Qt.ImhFormattedNumbersOnly
            onAccepted: { const next=Number(text); if(Number.isFinite(next))root.edited(next);numberPopup.close() }
            Accessible.name: root.control.label + " value"
        }
    }
    HoverHandler { id: hover }
    ColumnLayout {
        anchors.fill: parent
        spacing: 0
        RowLayout {
            Layout.fillWidth: true
            Text {
                text: root.control.label
                color: root.selected ? root.theme.accent : root.theme.ink
                font: root.theme.textFont
                Layout.fillWidth: true
                wrapMode: Text.WordWrap
            }
            ToolButton {
                text: "↺"
                opacity: hover.hovered || activeFocus ? 1 : 0
                enabled: root.editable
                implicitWidth: 24; implicitHeight: 24
                onClicked: root.resetRequested()
                Accessible.name: "Reset " + root.control.label
                ToolTip.visible: hovered; ToolTip.text: "Reset value [R]"
            }
            Text {
                id: valueLabel
                MouseArea { anchors.fill: parent; onDoubleClicked: { numberInput.text=String(root.value);numberPopup.open();numberInput.forceActiveFocus();numberInput.selectAll() } }
                text: Number(Math.abs(root.value) < Math.pow(10, -root.control.decimals) / 2 ? 0 : root.value).toFixed(root.control.decimals) + root.control.unit
                color: root.selected ? root.theme.accent : root.theme.ink
                font: root.theme.textFont
            }
        }
        Slider {
            id: slider
            wheelEnabled: false
            Layout.fillWidth: true
            Layout.preferredHeight: 22
            enabled: root.editable
            from: Math.min(root.control.softMinimum, root.value); to: Math.max(root.control.softMaximum, root.value)
            stepSize: root.control.step
            value: root.value
            onPressedChanged: if (pressed) root.selectedRequested()
            onActiveFocusChanged: if (activeFocus) root.selectedRequested()
            onMoved: root.edited(value)
            Accessible.name: root.control.section + " · " + root.control.label
            background: Rectangle {
                x: slider.leftPadding; y: slider.topPadding + slider.availableHeight / 2
                width: slider.availableWidth; height: 3
                gradient: Gradient {
                    id: trackGradient
                    orientation: Gradient.Horizontal
                }
                Component { id: stopComponent; GradientStop {} }
                Component.onCompleted: {
                    const stops=[]
                    for (let i=0;i<root.colors.length;++i)
                        stops.push(stopComponent.createObject(trackGradient,{position:i/(root.colors.length-1),color:root.colors[i]}))
                    trackGradient.stops=stops
                }
            }
            handle: Rectangle {
                x: slider.leftPadding + slider.visualPosition * (slider.availableWidth - width)
                y: slider.topPadding + slider.availableHeight / 2 - height / 2
                width: 8; height: 8
                color: root.selected ? root.theme.accent : root.theme.ink
            }
        }
    }
}
