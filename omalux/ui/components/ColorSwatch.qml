import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

// A colour parameter (borders frame colour, colorize, split-toning, ...): a row with the
// darktable label and a swatch that opens a compact picker. The picker has a hue ×
// saturation field, a value slider and a hex field. `color` is r, g, b in 0..1 as an array,
// an {r, g, b} object or a QML color; colorEdited always reports [r, g, b]. Right-click the
// swatch for the reset.
Item {
    id: root
    required property var theme
    property var color: [0.5, 0.5, 0.5]
    property string label: ""
    property bool editable: true
    signal colorEdited(var rgb)
    signal interactionChanged(bool active)
    signal resetRequested()
    // Keyboard: Enter opens the picker, R resets (see NavTarget).
    property alias navTarget: navTarget
    NavTarget {
        id: navTarget
        navId: "color"
        label: root.label || "colour"
        kind: "button"
        activateLabel: "PICK COLOUR"
        resettable: true
        enabled: root.editable
        onActivate: root.openPicker()
        onReset: root.resetRequested()
    }

    readonly property var rgb: {
        const c = root.color
        if (c === undefined || c === null) return [0, 0, 0]
        if (Array.isArray(c) || (typeof c === "object" && c.length === 3)) return [Number(c[0]), Number(c[1]), Number(c[2])]
        return [Number(c.r), Number(c.g), Number(c.b)]
    }
    readonly property color qcolor: Qt.rgba(clamp(rgb[0]), clamp(rgb[1]), clamp(rgb[2]), 1)
    readonly property string hex: toHex(rgb)

    // Picker state in HSV, kept separately so hue survives grey and black.
    property real hue: 0
    property real sat: 0
    property real val: 0
    property bool dragging: false

    implicitWidth: 260
    implicitHeight: 26

    function clamp(v) { return Math.max(0, Math.min(1, v)) }
    function toHex(c) {
        return "#" + c.map(v => Math.round(clamp(v) * 255).toString(16).padStart(2, "0")).join("")
    }
    function syncFromColor() {
        const r = clamp(rgb[0]), g = clamp(rgb[1]), b = clamp(rgb[2])
        const max = Math.max(r, g, b), min = Math.min(r, g, b), d = max - min
        val = max
        if (max > 0) sat = d / max
        if (d > 0) {
            let h = max === r ? (g - b) / d : max === g ? 2 + (b - r) / d : 4 + (r - g) / d
            hue = ((h / 6) % 1 + 1) % 1
        }
    }
    function hsvToRgb(h, s, v) {
        const i = Math.floor(h * 6) % 6, f = h * 6 - Math.floor(h * 6)
        const p = v * (1 - s), q = v * (1 - f * s), t = v * (1 - (1 - f) * s)
        return [[v, t, p], [q, v, p], [p, v, t], [p, q, v], [t, p, v], [v, p, q]][i]
    }
    function emitHsv() { root.colorEdited(hsvToRgb(hue, sat, val)) }
    function openPicker() { if (editable) popup.open() }
    onRgbChanged: if (!dragging) syncFromColor()
    Component.onCompleted: syncFromColor()

    RowLayout {
        anchors.fill: parent
        spacing: 8
        Text {
            Layout.fillWidth: true
            text: root.label
            color: root.theme.muted
            font: root.theme.textFont
            elide: Text.ElideRight
        }
        Text {
            text: root.hex
            color: root.editable ? root.theme.ink : root.theme.muted
            font: root.theme.textFont
        }
        Button {
            id: swatch
            implicitWidth: 34
            implicitHeight: 16
            padding: 0
            hoverEnabled: true
            enabled: root.editable
            onClicked: popup.opened ? popup.close() : popup.open()
            Accessible.name: (root.label || "colour") + " " + root.hex
            background: Rectangle {
                radius: 4
                color: root.qcolor
                border.width: 1
                border.color: swatch.hovered || swatch.visualFocus || popup.opened || navTarget.current ? root.theme.accent : root.theme.line
                opacity: root.editable ? 1 : 0.5
            }
            TapHandler {
                acceptedButtons: Qt.RightButton
                onTapped: menu.popup()
            }
        }
    }

    Menu {
        id: menu
        MenuItem { text: "Reset " + (root.label || "colour"); enabled: root.editable; onTriggered: root.resetRequested() }
    }

    Popup {
        id: popup
        x: root.width - width
        y: root.height + 4
        width: 220
        padding: 10
        // Takes the keys while open, so Escape closes it (the navigator leaves popups alone).
        focus: true
        onOpened: root.syncFromColor()
        background: Rectangle {
            radius: 6
            color: root.theme.background
            border.color: root.theme.line
        }
        contentItem: ColumnLayout {
            spacing: 8
            // Hue across, saturation up.
            Item {
                id: field
                Layout.fillWidth: true
                Layout.preferredHeight: 120
                Rectangle {
                    anchors.fill: parent
                    radius: 3
                    gradient: Gradient {
                        orientation: Gradient.Horizontal
                        GradientStop { position: 0 / 6; color: "#ff0000" }
                        GradientStop { position: 1 / 6; color: "#ffff00" }
                        GradientStop { position: 2 / 6; color: "#00ff00" }
                        GradientStop { position: 3 / 6; color: "#00ffff" }
                        GradientStop { position: 4 / 6; color: "#0000ff" }
                        GradientStop { position: 5 / 6; color: "#ff00ff" }
                        GradientStop { position: 6 / 6; color: "#ff0000" }
                    }
                }
                Rectangle {
                    anchors.fill: parent
                    radius: 3
                    gradient: Gradient {
                        GradientStop { position: 0; color: "#00ffffff" }
                        GradientStop { position: 1; color: "#ffffffff" }
                    }
                }
                Rectangle {   // darkens the field to the current value, as the result will look
                    anchors.fill: parent
                    radius: 3
                    color: "black"
                    opacity: 1 - Math.max(0.25, root.val)
                    border.color: root.theme.line
                }
                Rectangle {
                    width: 10; height: 10; radius: 5
                    x: root.hue * field.width - width / 2
                    y: (1 - root.sat) * field.height - height / 2
                    color: root.qcolor
                    border.color: "white"; border.width: 1.5
                }
                MouseArea {
                    anchors.fill: parent
                    preventStealing: true
                    function pick(m) {
                        root.hue = Math.max(0, Math.min(0.9999, m.x / width))
                        root.sat = Math.max(0, Math.min(1, 1 - m.y / height))
                        if (root.val === 0) root.val = 1
                        root.emitHsv()
                    }
                    onPressed: m => { root.dragging = true; root.interactionChanged(true); pick(m) }
                    onPositionChanged: m => { if (pressed) pick(m) }
                    onReleased: { root.dragging = false; root.interactionChanged(false) }
                }
            }
            // Value.
            Item {
                id: valueTrack
                Layout.fillWidth: true
                Layout.preferredHeight: 14
                Rectangle {
                    anchors.verticalCenter: parent.verticalCenter
                    width: parent.width; height: 6; radius: 3
                    border.color: root.theme.line
                    gradient: Gradient {
                        orientation: Gradient.Horizontal
                        GradientStop { position: 0; color: "black" }
                        GradientStop {
                            position: 1
                            color: { const c = root.hsvToRgb(root.hue, root.sat, 1); return Qt.rgba(c[0], c[1], c[2], 1) }
                        }
                    }
                }
                Rectangle {
                    width: 10; height: 10; radius: 5
                    anchors.verticalCenter: parent.verticalCenter
                    x: root.val * (valueTrack.width - width)
                    color: root.theme.background
                    border.color: root.theme.ink
                }
                MouseArea {
                    anchors.fill: parent
                    preventStealing: true
                    function pick(m) { root.val = Math.max(0, Math.min(1, m.x / width)); root.emitHsv() }
                    onPressed: m => { root.dragging = true; root.interactionChanged(true); pick(m) }
                    onPositionChanged: m => { if (pressed) pick(m) }
                    onReleased: { root.dragging = false; root.interactionChanged(false) }
                }
            }
            RowLayout {
                Layout.fillWidth: true
                spacing: 8
                Rectangle {
                    implicitWidth: 34; implicitHeight: 22; radius: 4
                    color: root.qcolor
                    border.color: root.theme.line
                }
                TextField {
                    id: hexField
                    Layout.fillWidth: true
                    implicitHeight: 24
                    text: root.hex
                    font: root.theme.textFont
                    color: acceptableInput ? root.theme.ink : root.theme.muted
                    validator: RegularExpressionValidator { regularExpression: /#?[0-9a-fA-F]{6}/ }
                    selectByMouse: true
                    onAccepted: {
                        const h = text.replace("#", "")
                        root.colorEdited([0, 2, 4].map(i => parseInt(h.substr(i, 2), 16) / 255))
                    }
                    background: Rectangle {
                        radius: 4
                        color: root.theme.well
                        border.color: hexField.activeFocus ? root.theme.accent : root.theme.line
                    }
                    Accessible.name: (root.label || "colour") + " hex value"
                }
            }
        }
    }
}
