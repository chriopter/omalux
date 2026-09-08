import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

Item {
    id: root
    required property var theme
    required property var control
    required property real value
    required property bool editable
    property bool compact: false
    property bool moduleToggleAvailable: compact
    property string displayLabel: ""
    property bool qualifyLabel: true
    property bool darkVignette: false
    property string moduleIconKey: ""
    property string moduleName: ""
    property bool moduleEnabled: true
    signal moduleToggleRequested()
    property bool selected: false
    property bool detailsAvailable: false
    property bool detailsExpanded: false
    signal detailsRequested()
    signal selectedRequested()
    signal interactionChanged(bool active)
    signal edited(real value)
    signal resetRequested()
    implicitHeight: compact ? Math.max(48, controlLabel.implicitHeight + 28) : 52
    readonly property var colors: {
        switch (control.colors) {
        case "light": return [theme.ink, theme.ink]
        case "saturation": return ["#8a8a92", "#e05555"]
        case "temperature": return ["#5a8ad0", "#e0954a"]
        case "tint": return ["#d05ad0", "#5ac06a"]
        case "hue": return ["#d05a5a", "#d0b05a", "#5ac06a", "#4fc3c3", "#5a6fd0", "#c05ad0", "#d05a5a"]
        default: return [theme.ink, theme.ink]
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
    TapHandler {
        acceptedButtons: Qt.RightButton
        onTapped: (eventPoint, button) => actionMenu.popup(eventPoint.position.x, eventPoint.position.y)
    }
    Menu {
        id: actionMenu
        MenuItem {
            text: "Reset " + root.control.label
            enabled: root.editable
            onTriggered: root.resetRequested()
        }
        MenuItem {
            visible: root.moduleToggleAvailable
            text: (root.moduleEnabled ? "Disable " : "Enable ") + root.moduleName
            enabled: root.editable
            onTriggered: root.moduleToggleRequested()
        }
    }
    ColumnLayout {
        anchors.fill: parent
        anchors.rightMargin: root.compact ? 28 : 0
        spacing: 0
        RowLayout {
            id: titleRow
            Layout.fillWidth: true
            spacing: 3
            ToolButton {
                id: labelButton
                Layout.fillWidth: true
                padding: 0
                enabled: root.editable
                onClicked: { root.selectedRequested(); if (root.moduleToggleAvailable) root.moduleToggleRequested() }
                Accessible.name: root.moduleName + " · " + root.control.label
                Accessible.checkable: root.moduleToggleAvailable
                Accessible.checked: root.moduleEnabled
                background: Rectangle {
                    color: "transparent"
                    border.color: labelButton.activeFocus ? root.theme.accent : "transparent"
                }
                contentItem: RowLayout {
                    spacing: 5
                    Rectangle {
                        visible: root.moduleToggleAvailable && root.moduleEnabled
                        width: 4; height: 4; radius: 2
                        color: root.theme.accent
                    }
                    ModuleIcon {
                        visible: root.moduleIconKey !== ""
                        moduleKey: root.moduleIconKey
                    }
                    Text {
                        id: controlLabel
                        Layout.fillWidth: true
                        text: root.displayLabel || (root.compact && root.qualifyLabel && ["strength", "amount", "detail", "brightness"].includes(root.control.label) && root.moduleName !== "contrast brightness saturation"
                              ? root.moduleName + " · " + root.control.label : root.control.label)
                        color: root.selected || labelButton.hovered ? root.theme.accent : root.theme.ink
                        font: root.theme.settingsFont
                        wrapMode: Text.WordWrap
                        horizontalAlignment: Text.AlignLeft
                    }
                }

            }
            Text {
                id: valueLabel
                Layout.minimumWidth: root.compact ? 90 : implicitWidth
                Layout.maximumWidth: root.compact ? 90 : implicitWidth
                horizontalAlignment: Text.AlignRight
                MouseArea { anchors.fill: parent; onDoubleClicked: { numberInput.text=String(root.value);numberPopup.open();numberInput.forceActiveFocus();numberInput.selectAll() } }
                text: Number(Math.abs(root.value) < Math.pow(10, -root.control.decimals) / 2 ? 0 : root.value).toFixed(root.control.decimals) + root.control.unit
                color: root.selected ? root.theme.accent : root.theme.ink
                font: root.theme.settingsFont
            }
        }
        Slider {
            id: slider
            objectName: "control-slider-" + root.control.id
            live: true
            wheelEnabled: false
            leftPadding: 0; rightPadding: 0
            Layout.fillWidth: true
            Layout.preferredHeight: 22
            enabled: root.editable
            from: Math.min(root.control.softMinimum, root.value); to: Math.max(root.darkVignette ? 0 : root.control.softMaximum, root.value)
            stepSize: root.control.step
            value: root.value
            onPressedChanged: { root.interactionChanged(pressed); if (pressed) root.selectedRequested() }
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
                width: root.compact ? 10 : 8; height: width
                radius: root.compact ? width / 2 : 0
                border.width: root.compact ? 1 : 0
                border.color: root.selected ? root.theme.accent : root.theme.ink
                color: root.compact ? root.theme.background : (root.selected ? root.theme.accent : root.theme.ink)
            }
        }
    }
    ToolButton {
        id: detailsButton
        objectName: "control-details-" + root.control.id
        visible: root.detailsAvailable
        anchors.right: parent.right
        y: 1
        width: 24; height: 22; padding: 0
        onClicked: root.detailsRequested()
        Accessible.name: (root.detailsExpanded ? "Hide details for " : "Details for ") + root.control.label
        contentItem: Item {
            Canvas {
                anchors.centerIn: parent
                width: 10; height: 10
                rotation: root.detailsExpanded ? 90 : 0
                onPaint: {
                    const c = getContext("2d")
                    c.clearRect(0,0,width,height)
                    c.strokeStyle = root.theme.muted; c.lineWidth = 1.4
                    c.beginPath(); c.moveTo(3,1); c.lineTo(7,5); c.lineTo(3,9); c.stroke()
                }
            }
        }
        background: Rectangle {
            color: "transparent"
            border.color: detailsButton.activeFocus ? root.theme.accent : "transparent"
        }
    }

}
