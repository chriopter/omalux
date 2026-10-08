import QtQuick
import "CanvasDraw.js" as Draw

// graduated density's line on the photo (src/iop/graduatednd.c:486–706): drag an end to
// turn the line, the line itself to move it, or right-drag (or "draw line") for a new one.
// As in darktable the module takes the line only on release; the engine converts it into
// rotation and offset and reports the line it draws for them.
Item {
    id: root
    required property var theme
    property var overlay: ({})
    property bool editable: true
    signal edited(var gesture)
    signal interactionChanged(bool active)

    property bool drawing: false
    readonly property var tools: [{ key: "draw", label: "draw line", checked: drawing,
                                    tooltip: "drag on the photo to draw a new gradient line (or right-drag)" }]
    function toolClicked(key, modifiers) { if (key === "draw") drawing = !drawing }
    readonly property bool capturing: drawing

    readonly property var line: overlay && overlay.line ? overlay.line : null
    // The dragged line (fractions) until release.
    property var local: null
    readonly property var shown: local || (line ? { a: line.a, b: line.b } : null)
    property int selected: 0     // 1 start, 2 end, 3 line
    property int dragging: 0
    property var last: null
    readonly property real near: 10

    function at(p) { return [p[0] * width, p[1] * height] }
    function hit(x, y) {
        if (!shown) return 0
        const a = at(shown.a), b = at(shown.b)
        if (Math.abs(x - a[0]) < near && Math.abs(y - a[1]) < near) return 1
        if (Math.abs(x - b[0]) < near && Math.abs(y - b[1]) < near) return 2
        if (Draw.segmentDistance(a, b, x, y) < near * 0.7) return 3
        return 0
    }
    onShownChanged: paint.requestPaint()
    onSelectedChanged: paint.requestPaint()
    onWidthChanged: paint.requestPaint()
    onLineChanged: if (!dragging) local = null

    Canvas {
        id: paint
        anchors.fill: parent
        onPaint: {
            const ctx = getContext("2d")
            ctx.clearRect(0, 0, width, height)
            if (!root.shown) return
            const a = root.at(root.shown.a), b = root.at(root.shown.b)
            const active = root.selected === 3 || root.dragging === 3
            Draw.stroke(ctx, [a, b], false, active)
            // The ends as small triangles, as darktable draws them (graduatednd.c:529).
            const l = Math.hypot(b[0] - a[0], b[1] - a[1]) || 1
            const ext = width * 0.01
            const tri = (p, q, flip, on) => {
                const x1 = p[0] + (q[0] - p[0]) * ext / l, y1 = p[1] + (q[1] - p[1]) * ext / l
                const x2 = (p[0] + x1) / 2 - flip * (y1 - p[1]), y2 = (p[1] + y1) / 2 + flip * (x1 - p[0])
                ctx.beginPath(); ctx.moveTo(p[0], p[1]); ctx.lineTo(x1, y1); ctx.lineTo(x2, y2); ctx.closePath()
                ctx.fillStyle = on ? "rgba(255,255,255,1)" : "rgba(255,255,255,0.5)"
                ctx.fill()
                ctx.lineWidth = 1; ctx.strokeStyle = on ? "rgba(0,0,0,1)" : "rgba(0,0,0,0.5)"; ctx.stroke()
            }
            tri(a, b, 1, root.selected === 1 || root.dragging === 1)
            tri(b, a, -1, root.selected === 2 || root.dragging === 2)
        }
    }
    MouseArea {
        anchors.fill: parent
        enabled: root.editable
        hoverEnabled: true
        preventStealing: true
        acceptedButtons: Qt.LeftButton | Qt.RightButton
        cursorShape: root.selected === 3 ? Qt.SizeAllCursor : root.selected ? Qt.CrossCursor : Qt.ArrowCursor
        onPositionChanged: mouse => {
            const p = [mouse.x / width, mouse.y / height]
            if (!pressed) { root.selected = root.hit(mouse.x, mouse.y); return }
            const s = root.local
            if (root.dragging === 1) root.local = { a: p, b: s.b }
            else if (root.dragging === 2) root.local = { a: s.a, b: p }
            else if (root.dragging === 3) {
                const dx = p[0] - root.last[0], dy = p[1] - root.last[1]
                root.local = { a: [s.a[0] + dx, s.a[1] + dy], b: [s.b[0] + dx, s.b[1] + dy] }
            }
            root.last = p
        }
        onPressed: mouse => {
            const p = [mouse.x / width, mouse.y / height]
            const drawNew = mouse.button === Qt.RightButton || root.drawing
            const target = drawNew ? 0 : root.hit(mouse.x, mouse.y)
            if (!drawNew && !target) { mouse.accepted = false; return }
            root.dragging = drawNew ? 2 : target
            root.local = drawNew ? { a: p, b: p } : { a: root.shown.a, b: root.shown.b }
            root.last = p
            root.interactionChanged(true)
        }
        onReleased: {
            const s = root.local, kind = root.dragging
            root.dragging = 0
            root.interactionChanged(false)
            if (s && Math.hypot((s.b[0] - s.a[0]) * width, (s.b[1] - s.a[1]) * height) > 4)
                root.edited({ action: "line", a: s.a, b: s.b, keepRotation: kind === 3 })
            else root.local = null
            root.drawing = false
        }
        onCanceled: { root.dragging = 0; root.local = null; root.interactionChanged(false) }
    }
}
