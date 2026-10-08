import QtQuick
import "CanvasDraw.js" as Draw

// darktable's drawn shapes on the photo (develop/masks/*.c): retouch, spot removal and the
// drawn mask of a module's blend section. The engine sends every shape's outline, feather
// and clone source as the preview shows them; this item draws them and turns the pointer
// into gestures (engine/canvas.h):
//   pick a shape on the toolbar, then click (circle, ellipse), drag (gradient), click the
//   corners (path; finish with a right-click, a double-click or on the first corner) or
//   paint (brush); Ctrl when picking keeps adding; Shift+click first places a clone source
//   drag a shape to move it, its source to move that, its outline to resize it and its
//   feather line to soften it; the round handle turns an ellipse or gradient
//   the wheel over a shape: size, Shift feather, Ctrl opacity, Shift+Ctrl rotation (as in
//   darktable); right-click removes the shape
// While a drag is on, the shape follows the pointer locally; the engine's next overlay
// replaces it after release.
Item {
    id: root
    required property var theme
    property var overlay: ({})
    property bool editable: true
    signal edited(var gesture)
    signal interactionChanged(bool active)

    readonly property string tool: overlay && overlay.tool ? overlay.tool : "mask"
    readonly property var shapes: overlay && overlay.shapes ? overlay.shapes : []
    readonly property int selected: overlay && overlay.selected ? overlay.selected : 0
    readonly property string algorithm: overlay && overlay.algorithm ? overlay.algorithm : ""
    readonly property var selectedShape: shapes.find(s => s.id === selected) || null

    property string mode: ""            // a shape type while adding one
    property bool continuous: false
    property var pendingSource: null
    property var pathPoints: []
    property var brushPoints: []
    property var gradientFrom: null
    property var drag: null
    property var settled: null          // the finished drag, shown until the engine answers
    property var frozen: null           // shapes as they were when the drag began
    property var hover: null
    readonly property real near: 7
    readonly property bool capturing: mode !== ""

    readonly property var shapeTypes: tool === "retouch" ? ["circle", "ellipse", "path", "brush"]
                                    : tool === "spots" ? ["circle", "ellipse", "path"]
                                    : ["circle", "ellipse", "path", "brush", "gradient"]
    readonly property var tools: {
        const out = shapeTypes.map(t => ({ key: "shape:" + t, label: t, checked: mode === t,
                                           tooltip: "add a " + t + " (Ctrl: several)" }))
        if (tool === "retouch")
            for (const a of ["clone", "heal", "blur", "fill"])
                out.push({ key: "algo:" + a, label: a, checked: algorithm === a, separated: a === "clone",
                           tooltip: a + " for new shapes (Ctrl: the selected shape)" })
        if (mode === "path" && pathPoints.length)
            out.push({ key: "finish", label: "finish", separated: true, enabled: pathPoints.length >= 3, tooltip: "close the path" },
                     { key: "cancel", label: "cancel", tooltip: "discard the path" })
        if (selectedShape && !mode) {
            out.push({ key: "opacity-", label: "−", separated: true, tooltip: "less opacity" },
                     { key: "opacity", label: Math.round(selectedShape.opacity * 100) + "%", enabled: false, tooltip: "opacity of the selected shape" },
                     { key: "opacity+", label: "+", tooltip: "more opacity" },
                     { key: "remove", label: "remove", tooltip: "remove the selected shape" })
        }
        return out
    }
    function toolClicked(key, modifiers) {
        if (key.startsWith("shape:")) {
            const type = key.slice(6)
            mode = mode === type ? "" : type
            continuous = !!(modifiers & Qt.ControlModifier)
            pathPoints = []; brushPoints = []; gradientFrom = null
        } else if (key.startsWith("algo:")) {
            const gesture = { action: "algorithm", value: key.slice(5) }
            if ((modifiers & Qt.ControlModifier) && selected) gesture.id = selected
            edited(gesture)
        } else if (key === "finish") finishPath()
        else if (key === "cancel") { pathPoints = []; mode = "" }
        else if (key === "remove" && selected) edited({ action: "remove", id: selected })
        else if ((key === "opacity-" || key === "opacity+") && selectedShape)
            edited({ action: "opacity", id: selected,
                     value: Math.max(0.05, Math.min(1, selectedShape.opacity + (key === "opacity+" ? 0.05 : -0.05))) })
    }
    function cancel() { mode = ""; pathPoints = []; brushPoints = []; gradientFrom = null }

    function finishPath() {
        if (pathPoints.length >= 3) add({ action: "add", type: "path", points: pathPoints })
        pathPoints = []
        if (!continuous) mode = ""
    }
    function add(gesture) {
        if (pendingSource && clones()) gesture.source = pendingSource
        pendingSource = null
        edited(gesture)
    }
    function clones() { return tool === "spots" || (tool === "retouch" && (algorithm === "clone" || algorithm === "heal")) }

    // Pixel geometry of the shapes, from the engine's fractions.
    readonly property var drawn: (frozen || shapes).map(s => shapePixels(s))
    function shapePixels(s) {
        const w = width, h = height
        const out = Object.assign({}, s)
        out.outlinePx = Draw.scale(s.outline, w, h)
        out.borderPx = Draw.scale(s.border, w, h)
        out.sourcePx = Draw.scale(s.source, w, h)
        out.centerPx = s.center ? [s.center[0] * w, s.center[1] * h] : null
        out.pivotPx = s.pivot ? [s.pivot[0] * w, s.pivot[1] * h] : null
        out.pivot2Px = s.pivot2 ? [s.pivot2[0] * w, s.pivot2[1] * h] : null
        out.sourceCenterPx = s.sourceCenter ? [s.sourceCenter[0] * w, s.sourceCenter[1] * h] : null
        return out
    }
    // The dragged shape as it will be: moved, scaled or turned around its centre.
    function dragged(s) {
        const d = drag || settled
        if (!d || s.id !== d.id) return s
        const out = Object.assign({}, s)
        const c = s.centerPx || [0, 0]
        if (d.kind === "move") {
            const dx = d.total[0], dy = d.total[1]
            out.outlinePx = Draw.translate(s.outlinePx, dx, dy); out.borderPx = Draw.translate(s.borderPx, dx, dy)
            out.centerPx = [c[0] + dx, c[1] + dy]
            if (s.pivotPx) out.pivotPx = [s.pivotPx[0] + dx, s.pivotPx[1] + dy]
            if (s.pivot2Px) out.pivot2Px = [s.pivot2Px[0] + dx, s.pivot2Px[1] + dy]
        } else if (d.kind === "source") {
            out.sourcePx = Draw.translate(s.sourcePx, d.total[0], d.total[1])
        } else if (d.kind === "size") {
            out.outlinePx = Draw.scaleAround(s.outlinePx, c, d.factor); out.borderPx = Draw.scaleAround(s.borderPx, c, d.factor)
        } else if (d.kind === "feather") {
            out.borderPx = Draw.scaleAround(s.borderPx, c, d.factor)
        } else if (d.kind === "rotate") {
            out.outlinePx = Draw.rotateAround(s.outlinePx, c, d.angle); out.borderPx = Draw.rotateAround(s.borderPx, c, d.angle)
            out.pivotPx = Draw.rotateAround([s.pivotPx], c, d.angle)[0]
            if (s.pivot2Px) out.pivot2Px = Draw.rotateAround([s.pivot2Px], c, d.angle)[0]
        }
        return out
    }

    function hitShape(s, x, y) {
        const closed = s.closed
        if (s.pivotPx && Math.hypot(s.pivotPx[0] - x, s.pivotPx[1] - y) < near) return "rotate"
        if (s.pivot2Px && Math.hypot(s.pivot2Px[0] - x, s.pivot2Px[1] - y) < near) return "rotate"
        if (s.sourcePx && s.sourcePx.length && (closed ? Draw.inside(s.sourcePx, x, y) : Draw.distance(s.sourcePx, x, y, false) < near))
            return "source"
        const isSelected = s.id === selected
        if (isSelected && closed && Draw.distance(s.outlinePx, x, y, true) < near) return "size"
        if (isSelected && s.borderPx.length && Draw.distance(s.borderPx, x, y, closed) < near) return "feather"
        if (closed && Draw.inside(s.outlinePx, x, y)) return "move"
        if (!closed && Draw.distance(s.outlinePx, x, y, false) < near) return "move"
        if (s.type === "brush" && s.borderPx.length && Draw.inside(s.borderPx, x, y)) return "move"
        return ""
    }
    function hitAt(x, y) {
        const order = drawn.slice().reverse()
        order.sort((a, b) => (b.id === selected) - (a.id === selected))
        for (const s of order) {
            const kind = hitShape(s, x, y)
            if (kind) return { kind: kind, id: s.id, shape: s }
        }
        return null
    }

    onShapesChanged: { if (!drag) { frozen = null; settled = null } paint.requestPaint() }
    onDragChanged: paint.requestPaint()
    onHoverChanged: paint.requestPaint()
    onPathPointsChanged: paint.requestPaint()
    onBrushPointsChanged: paint.requestPaint()
    onPendingSourceChanged: paint.requestPaint()
    onWidthChanged: paint.requestPaint()
    onHeightChanged: paint.requestPaint()

    Canvas {
        id: paint
        anchors.fill: parent
        onPaint: {
            const ctx = getContext("2d")
            ctx.clearRect(0, 0, width, height)
            for (const raw of root.drawn) {
                const s = root.dragged(raw)
                const on = s.id === root.selected || (root.hover && root.hover.id === s.id)
                Draw.stroke(ctx, s.outlinePx, s.closed, on)
                if (s.borderPx.length) Draw.stroke(ctx, s.borderPx, s.closed, false, null, true)
                if (s.sourcePx.length) {
                    Draw.stroke(ctx, s.sourcePx, s.closed, on, "rgba(255,255,255,0.7)", true)
                    if (s.sourceCenterPx && s.centerPx)
                        Draw.arrow(ctx, s.sourceCenterPx, s.centerPx, 8, false)
                }
                if (s.centerPx && s.type !== "gradient") Draw.cross(ctx, s.centerPx[0], s.centerPx[1], on ? 6 : 4)
                if (s.pivotPx && on) Draw.handle(ctx, s.pivotPx[0], s.pivotPx[1], 4, true, true)
                if (s.pivot2Px && on) Draw.handle(ctx, s.pivot2Px[0], s.pivot2Px[1], 4, true, true)
            }
            if (root.pathPoints.length) {
                const pts = Draw.scale(root.pathPoints, width, height)
                Draw.stroke(ctx, pts, false, true)
                for (const p of pts) Draw.handle(ctx, p[0], p[1], 3, false, true)
            }
            if (root.brushPoints.length) Draw.stroke(ctx, Draw.scale(root.brushPoints, width, height), false, true)
            if (root.gradientFrom && root.drag && root.drag.kind === "gradient")
                Draw.arrow(ctx, [root.gradientFrom[0] * width, root.gradientFrom[1] * height],
                           [root.drag.to[0] * width, root.drag.to[1] * height], 10, true)
            if (root.pendingSource)
                Draw.cross(ctx, root.pendingSource[0] * width, root.pendingSource[1] * height, 8)
        }
    }

    MouseArea {
        id: area
        anchors.fill: parent
        enabled: root.editable
        hoverEnabled: true
        preventStealing: true
        acceptedButtons: Qt.LeftButton | Qt.RightButton
        cursorShape: root.mode ? Qt.CrossCursor
                   : !root.hover ? Qt.ArrowCursor
                   : root.hover.kind === "move" || root.hover.kind === "source" ? Qt.SizeAllCursor
                   : root.hover.kind === "rotate" ? Qt.PointingHandCursor : Qt.SizeFDiagCursor
        function frac(mouse) { return [mouse.x / width, mouse.y / height] }
        onPressed: mouse => {
            const p = frac(mouse)
            if (root.mode) {
                if (mouse.button === Qt.RightButton) {
                    if (root.mode === "path" && root.pathPoints.length >= 3) root.finishPath()
                    else root.cancel()
                    return
                }
                if ((mouse.modifiers & Qt.ShiftModifier) && root.clones()) { root.pendingSource = p; return }
                if (root.mode === "brush") { root.brushPoints = [p]; root.drag = { kind: "brush" }; return }
                if (root.mode === "gradient") { root.gradientFrom = p; root.drag = { kind: "gradient", to: p }; return }
                if (root.mode === "path") {
                    const first = root.pathPoints[0]
                    if (first && root.pathPoints.length >= 3 && Math.hypot((first[0] - p[0]) * width, (first[1] - p[1]) * height) < root.near) {
                        root.finishPath(); return
                    }
                    root.pathPoints = root.pathPoints.concat([p]); return
                }
                root.add({ action: "add", type: root.mode, at: p })
                if (!root.continuous) root.mode = ""
                return
            }
            const hit = root.hitAt(mouse.x, mouse.y)
            if (!hit) {
                if (root.selected && mouse.button === Qt.LeftButton) root.edited({ action: "select", id: 0 })
                mouse.accepted = false       // the photograph pans
                return
            }
            if (mouse.button === Qt.RightButton) { root.edited({ action: "remove", id: hit.id }); return }
            if (hit.id !== root.selected) root.edited({ action: "select", id: hit.id })
            root.frozen = root.shapes
            root.drag = { kind: hit.kind, id: hit.id, start: [mouse.x, mouse.y], last: p, total: [0, 0],
                          factor: 1, angle: 0, center: hit.shape.centerPx }
            root.interactionChanged(true)
        }
        onPositionChanged: mouse => {
            const p = frac(mouse)
            if (!pressed || !root.drag) { if (!root.mode) root.hover = root.hitAt(mouse.x, mouse.y); return }
            const d = Object.assign({}, root.drag)
            if (d.kind === "brush") {
                const last = root.brushPoints[root.brushPoints.length - 1]
                if (Math.hypot((last[0] - p[0]) * width, (last[1] - p[1]) * height) >= 2) root.brushPoints = root.brushPoints.concat([p])
                return
            }
            if (d.kind === "gradient") { d.to = p; root.drag = d; return }
            const c = d.center || [mouse.x, mouse.y]
            if (d.kind === "move" || d.kind === "source") {
                root.edited({ action: d.kind === "move" ? "move" : "move-source", id: d.id, from: d.last, to: p })
                d.total = [mouse.x - d.start[0], mouse.y - d.start[1]]
            } else if (d.kind === "size" || d.kind === "feather") {
                const before = Math.hypot(d.last[0] * width - c[0], d.last[1] * height - c[1])
                const after = Math.hypot(mouse.x - c[0], mouse.y - c[1])
                if (before > 1 && after > 1) {
                    root.edited({ action: d.kind === "size" ? "scale" : "feather", id: d.id, factor: after / before })
                    d.factor *= after / before
                }
            } else if (d.kind === "rotate") {
                root.edited({ action: "rotate", id: d.id, at: p })
                d.angle = Math.atan2(mouse.y - c[1], mouse.x - c[0]) - Math.atan2(d.start[1] - c[1], d.start[0] - c[0])
            }
            d.last = p
            root.drag = d
        }
        onReleased: mouse => {
            const d = root.drag
            if (!d) return
            root.drag = null
            if (d.kind === "brush") {
                root.add({ action: "add", type: "brush", points: root.brushPoints })
                root.brushPoints = []
                if (!root.continuous) root.mode = ""
            } else if (d.kind === "gradient") {
                root.add({ action: "add", type: "gradient", at: root.gradientFrom, to: d.to })
                root.gradientFrom = null
                if (!root.continuous) root.mode = ""
            } else {
                root.settled = d
                root.interactionChanged(false)
            }
        }
        onDoubleClicked: if (root.mode === "path") root.finishPath()
        onCanceled: { root.drag = null; root.settled = null; root.frozen = null; root.interactionChanged(false) }
        onExited: root.hover = null
        onWheel: wheel => {
            const hit = root.hitAt(wheel.x, wheel.y)
            const s = hit ? hit.shape : null
            if (!s || root.mode) { wheel.accepted = false; return }
            const up = wheel.angleDelta.y > 0
            const ctrl = wheel.modifiers & Qt.ControlModifier, shift = wheel.modifiers & Qt.ShiftModifier
            if (ctrl && !shift)
                root.edited({ action: "opacity", id: s.id, value: Math.max(0.05, Math.min(1, s.opacity + (up ? 0.05 : -0.05))) })
            else
                root.edited({ action: "scroll", id: s.id, up: up, modifier: ctrl && shift ? "shift+ctrl" : shift ? "shift" : "" })
            wheel.accepted = true
        }
    }
}
