import QtQuick
import "CanvasDraw.js" as Draw

// rotate and perspective on the photo (src/iop/ashift.c): a level line straightens the photo
// (right-drag, or "straighten"; _calculate_straightening, ashift.c:4070, needs at least
// 25 screen pixels), and structure is drawn as lines or as a rectangle along the building.
// The drawn structure is stored in the module's parameters like darktable stores it
// (last_drawn_lines, last_quad_lines) for the fit buttons; ends and corners stay draggable.
// Lines are green when closer to vertical, blue when closer to horizontal, as in darktable.
Item {
    id: root
    required property var theme
    property var overlay: ({})
    property bool editable: true
    signal edited(var gesture)
    signal interactionChanged(bool active)

    property string mode: ""        // "", "straighten", "lines", "rectangle"
    readonly property bool capturing: mode !== ""
    readonly property var tools: [
        { key: "straighten", label: "straighten", checked: mode === "straighten",
          tooltip: "drag along something level or plumb to straighten the photo (or right-drag)" },
        { key: "lines", label: "lines", checked: mode === "lines",
          tooltip: "drag along vertical and horizontal edges; right-click a line to remove it" },
        { key: "rectangle", label: "rectangle", checked: mode === "rectangle",
          tooltip: "drag a rectangle, then move its corners onto the building" },
        { key: "clear", label: "clear", enabled: lines.length > 0 || !!quad, tooltip: "remove the drawn structure" }]
    function toolClicked(key, modifiers) {
        if (key === "clear") { edited({ action: "clear-structure" }); localLines = null; localQuad = null; mode = ""; return }
        mode = mode === key ? "" : key
    }

    // The engine's structure, or what is being edited until it reports back.
    property var localLines: null
    property var localQuad: null
    readonly property var lines: localLines || (overlay && overlay.lines ? overlay.lines : [])
    readonly property var quad: localQuad || (overlay && overlay.quad ? overlay.quad : null)
    onOverlayChanged: if (!area.pressed) { localLines = null; localQuad = null }

    property var drag: null         // { kind, index, end, from, to }
    readonly property real near: 9

    function px(p) { return [p[0] * width, p[1] * height] }
    function frac(x, y) { return [x / width, y / height] }
    function hitEnd(x, y) {
        for (let i = 0; i < lines.length; ++i) {
            const l = lines[i]
            if (Math.hypot(l[0] * width - x, l[1] * height - y) < near) return { kind: "end", index: i, end: 0 }
            if (Math.hypot(l[2] * width - x, l[3] * height - y) < near) return { kind: "end", index: i, end: 1 }
        }
        if (quad)
            for (let i = 0; i < 4; ++i)
                if (Math.hypot(quad[i][0] * width - x, quad[i][1] * height - y) < near) return { kind: "corner", index: i }
        return null
    }
    function hitLine(x, y) {
        for (let i = 0; i < lines.length; ++i) {
            const l = lines[i]
            if (Draw.segmentDistance([l[0] * width, l[1] * height], [l[2] * width, l[3] * height], x, y) < near * 0.7) return i
        }
        return -1
    }
    onLinesChanged: paint.requestPaint()
    onQuadChanged: paint.requestPaint()
    onDragChanged: paint.requestPaint()
    onWidthChanged: paint.requestPaint()

    Canvas {
        id: paint
        anchors.fill: parent
        onPaint: {
            const ctx = getContext("2d")
            ctx.clearRect(0, 0, width, height)
            const colour = (a, b) => Math.abs(b[0] - a[0]) > Math.abs(b[1] - a[1]) ? "rgba(0,0,255,0.8)" : "rgba(0,255,0,0.8)"
            for (const l of root.lines) {
                const a = [l[0] * width, l[1] * height], b = [l[2] * width, l[3] * height]
                Draw.stroke(ctx, [a, b], false, false, colour(a, b))
                Draw.handle(ctx, a[0], a[1], 3, false, true)
                Draw.handle(ctx, b[0], b[1], 3, false, true)
            }
            if (root.quad) {
                const q = root.quad.map(p => root.px(p))
                for (let i = 0; i < 4; ++i)
                    Draw.stroke(ctx, [q[i], q[(i + 1) % 4]], false, false, colour(q[i], q[(i + 1) % 4]))
                for (const c of q) Draw.handle(ctx, c[0], c[1], 4, false, true)
            }
            const d = root.drag
            if (d && (d.kind === "straighten" || d.kind === "line")) {
                const a = root.px(d.from), b = root.px(d.to)
                Draw.stroke(ctx, [a, b], false, true, d.kind === "line" ? colour(a, b) : null)
            } else if (d && d.kind === "rectangle") {
                const a = root.px(d.from), b = root.px(d.to)
                Draw.stroke(ctx, [a, [b[0], a[1]], b, [a[0], b[1]]], true, true)
            }
        }
    }
    MouseArea {
        id: area
        anchors.fill: parent
        enabled: root.editable
        hoverEnabled: true
        preventStealing: true
        acceptedButtons: Qt.LeftButton | Qt.RightButton
        cursorShape: root.mode ? Qt.CrossCursor : Qt.ArrowCursor
        onPressed: mouse => {
            const p = root.frac(mouse.x, mouse.y)
            if (mouse.button === Qt.RightButton) {
                const line = root.mode === "lines" ? root.hitLine(mouse.x, mouse.y) : -1
                if (line >= 0) {
                    const kept = root.lines.filter((l, i) => i !== line)
                    root.localLines = kept
                    root.edited({ action: "lines", lines: kept })
                    return
                }
                root.drag = { kind: "straighten", from: p, to: p }
            } else if (root.mode === "straighten" || root.mode === "lines" || root.mode === "rectangle") {
                const end = root.mode === "lines" ? root.hitEnd(mouse.x, mouse.y) : null
                root.drag = end ? Object.assign(end, { from: p, to: p })
                                : { kind: root.mode === "lines" ? "line" : root.mode, from: p, to: p }
            } else {
                const end = root.hitEnd(mouse.x, mouse.y)
                if (!end) { mouse.accepted = false; return }
                root.drag = Object.assign(end, { from: p, to: p })
            }
            root.interactionChanged(true)
        }
        onPositionChanged: mouse => {
            if (!pressed || !root.drag) return
            const d = Object.assign({}, root.drag, { to: root.frac(mouse.x, mouse.y) })
            if (d.kind === "end") {
                const lines = root.lines.map(l => l.slice())
                lines[d.index][2 * d.end] = d.to[0]; lines[d.index][2 * d.end + 1] = d.to[1]
                root.localLines = lines
            } else if (d.kind === "corner") {
                const q = root.quad.map(c => c.slice())
                q[d.index] = d.to
                root.localQuad = q
            }
            root.drag = d
        }
        onReleased: {
            const d = root.drag
            root.drag = null
            root.interactionChanged(false)
            if (!d) return
            const length = Math.hypot((d.to[0] - d.from[0]) * width, (d.to[1] - d.from[1]) * height)
            if (d.kind === "straighten") {
                if (length >= 25) root.edited({ action: "straighten", a: d.from, b: d.to })
                if (root.mode === "straighten") root.mode = ""
            } else if (d.kind === "line") {
                if (length >= 8) {
                    const lines = root.lines.concat([[d.from[0], d.from[1], d.to[0], d.to[1]]])
                    root.localLines = lines
                    root.edited({ action: "lines", lines: lines })
                }
            } else if (d.kind === "end") {
                root.edited({ action: "lines", lines: root.lines })
            } else if (d.kind === "rectangle") {
                if (length >= 8) {
                    const l = Math.min(d.from[0], d.to[0]), r = Math.max(d.from[0], d.to[0])
                    const t = Math.min(d.from[1], d.to[1]), b = Math.max(d.from[1], d.to[1])
                    root.localQuad = [[l, t], [r, t], [r, b], [l, b]]
                    root.edited({ action: "quad", topLeft: [l, t], topRight: [r, t], bottomRight: [r, b], bottomLeft: [l, b] })
                }
                root.mode = ""
            } else if (d.kind === "corner") {
                const q = root.quad
                root.edited({ action: "quad", topLeft: q[0], topRight: q[1], bottomRight: q[2], bottomLeft: q[3] })
            }
        }
        onCanceled: { root.drag = null; root.interactionChanged(false) }
    }
}
