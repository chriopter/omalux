import QtQuick

// The Lab a/b panels darktable draws in two modules' sidebars, with their mouse handling:
// - mode "monochrome" (monochrome.c _monochrome_draw 381, motion/press 462-538, scroll 540): an
//   8 × 8 grid of a, b over ±128 at L 53.39, darkened by the colour filter around (a, b); the
//   circle shows the filter size. Click or drag sets a, b; the wheel changes size (0.1 per step,
//   0.5…3); double-click resets a, b and size.
// - mode "correction" (colorcorrection.c draw 274, motion 349, press 393, scroll 431, keys 451):
//   the 8 × 8 grid of a, b over ±40 scaled by saturation, the shadows (dark) and highlights
//   (light) points joined by a line. A point within 5 px of the pointer is selected; dragging moves
//   it; arrows move it by 0.5 (Shift ×10, Ctrl ×0.1); double-click resets it, or all with none
//   selected; the wheel changes saturation (−0.1 per step, −3…3).
// Values are params; edits leave through edited({name: value}) and resetRequested(names).
Item {
    id: root
    required property var theme
    property string mode: "monochrome"
    property var values: ({})          // monochrome: a, b, size; correction: loa, lob, hia, hib, saturation
    property bool editable: true
    property alias navTarget: nav
    signal edited(var changes)
    signal resetRequested(var names)
    signal interactionChanged(bool active)
    property int selected: 0           // correction: 1 shadows, 2 highlights
    implicitHeight: width
    readonly property int inset: 5
    readonly property real span: mode === "monochrome" ? 256 : 80   // PANEL_WIDTH, 2 × DT_COLORCORRECTION_MAX
    function v(name, fallback) { const x = Number(root.values[name]); return isFinite(x) ? x : fallback }
    // panel position (0…1, y up) → a/b value and back
    function toValue(t) { return root.span * (t - .5) }
    function toPos(x) { return x / root.span + .5 }
    function labToRgb(L, a, b) {
        const fy = (L + 16) / 116, fx = fy + a / 500, fz = fy - b / 200
        const inv = t => t > 6 / 29 ? t * t * t : 3 * (6 / 29) * (6 / 29) * (t - 4 / 29)
        const X = 0.9642 * inv(fx), Y = inv(fy), Z = 0.8249 * inv(fz)
        const lin = [3.1338561 * X - 1.6168667 * Y - 0.4906146 * Z, -0.9787684 * X + 1.9161415 * Y + 0.0334540 * Z,
                     0.0719453 * X - 0.2289914 * Y + 1.4052427 * Z]
        return lin.map(c => { c = Math.max(0, Math.min(1, c)); return c <= 0.0031308 ? 12.92 * c : 1.055 * Math.pow(c, 1 / 2.4) - 0.055 })
    }
    function pointAt(mx, my) {
        const w = width - 2 * inset, h = height - 2 * inset
        return [Math.max(0, Math.min(w, mx - inset)) / w, Math.max(0, Math.min(h, h - 1 - my + inset)) / h]
    }
    function pick(mx, my) {
        if (root.mode !== "correction") return
        const p = pointAt(mx, my), ma = toValue(p[0]), mb = toValue(p[1])
        const thrs = 5   // darktable compares the a/b distance with DT_PIXEL_APPLY_DPI(5)
        const dl = (v("loa", 0) - ma) ** 2 + (v("lob", 0) - mb) ** 2, dh = (v("hia", 0) - ma) ** 2 + (v("hib", 0) - mb) ** 2
        root.selected = dl < thrs * thrs && dl < dh ? 1 : dh < thrs * thrs && dh <= dl ? 2 : 0
    }
    function setAt(mx, my) {
        const p = pointAt(mx, my), a = toValue(p[0]), b = toValue(p[1])
        if (root.mode === "monochrome") root.edited({ a: a, b: b })
        else if (root.selected === 1) root.edited({ loa: a, lob: b })
        else if (root.selected === 2) root.edited({ hia: a, hib: b })
    }
    function step(dx, dy) {
        if (root.mode === "monochrome") {
            root.edited({ a: Math.max(-128, Math.min(128, v("a", 0) + dx)), b: Math.max(-128, Math.min(128, v("b", 0) + dy)) })
            return
        }
        const k = root.selected === 1 ? ["loa", "lob"] : root.selected === 2 ? ["hia", "hib"] : null
        if (!k) return
        const out = {}
        out[k[0]] = Math.max(-40, Math.min(40, v(k[0], 0) + dx)); out[k[1]] = Math.max(-40, Math.min(40, v(k[1], 0) + dy))
        root.edited(out)
    }
    NavTarget {
        id: nav
        navId: "colorgrid-" + root.mode
        label: root.mode === "monochrome" ? "filter color" : "color correction grid"
        kind: "graph"
        focusItem: root
        enabled: root.editable
        activateLabel: "EDIT"
        onReset: root.resetRequested(root.mode === "monochrome" ? ["a", "b", "size"] : ["loa", "lob", "hia", "hib", "saturation"])
    }
    Keys.onPressed: event => {
        const f = (event.modifiers & Qt.ShiftModifier) ? 10 : (event.modifiers & Qt.ControlModifier) ? .1 : 1
        const s = (root.mode === "monochrome" ? 1 : .5) * f
        if (event.key === Qt.Key_Tab && root.mode === "correction") { root.selected = root.selected === 1 ? 2 : 1; event.accepted = true }
        else if (event.key === Qt.Key_Left) { root.step(-s, 0); event.accepted = true }
        else if (event.key === Qt.Key_Right) { root.step(s, 0); event.accepted = true }
        else if (event.key === Qt.Key_Up) { root.step(0, s); event.accepted = true }
        else if (event.key === Qt.Key_Down) { root.step(0, -s); event.accepted = true }
    }
    Canvas {
        id: canvas
        anchors.fill: parent
        onWidthChanged: requestPaint()
        Connections { target: root; function onValuesChanged() { canvas.requestPaint() } function onSelectedChanged() { canvas.requestPaint() } }
        onPaint: {
            const c = getContext("2d")
            c.fillStyle = Qt.rgba(.2, .2, .2, 1)
            c.fillRect(0, 0, width, height)
            const w = width - 2 * root.inset, h = height - 2 * root.inset, cells = 8
            for (let j = 0; j < cells; ++j)
                for (let i = 0; i < cells; ++i) {
                    let L = 53.390011, a, b
                    if (root.mode === "monochrome") {
                        a = 256 * (i / (cells - 1) - .5); b = 256 * (j / (cells - 1) - .5)
                        const d = 40 * 40 * root.v("size", 2) * root.v("size", 2)
                        const f = Math.exp(-Math.max(0, Math.min(1, ((a - root.v("a", 0)) ** 2 + (b - root.v("b", 0)) ** 2) / d)))
                        L *= f * f
                    } else {
                        const s = root.v("saturation", 1)
                        a = s * (L * .05 * 40 * (i / (cells - 1) - .5)); b = s * (L * .05 * 40 * (j / (cells - 1) - .5))
                    }
                    const rgb = root.labToRgb(L, a, b)
                    c.fillStyle = Qt.rgba(rgb[0], rgb[1], rgb[2], 1)
                    // y up: row j from the bottom
                    c.fillRect(root.inset + w * i / cells, root.inset + h - h * (j + 1) / cells + 1, w / cells - 1, h / cells - 1)
                }
            const px = x => root.inset + root.toPos(x) * w, py = y => root.inset + h - root.toPos(y) * h
            if (root.mode === "monochrome") {
                c.strokeStyle = Qt.rgba(.7, .7, .7, 1); c.lineWidth = 2
                c.beginPath(); c.arc(px(root.v("a", 0)), py(root.v("b", 0)), w * .22 * root.v("size", 2), 0, 2 * Math.PI); c.stroke()
            } else {
                const lo = [px(root.v("loa", 0)), py(root.v("lob", 0))], hi = [px(root.v("hia", 0)), py(root.v("hib", 0))]
                c.strokeStyle = Qt.rgba(.6, .6, .6, 1); c.lineWidth = 2
                c.beginPath(); c.moveTo(lo[0], lo[1]); c.lineTo(hi[0], hi[1]); c.stroke()
                c.fillStyle = Qt.rgba(.1, .1, .1, 1)
                c.beginPath(); c.arc(lo[0], lo[1], root.selected === 1 ? 5 : 3, 0, 2 * Math.PI); c.fill()
                c.fillStyle = Qt.rgba(.9, .9, .9, 1)
                c.beginPath(); c.arc(hi[0], hi[1], root.selected === 2 ? 5 : 3, 0, 2 * Math.PI); c.fill()
            }
            if (nav.current || root.activeFocus) { c.strokeStyle = root.theme.accent; c.lineWidth = 1; c.strokeRect(.5, .5, width - 1, height - 1) }
        }
    }
    MouseArea {
        anchors.fill: parent
        enabled: root.editable
        hoverEnabled: true
        preventStealing: true
        onPositionChanged: mouse => { if (pressed) root.setAt(mouse.x, mouse.y); else root.pick(mouse.x, mouse.y) }
        onPressed: mouse => {
            nav.claim()
            root.pick(mouse.x, mouse.y)
            root.interactionChanged(true)
            if (root.mode === "monochrome") root.setAt(mouse.x, mouse.y)
        }
        onReleased: root.interactionChanged(false)
        onCanceled: root.interactionChanged(false)
        onDoubleClicked: {
            if (root.mode === "monochrome") root.resetRequested(["a", "b", "size"])
            else root.resetRequested(root.selected === 1 ? ["loa", "lob"] : root.selected === 2 ? ["hia", "hib"]
                                                          : ["loa", "lob", "hia", "hib", "saturation"])
        }
        onWheel: wheel => {
            // GTK scroll delta: +1 down, -1 up
            const d = wheel.angleDelta.y > 0 ? -1 : wheel.angleDelta.y < 0 ? 1 : 0
            if (!d) return
            if (root.mode === "monochrome") root.edited({ size: Math.max(.5, Math.min(3, root.v("size", 2) + d * .1)) })
            else root.edited({ saturation: Math.max(-3, Math.min(3, root.v("saturation", 1) - .1 * d)) })
        }
    }
}
