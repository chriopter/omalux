import QtQuick
import "CanvasDraw.js" as Draw

// liquify on the photo (src/iop/liquify.c): each warp is a centre, a radius (the circle) and
// a strength arrow. Add a point, line or curve from the toolbar, then click or drag on the
// photo; drag a centre, the radius handle, the arrow's head or a curve's control points;
// Ctrl+click an arrow head to cycle linear, grow and shrink; right-click a node to delete
// it, Ctrl+right-click to delete its whole path. The dragged handle follows the pointer
// locally until the engine reports the warp.
Item {
    id: root
    required property var theme
    property var overlay: ({})
    property bool editable: true
    property real viewScale: 1
    signal edited(var gesture)
    signal interactionChanged(bool active)

    property string mode: ""                // "point", "line", "curve" while adding
    readonly property bool capturing: mode !== ""
    readonly property var tools: ["point", "line", "curve"].map(t => ({ key: t, label: t, checked: mode === t,
                                                                         tooltip: "add a " + t + " warp" }))
    function toolClicked(key, modifiers) { mode = mode === key ? "" : key }

    readonly property var nodes: overlay && overlay.nodes ? overlay.nodes : []
    property var drag: null                 // { index, part, to } or { kind: "new", from, to }
    property var hover: null
    readonly property real near: 8
    onNodesChanged: if (!area.pressed) { drag = null; paint.requestPaint() }
    onDragChanged: paint.requestPaint()
    onHoverChanged: paint.requestPaint()
    onWidthChanged: paint.requestPaint()

    function px(p) { return p ? [p[0] * width, p[1] * height] : null }
    function shown(n) {
        const d = drag
        if (!d || d.index !== n.index) return n
        const out = Object.assign({}, n)
        if (d.part === "center") {
            const dx = d.to[0] - n.center[0], dy = d.to[1] - n.center[1]
            out.center = d.to
            out.radius = [n.radius[0] + dx, n.radius[1] + dy]
            out.strength = [n.strength[0] + dx, n.strength[1] + dy]
        } else out[d.part] = d.to
        return out
    }
    // The nearest handle within reach; the centre wins a tie.
    function hitAt(x, y) {
        let best = null, distance = near
        for (let i = nodes.length - 1; i >= 0; --i) {
            const n = shown(nodes[i])
            for (const part of ["center", "strength", "radius", "ctrl1", "ctrl2"]) {
                const p = px(n[part])
                const d = p ? Math.hypot(p[0] - x, p[1] - y) : Infinity
                if (d < distance) { best = { index: n.index, part: part }; distance = d }
            }
        }
        return best
    }

    Canvas {
        id: paint
        anchors.fill: parent
        onPaint: {
            const ctx = getContext("2d")
            ctx.clearRect(0, 0, width, height)
            const byIndex = {}
            for (const raw of root.nodes) byIndex[raw.index] = root.shown(raw)
            for (const raw of root.nodes) {
                const n = byIndex[raw.index]
                const c = root.px(n.center), r = root.px(n.radius), s = root.px(n.strength)
                const on = root.hover && root.hover.index === n.index
                // The path from the previous node: a line or darktable's Bézier segment.
                const prev = n.prev >= 0 ? byIndex[n.prev] : null
                if (prev) {
                    const a = root.px(prev.center)
                    if (n.type === "curve" && n.ctrl1 && n.ctrl2) {
                        const c1 = root.px(n.ctrl1), c2 = root.px(n.ctrl2), pts = []
                        for (let k = 0; k <= 24; ++k) {
                            const t = k / 24, u = 1 - t
                            pts.push([u * u * u * a[0] + 3 * u * u * t * c1[0] + 3 * u * t * t * c2[0] + t * t * t * c[0],
                                      u * u * u * a[1] + 3 * u * u * t * c1[1] + 3 * u * t * t * c2[1] + t * t * t * c[1]])
                        }
                        Draw.stroke(ctx, pts, false, on)
                        Draw.stroke(ctx, [a, c1, null, c2, c], false, false, "rgba(255,255,255,0.5)", true)
                        Draw.handle(ctx, c1[0], c1[1], 3, false, false)
                        Draw.handle(ctx, c2[0], c2[1], 3, false, false)
                    } else Draw.stroke(ctx, [a, c], false, on)
                }
                const radius = Math.hypot(r[0] - c[0], r[1] - c[1])
                ctx.beginPath(); ctx.arc(c[0], c[1], radius, 0, Math.PI * 2)
                ctx.fillStyle = "rgba(255,255,255,0.08)"; ctx.fill()
                const circle = []
                for (let k = 0; k <= 64; ++k) circle.push([c[0] + radius * Math.cos(k * Math.PI / 32), c[1] + radius * Math.sin(k * Math.PI / 32)])
                Draw.stroke(ctx, circle, true, false, null, true)
                Draw.arrow(ctx, c, s, 8, on)
                if (n.warp !== "linear") {
                    ctx.fillStyle = "white"; ctx.font = "11px sans-serif"
                    ctx.fillText(n.warp === "grow" ? "+" : "−", s[0] + 5, s[1] - 5)
                }
                Draw.handle(ctx, c[0], c[1], 4, on, true)
                Draw.handle(ctx, r[0], r[1], 3, on, false)
            }
            const d = root.drag
            if (d && d.kind === "new") Draw.stroke(ctx, [root.px(d.from), root.px(d.to)], false, true)
        }
    }
    MouseArea {
        id: area
        anchors.fill: parent
        enabled: root.editable
        hoverEnabled: true
        preventStealing: true
        acceptedButtons: Qt.LeftButton | Qt.RightButton
        cursorShape: root.mode ? Qt.CrossCursor : root.hover ? Qt.SizeAllCursor : Qt.ArrowCursor
        function frac(mouse) { return [mouse.x / width, mouse.y / height] }
        onPressed: mouse => {
            const p = frac(mouse)
            if (root.mode && mouse.button === Qt.LeftButton) {
                if (root.mode === "point") {
                    root.edited({ action: "add-point", at: p, scale: root.viewScale })
                    root.mode = ""
                } else root.drag = { kind: "new", from: p, to: p }
                return
            }
            const hit = root.hitAt(mouse.x, mouse.y)
            if (!hit) { mouse.accepted = false; return }
            if (mouse.button === Qt.RightButton) {
                root.edited({ action: mouse.modifiers & Qt.ControlModifier ? "remove-path" : "remove", index: hit.index })
                return
            }
            if (hit.part === "strength" && (mouse.modifiers & Qt.ControlModifier)) {
                const n = root.nodes.find(x => x.index === hit.index)
                const next = { linear: "grow", grow: "shrink", shrink: "linear" }[n.warp] || "linear"
                root.edited({ action: "warp", index: hit.index, value: next })
                return
            }
            root.drag = { index: hit.index, part: hit.part, to: p }
            root.interactionChanged(true)
        }
        onPositionChanged: mouse => {
            if (!pressed || !root.drag) { if (!root.mode) root.hover = root.hitAt(mouse.x, mouse.y); return }
            const p = frac(mouse)
            const d = Object.assign({}, root.drag, { to: p })
            root.drag = d
            if (d.kind !== "new") root.edited({ action: "move", index: d.index, part: d.part, to: p })
        }
        onReleased: {
            const d = root.drag
            if (d && d.kind === "new") {
                root.drag = null
                if (Math.hypot((d.to[0] - d.from[0]) * width, (d.to[1] - d.from[1]) * height) > 4)
                    root.edited({ action: root.mode === "curve" ? "add-curve" : "add-line", from: d.from, to: d.to,
                                  scale: root.viewScale })
                root.mode = ""
            } else if (d) root.interactionChanged(false)
        }
        onCanceled: { root.drag = null; root.interactionChanged(false) }
        onExited: root.hover = null
    }
}
