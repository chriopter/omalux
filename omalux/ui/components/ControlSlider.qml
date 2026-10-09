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
    // A parameter of an unfolded module, under the module's kept main row: quieter type and a
    // smaller knob, so it reads as a part of that row.
    property bool sub: false
    // The sub-row of the very parameter the main row drives: both show one value and move
    // together. A tick ties it to the guide line of the sub-rows, and it is not a keyboard stop
    // of its own (the main row is).
    property bool linked: false
    readonly property color ink: sub && !linked ? theme.subInk : theme.ink
    // Every slider row offers its reset while the pointer is on it or it is the keyboard
    // selection: one parameter back to its default, as R does (on a kept main row too; the
    // module's reset is in its heading or strip, and on Shift+R). `defaultValue` is that default
    // in displayed units when the owner knows it: the button stays away while the value is
    // already there.
    // The name as the row shows it.
    readonly property string labelText: controlLabel.text
    // The value as the row shows it, with darktable's digits and unit.
    readonly property string valueText: Number(Math.abs(value) < Math.pow(10, -control.decimals) / 2 ? 0 : value).toFixed(control.decimals) + control.unit
    property var defaultValue: undefined
    readonly property bool hot: rowHover.hovered || navTarget.current
    readonly property bool atDefault: defaultValue !== undefined && defaultValue !== null
                                      && Math.abs(value - defaultValue) < Math.pow(10, -Math.max(0, control.decimals)) / 2
    readonly property font labelFont: sub ? theme.textFont : theme.settingsFont
    property bool moduleToggleAvailable: compact
    property string displayLabel: ""
    property bool qualifyLabel: true
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
    // Keyboard (see NavTarget): Enter, Shift+R and a direct shortcut to this hidden row.
    signal activated()
    signal moduleResetRequested()
    signal revealRequested()
    property alias navTarget: navTarget
    // Selected by the panel (active control) or by the keyboard.
    readonly property bool marked: selected || navTarget.current
    NavTarget {
        id: navTarget
        navId: root.control.id + (root.linked ? "/@linked" : "")
        listed: !root.linked
        label: root.displayLabel || root.control.label
        kind: "slider"
        enabled: root.editable
        active: root.selected
        activateLabel: root.detailsAvailable ? (root.detailsExpanded ? "COLLAPSE" : "EXPAND") : ""
        onAdjust: steps => root.step(steps)
        onReset: root.resetRequested()
        onActivate: root.detailsAvailable ? root.detailsRequested() : root.activated()
        onResetGroup: root.moduleResetRequested()
        onToggleGroup: root.moduleToggleRequested()
        onSelected: root.selectedRequested()
        onRevealRequested: root.revealRequested()
    }
    // darktable's own step for the arrow keys (bauhaus.c, dt_bauhaus_slider_get_step with
    // bauhaus/zoom_step on, as shipped): about a hundredth of the range the slider shows, as 1 or
    // 5 of a decade of the displayed value; one native unit once that range reaches 100.
    readonly property real keyStep: {
        const f = Math.abs(root.control.factor || 1), o = root.control.offset || 0
        const a = (slider.from - o) / f, b = (slider.to - o) / f
        const top = Math.min(b - a, Math.max(Math.abs(a), Math.abs(b)))
        if (!(top > 0)) return root.control.step
        if (top >= 100) return f
        const lg = Math.log10(top * f / 100), whole = Math.floor(lg + .1)
        return Math.pow(10, whole) * (lg - whole > .5 ? 5 : 1)
    }
    // Values are stored as darktable stores them: rounded to the digits it displays.
    function shown(value) { return Number(Number(value).toFixed(Math.max(0, root.control.decimals))) }
    // One keyboard step, within the hard range; whole steps stay whole for integer parameters.
    function step(steps) {
        if (!root.editable) return
        let delta = steps * root.keyStep
        if (root.control.decimals === 0) delta = Math.sign(delta) * Math.max(1, Math.round(Math.abs(delta)))
        const low = root.control.minimum !== undefined ? root.control.minimum : -Infinity
        const high = root.control.maximum !== undefined ? root.control.maximum : Infinity
        const next = Math.max(low, Math.min(high, root.shown(root.value + delta)))
        if (next !== root.value) root.edited(next)
    }
    implicitHeight: sub ? Math.max(44, controlLabel.implicitHeight + 28) : compact ? Math.max(48, controlLabel.implicitHeight + 28) : 52
    readonly property var colors: {
        switch (control.colors) {
        // Dark to light: exposure, brightness, shadows and highlights, tone zones.
        case "light": return [theme.line, "#a6adc8", "#ffffff"]
        case "saturation": return ["#8a8a92", "#e05555"]
        case "temperature": return ["#5a8ad0", "#e0954a"]
        case "tint": return ["#d05ad0", "#5ac06a"]
        // Color contrast along one opponent axis (Lab a or b): from flat to both colours.
        case "green-magenta": return ["#7a7a84", "#5ac06a", "#d05ad0"]
        case "blue-yellow": return ["#7a7a84", "#5a8ad0", "#d8c050"]
        case "hue": return ["#d05a5a", "#d0b05a", "#5ac06a", "#4fc3c3", "#5a6fd0", "#c05ad0", "#d05a5a"]
        default: return [theme.ink, theme.ink]
        }
    }
    // Typing a value (double-click on it) and the right-click menu are built when first used.
    OnDemand {
        id: numberPopup
        parent: root
        Popup {
            id: popup
            x: root.width-width; y: 0
            function edit() { numberInput.text=String(root.shown(root.value));open();numberInput.forceActiveFocus();numberInput.selectAll() }
            TextField {
                id: numberInput
                width: 110
                inputMethodHints: Qt.ImhFormattedNumbersOnly
                onAccepted: { const next=Number(text); if(Number.isFinite(next))root.edited(next);popup.close() }
                Accessible.name: root.control.label + " value"
            }
        }
    }
    TapHandler {
        acceptedButtons: Qt.RightButton
        onTapped: (eventPoint, button) => actionMenu.get().popup(eventPoint.position.x, eventPoint.position.y)
    }
    OnDemand {
        id: actionMenu
        parent: root
        Menu {
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
                onClicked: { navTarget.claim(); root.selectedRequested(); if (root.moduleToggleAvailable) root.moduleToggleRequested() }
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
                    // Only curated rows carry a module icon; generated rows skip building one.
                    Loader {
                        active: root.moduleIconKey !== ""
                        visible: active
                        sourceComponent: ModuleIcon { moduleKey: root.moduleIconKey }
                    }
                    Text {
                        id: controlLabel
                        Layout.fillWidth: true
                        text: root.displayLabel || (root.compact && root.qualifyLabel && ["strength", "amount", "detail", "brightness"].includes(root.control.label) && root.moduleName !== "contrast brightness saturation"
                              ? root.moduleName + " · " + root.control.label : root.control.label)
                        color: root.marked || labelButton.hovered ? root.theme.accent : root.ink
                        font: root.labelFont
                        wrapMode: Text.WordWrap
                        horizontalAlignment: Text.AlignLeft
                    }
                }

            }
            Text {
                id: valueLabel
                // Compact rows keep one value column; long values may widen it.
                Layout.preferredWidth: root.compact ? Math.max(64, implicitWidth) : implicitWidth
                horizontalAlignment: Text.AlignRight
                MouseArea { anchors.fill: parent; onDoubleClicked: numberPopup.get().edit() }
                text: root.valueText
                color: root.marked ? root.theme.accent : root.ink
                font: root.labelFont
            }
        }
        Slider {
            id: slider
            objectName: "control-slider-" + root.control.id + (root.linked ? "/@linked" : "")
            live: true
            wheelEnabled: false
            leftPadding: 0; rightPadding: 0
            Layout.fillWidth: true
            Layout.preferredHeight: 22
            enabled: root.editable
            from: Math.min(root.control.softMinimum, root.value); to: Math.max(root.control.softMaximum, root.value)
            stepSize: root.control.step
            value: root.value
            onPressedChanged: { root.interactionChanged(pressed); if (pressed) { navTarget.claim(); root.selectedRequested() } }
            onActiveFocusChanged: if (activeFocus) root.selectedRequested()
            onMoved: root.edited(root.shown(value))
            Accessible.name: root.control.section + " · " + root.control.label
            // Track and knob share one centre line: both are placed with the same integer
            // rounding, the track 3 px and the knob an odd size, so their centres coincide
            // on whole or half pixels alike at every scale.
            readonly property int trackHeight: 3
            readonly property int trackY: slider.topPadding + Math.floor((slider.availableHeight - trackHeight) / 2)
            readonly property real centerY: trackY + trackHeight / 2
            background: Rectangle {
                x: slider.leftPadding; y: slider.trackY
                width: slider.availableWidth; height: slider.trackHeight
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
                x: slider.leftPadding + Math.round(slider.visualPosition * (slider.availableWidth - width))
                y: slider.centerY - height / 2
                width: root.compact && !root.sub ? 11 : 9; height: width
                radius: root.compact ? width / 2 : 0
                border.width: root.compact ? 1 : 0
                border.color: root.marked ? root.theme.accent : root.ink
                color: root.compact ? root.theme.background : (root.marked ? root.theme.accent : root.theme.ink)
            }
        }
    }
    HoverHandler { id: rowHover }
    // Laid over the free start of the value column, so nothing moves when it appears; clear of
    // the slider (drag) and of the disclosure column (chevron, picker).
    Loader {
        active: root.hot && root.editable && !root.atDefault
        visible: active
        x: titleRow.x + valueLabel.x + valueLabel.width - valueLabel.contentWidth - width - 3
        y: titleRow.y + Math.round((titleRow.height - height) / 2)
        width: 20; height: 18
        sourceComponent: ResetButton {
            objectName: "control-reset-" + root.control.id + (root.linked ? "/@linked" : "")
            theme: root.theme
            title: root.control.label
            ToolTip.text: "reset (R)"
            // A small raised chip: it stays readable where a long label reaches under it.
            background: Rectangle {
                radius: 3
                color: parent.pressed ? root.theme.active : root.theme.hover
                border.width: 1
                border.color: parent.hovered ? root.theme.muted : root.theme.line
            }
            onClicked: { navTarget.claim(); root.resetRequested() }
        }
    }
    // A linked sub-row: a tick from the guide line (13 px to the left) towards the label.
    Loader {
        active: root.linked
        visible: active
        x: -13
        y: titleRow.y + Math.round(titleRow.height / 2) - 2
        sourceComponent: Item {
            objectName: "control-linked-" + root.control.id
            width: 9; height: 5
            Rectangle { y: 2; width: 6; height: 1; color: root.theme.ink; opacity: .8 }
            Rectangle { x: 4; width: 5; height: 5; radius: 2.5; color: root.theme.ink }
        }
    }
    // Only rows with details have the chevron; the others do not build one.
    Loader {
        active: root.detailsAvailable
        visible: active
        anchors.right: parent.right
        y: titleRow.y + (titleRow.height - height) / 2
        sourceComponent: DisclosureButton {
            objectName: "control-details-" + root.control.id
            theme: root.theme
            expanded: root.detailsExpanded
            onClicked: root.detailsRequested()
            Accessible.name: (root.detailsExpanded ? "Hide details for " : "Details for ") + root.control.label
        }
    }

}
