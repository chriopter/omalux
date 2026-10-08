import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

// One range of a parametric mask: darktable's gradient slider with four markers
// (dtgtk/gradientslider.c as blend_gui.c sets it up). Between the two filled markers the
// module blends fully, outside the two open ones not at all, in between gradually; the
// polarity button (±) inverts that. Values are darktable's stored 0…1 positions; the labels
// above print them as darktable does (_blendif_scale_print_default/_ab/_hue, with the channel
// boost factor). Drag a marker (it never passes its neighbours), double-click resets the
// range. Keyboard: Enter lends the keys, ←/→ move the active marker (Shift ×10, Ctrl ×0.1),
// ↑/↓ pick the marker, Escape gives the keys back.
FocusScope {
    id: root
    required property var theme
    property string label: "input"
    property var values: [0, 0, 1, 1]
    property var stops: []              // [[position 0…1, "#rrggbb"], …]
    property bool negative: false       // polarity: darktable's blendif bit (channel + 16)
    property string scale: "default"    // default · ab · hue
    property real boost: 1              // exp2 of the channel's boost factor
    property real increment: 0.01
    property bool editable: true
    property string tooltip: ""
    signal valuesEdited(var values)
    signal polarityToggled(bool negative)
    signal resetRequested()
    signal interactionChanged(bool active)
    property alias navTarget: navTarget
    property alias polarityTarget: polarityNav
    property int activeMarker: -1

    implicitWidth: 260
    implicitHeight: header.height + 4 + 30

    NavTarget {
        id: navTarget
        navId: "blendif-" + root.label
        label: root.label
        kind: "graph"
        focusItem: root
        enabled: root.editable
        activateLabel: "EDIT MARKERS"
        onReset: root.resetRequested()
    }

    // _blendif_scale_print_*: what darktable prints for one marker.
    function format(v) {
        if (root.scale === "hue")
            return (v * 360).toFixed(0)
        if (root.scale === "ab") {
            const s = (v * 256 - 128) * root.boost
            return s.toFixed(Math.abs(s) < 10 ? 1 : 0)
        }
        const s = v * root.boost
        const digits = s < 0.0001 ? 0 : s < 0.01 ? 2 : s < 0.999 ? 1 : 0
        return (s * 100).toFixed(digits)
    }
    function setMarker(i, v) {
        if (!root.editable || i < 0 || i > 3) return
        const low = i > 0 ? root.values[i - 1] : 0
        const high = i < 3 ? root.values[i + 1] : 1
        const next = root.values.slice()
        next[i] = Math.max(low, Math.min(high, Math.max(0, Math.min(1, v))))
        if (next[i] !== root.values[i]) root.valuesEdited(next)
    }
    function nearest(x, y) {
        const w = bar.width - 2 * bar.inset
        const distances = root.values.map(v => Math.abs(bar.inset + v * w - x))
        const least = Math.min(...distances)
        const close = [0, 1, 2, 3].filter(i => distances[i] - least < 1)
        // Coinciding markers: the half with the filled triangles takes markers 1 and 2, the
        // other half 0 and 3; of the rest, the one on the side the pointer is.
        const upper = (y < bar.height / 2) !== root.negative
        const preferred = close.filter(i => (i === 1 || i === 2) === upper)
        const pool = preferred.length ? preferred : close
        return x >= bar.inset + root.values[pool[0]] * w ? pool[pool.length - 1] : pool[0]
    }

    Keys.onPressed: event => {
        const fine = event.modifiers & (Qt.ControlModifier | Qt.AltModifier)
        const factor = event.modifiers & Qt.ShiftModifier ? 10 : fine ? 0.1 : 1
        const marker = root.activeMarker < 0 ? 1 : root.activeMarker
        if (event.key === Qt.Key_Left || event.key === Qt.Key_Right) {
            root.activeMarker = marker
            root.setMarker(marker, root.values[marker] + (event.key === Qt.Key_Right ? 1 : -1) * root.increment * factor)
            event.accepted = true
        } else if (event.key === Qt.Key_Up || event.key === Qt.Key_Down) {
            root.activeMarker = (marker + (event.key === Qt.Key_Down ? 1 : 3)) % 4
            event.accepted = true
        }
    }

    Item {
        id: header
        width: parent.width
        height: 18
        RowLayout {
            anchors.fill: parent
            anchors.rightMargin: polarity.width + 6
            spacing: 0
            Text {
                Layout.preferredWidth: parent.width * .25
                text: root.label
                color: navTarget.current || root.activeFocus ? root.theme.accent : root.theme.ink
                font: root.theme.textFont
                elide: Text.ElideRight
                HoverHandler { id: labelHover }
                ToolTip.visible: labelHover.hovered && root.tooltip !== ""
                ToolTip.text: root.tooltip
                ToolTip.delay: 600
            }
            Repeater {
                model: 4
                Text {
                    required property int index
                    Layout.fillWidth: true
                    horizontalAlignment: Text.AlignRight
                    text: root.format(root.values[index])
                    color: root.activeMarker === index && root.activeFocus ? root.theme.accent : root.theme.muted
                    font: root.theme.textFont
                }
            }
        }
    }
    RowLayout {
        y: header.height + 4
        width: parent.width
        height: 30
        spacing: 6
        Canvas {
            id: bar
            objectName: "blendif-bar-" + root.label
            readonly property int inset: 6
            Layout.fillWidth: true
            Layout.fillHeight: true
            opacity: root.editable ? 1 : .5
            onPaint: {
                const c = getContext("2d")
                c.clearRect(0, 0, width, height)
                const w = width - 2 * inset, top = 7, bottom = height - 7
                // The channel's colour gradient, as darktable draws it behind the markers.
                const g = c.createLinearGradient(inset, 0, inset + w, 0)
                for (const s of root.stops) g.addColorStop(s[0], s[1])
                c.fillStyle = root.stops.length ? g : root.theme.line
                c.fillRect(inset, top, w, bottom - top)
                // Mask opacity over the channel: 1 between the filled markers.
                const v = root.values
                const hi = top + 2, lo = bottom - 2
                const on = root.negative ? lo : hi, off = root.negative ? hi : lo
                c.strokeStyle = root.theme.ink
                c.lineWidth = 1.5
                c.beginPath()
                c.moveTo(inset, off)
                c.lineTo(inset + v[0] * w, off)
                c.lineTo(inset + v[1] * w, on)
                c.lineTo(inset + v[2] * w, on)
                c.lineTo(inset + v[3] * w, off)
                c.lineTo(inset + w, off)
                c.stroke()
                // Markers: filled triangles for 1 and 2, open ones for 0 and 3; positive
                // polarity puts the filled ones on top, negative below.
                for (let i = 0; i < 4; ++i) {
                    const filled = i === 1 || i === 2
                    const above = filled !== root.negative
                    const x = inset + v[i] * w
                    c.beginPath()
                    if (above) { c.moveTo(x - 5, 0); c.lineTo(x + 5, 0); c.lineTo(x, top + 1) }
                    else { c.moveTo(x - 5, height); c.lineTo(x + 5, height); c.lineTo(x, bottom - 1) }
                    c.closePath()
                    const active = i === root.activeMarker && (root.activeFocus || drag.pressed)
                    c.strokeStyle = active ? root.theme.accent : root.theme.ink
                    c.fillStyle = active ? root.theme.accent : root.theme.ink
                    c.lineWidth = 1.2
                    if (filled) c.fill(); else c.stroke()
                }
                if (navTarget.current) {
                    c.strokeStyle = root.theme.accent
                    c.lineWidth = 1
                    c.strokeRect(0.5, 0.5, width - 1, height - 1)
                }
            }
            Connections {
                target: root
                function onValuesChanged() { bar.requestPaint() }
                function onNegativeChanged() { bar.requestPaint() }
                function onStopsChanged() { bar.requestPaint() }
                function onActiveMarkerChanged() { bar.requestPaint() }
                function onActiveFocusChanged() { bar.requestPaint() }
            }
            Connections {
                target: navTarget
                function onCurrentChanged() { bar.requestPaint() }
            }
            onWidthChanged: requestPaint()
            MouseArea {
                id: drag
                anchors.fill: parent
                enabled: root.editable
                preventStealing: true
                property int marker: -1
                function valueAt(x) { return (x - bar.inset) / Math.max(1, bar.width - 2 * bar.inset) }
                onPressed: mouse => {
                    navTarget.claim()
                    marker = root.nearest(mouse.x, mouse.y)
                    root.activeMarker = marker
                    root.interactionChanged(true)
                }
                onPositionChanged: mouse => { if (pressed) root.setMarker(marker, valueAt(mouse.x)) }
                onReleased: { root.interactionChanged(false); bar.requestPaint() }
                onCanceled: root.interactionChanged(false)
                onDoubleClicked: root.resetRequested()
            }
        }
        ToolButton {
            id: polarity
            objectName: "blendif-polarity-" + root.label
            Layout.preferredWidth: 22
            Layout.fillHeight: true
            padding: 0
            enabled: root.editable
            hoverEnabled: true
            onClicked: { polarityNav.claim(); root.polarityToggled(!root.negative) }
            NavTarget {
                id: polarityNav
                navId: "blendif-polarity-" + root.label
                label: "toggle polarity"
                kind: "switch"
                group: navTarget.group
                enabled: root.editable
                resettable: false
                onAdjust: root.polarityToggled(!root.negative)
                onActivate: root.polarityToggled(!root.negative)
            }
            ToolTip.visible: hovered
            ToolTip.text: "toggle polarity. best seen by enabling 'display mask'"
            ToolTip.delay: 600
            Accessible.name: "toggle polarity of " + root.label
            contentItem: Text {
                text: root.negative ? "−" : "+"
                horizontalAlignment: Text.AlignHCenter
                verticalAlignment: Text.AlignVCenter
                color: polarity.hovered || polarityNav.current ? root.theme.accent : root.theme.ink
                font: root.theme.settingsFont
            }
            background: Rectangle {
                radius: 3
                color: "transparent"
                border.color: polarityNav.current ? root.theme.accent : root.theme.line
            }
        }
    }
}
