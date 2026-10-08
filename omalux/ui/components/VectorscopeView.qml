import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

// darktable's RYB vectorscope with its colour harmony guide (src/libs/scopes/vectorscope.c),
// small, for color harmonizer. The plot comes from the engine as a PNG already oriented and
// coloured like _vec_draw (tools_vectorscope.c: 270°, v up, logarithmic scale); this component
// outlines the guide's sectors and dims the plot outside them (harmony/dim 0.7). Harmony chips
// choose the guide (a second click removes it), the wheel rotates it by 15° (Ctrl 1°),
// Shift+wheel changes the width and Alt+wheel cycles the harmony, as _vec_eventbox_scroll does.
// `customAngles` (turns) draws a custom harmony instead; the wheel then turns its nodes
// (customRotated). The component edits nothing: guideEdited reports the new guide.
Column {
    id: root
    required property var theme
    property string png: ""             // base64 PNG from the engine
    property var guide: ({ type: 0, rotation: 0, width: 0 })
    property var customAngles: []
    property bool editable: true
    signal guideEdited(int type, int rotation, int width)
    signal customRotated(real turns)
    spacing: 4

    // vectorscope.c:60-96
    readonly property var harmonies: [
        { name: "none", angle: [], length: [] },
        { name: "monochromatic", angle: [0], length: [.8] },
        { name: "analogous", angle: [-1 / 12, 0, 1 / 12], length: [.5, .8, .5] },
        { name: "analogous complementary", angle: [-1 / 12, 0, 1 / 12, 6 / 12], length: [.5, .8, .5, .5] },
        { name: "complementary", angle: [0, 6 / 12], length: [.8, .5] },
        { name: "split complementary", angle: [0, 5 / 12, 7 / 12], length: [.8, .5, .5] },
        { name: "dyad", angle: [-1 / 12, 1 / 12], length: [.8, .8] },
        { name: "triad", angle: [0, 4 / 12, 8 / 12], length: [.8, .5, .5] },
        { name: "tetrad", angle: [-1 / 12, 1 / 12, 5 / 12, 7 / 12], length: [.8, .8, .5, .5] },
        { name: "square", angle: [0, 3 / 12, 6 / 12, 9 / 12], length: [.8, .5, .5, .5] }]
    readonly property var widths: [0.5 / 12, 0.75 / 12, 0.25 / 12, 0]
    readonly property var widthNames: ["normal", "large", "narrow", "line"]
    // darktable's sectors for the current guide: [{ a1, a2 (turns), len (0..1 of the radius) }]
    readonly property var sectors: {
        const g = root.guide || { type: 0 }, hw = widths[g.width || 0], out = []
        if ((root.customAngles || []).length) {
            for (const a of root.customAngles) out.push({ a1: a - hw, a2: a + hw, len: .8 })
        } else if (g.type > 0) {
            const hm = harmonies[g.type]
            for (let i = 0; i < hm.angle.length; ++i) {
                const s1 = i > 0 ? Math.min(hw, (hm.angle[i] - hm.angle[i - 1]) / 2) : hw
                const s2 = i < hm.angle.length - 1 ? Math.min(hw, (hm.angle[i + 1] - hm.angle[i]) / 2) : hw
                out.push({ a1: hm.angle[i] - s1 + g.rotation / 360, a2: hm.angle[i] + s2 + g.rotation / 360, len: hm.length[i] })
            }
        }
        return out
    }
    function baselog(x) { return Math.log1p(29 * x) / Math.log(30) }   // VECTORSCOPE_BASE_LOG 30
    function setType(t) {
        // _color_harmony_toggled: a click on the active harmony removes the guide.
        root.guideEdited(t === root.guide.type ? 0 : t, root.guide.rotation, root.guide.width)
    }
    function scroll(delta, modifiers) {
        const g = root.guide
        if ((root.customAngles || []).length && !(modifiers & (Qt.ShiftModifier | Qt.AltModifier))) {
            root.customRotated(delta * ((modifiers & Qt.ControlModifier) ? 1 : 15) / 360)
            return
        }
        if (modifiers & Qt.ShiftModifier) root.guideEdited(g.type, g.rotation, (g.width + delta + 4) % 4)
        else if (modifiers & Qt.AltModifier) root.guideEdited((g.type + delta + 10) % 10, g.rotation, g.width)
        else {
            let r = g.rotation
            if (modifiers & Qt.ControlModifier) r += delta
            else r = Math.floor((r + 7) / 15) * 15 + 15 * delta
            root.guideEdited(g.type, ((r % 360) + 360) % 360, g.width)
        }
    }

    Rectangle {
        id: plotBox
        objectName: "vectorscope-plot"
        width: Math.min(root.width, 220); height: width
        anchors.horizontalCenter: parent.horizontalCenter
        color: Qt.rgba(.12, .12, .12, 1)
        Image {
            id: plotImage
            anchors.fill: parent
            anchors.margins: 2
            smooth: true
            source: root.png ? "data:image/png;base64," + root.png : ""
        }
        Canvas {
            id: overlay
            anchors.fill: parent
            onWidthChanged: requestPaint()
            Connections { target: root; function onSectorsChanged() { overlay.requestPaint() } }
            // a chromaticity angle (turns) at radius len → screen: rotate 270°, v up (_vec_draw)
            function point(turn, len) {
                const a = 2 * Math.PI * turn, r = width / 2 - 2
                return [width / 2 - Math.sin(a) * len * r, height / 2 - Math.cos(a) * len * r]
            }
            function sectorPath(c, s) {
                const len = root.baselog(s.len)
                c.moveTo(width / 2, height / 2)
                for (let i = 0; i <= 16; ++i) { const p = point(s.a1 + (s.a2 - s.a1) * i / 16, len); c.lineTo(p[0], p[1]) }
                c.closePath()
            }
            onPaint: {
                const c = getContext("2d")
                c.reset()
                c.strokeStyle = Qt.rgba(1, 1, 1, .18); c.lineWidth = 1
                c.beginPath(); c.arc(width / 2, height / 2, width / 2 - 2, 0, 2 * Math.PI); c.stroke()
                const sectors = root.sectors
                if (!sectors.length) return
                // dim outside the sectors (unless "line"), then outline them
                if ((root.guide.width || 0) !== 3 || (root.customAngles || []).length) {
                    c.fillStyle = Qt.rgba(.12, .12, .12, .7)
                    c.fillRule = Qt.OddEvenFill
                    c.beginPath()
                    c.rect(0, 0, width, height)
                    for (const s of sectors) sectorPath(c, s)
                    c.fill()
                }
                c.strokeStyle = root.theme.ink
                c.beginPath()
                for (const s of sectors) sectorPath(c, s)
                c.stroke()
            }
        }
        WheelHandler {
            enabled: root.editable && (root.guide.type > 0 || (root.customAngles || []).length > 0)
            acceptedDevices: PointerDevice.Mouse | PointerDevice.TouchPad
            onWheel: event => root.scroll(event.angleDelta.y > 0 ? 1 : event.angleDelta.y < 0 ? -1 : 0, event.modifiers)
        }
        Text {
            visible: root.guide.type > 0 && !(root.customAngles || []).length
            anchors.right: parent.right; anchors.bottom: parent.bottom; anchors.margins: 4
            horizontalAlignment: Text.AlignRight
            text: root.guide.rotation + "°\n" + root.harmonies[root.guide.type].name
            color: root.theme.muted; font: root.theme.textFont
        }
    }
    // darktable's harmony buttons beside the scope
    Flow {
        width: root.width
        spacing: 4
        Repeater {
            model: root.harmonies.length - 1
            AbstractButton {
                id: chip
                required property int index
                readonly property int type: index + 1
                objectName: "vectorscope-harmony-" + type
                enabled: root.editable
                hoverEnabled: true
                implicitWidth: label.implicitWidth + 12; implicitHeight: 22
                onClicked: { nav.claim(); root.setType(type) }
                ToolTip.visible: hovered; ToolTip.delay: 600
                ToolTip.text: "scroll to coarse-rotate\nctrl+scroll to fine rotate\nshift+scroll to change width\nalt+scroll to cycle"
                NavTarget { id: nav; navId: "vectorscope/" + chip.type; label: root.harmonies[chip.type].name; kind: "button"
                            activateLabel: "GUIDE"; onActivate: root.setType(chip.type) }
                contentItem: Text { id: label; text: root.harmonies[chip.type].name; font: root.theme.textFont
                                    color: root.guide.type === chip.type || nav.current ? root.theme.accent : root.theme.ink
                                    horizontalAlignment: Text.AlignHCenter; verticalAlignment: Text.AlignVCenter }
                background: Rectangle { radius: 4; color: chip.hovered ? root.theme.hover : "transparent"
                                        border.color: root.guide.type === chip.type ? root.theme.accent : root.theme.line }
            }
        }
    }
    Text {
        visible: root.guide.type > 0
        text: "width: " + root.widthNames[root.guide.width || 0]
        color: root.theme.muted; font: root.theme.textFont
    }
}
