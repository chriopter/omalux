import QtQuick
import "CanvasDraw.js" as Draw

// vignetting on the photo, ported from darktable's gui_post_expose and mouse_moved
// (src/iop/vignette.c:381, 469): the centre, the inner ellipse where the fall-off starts and
// the outer one where it ends, with handles for the centre, the width and height (Ctrl: the
// size instead of the ratio) and the fall-off. darktable works in preview pixels; the
// fractions are the same in the photo's displayed pixels. Edits leave as parameter changes.
Item {
    id: root
    required property var theme
    property var moduleState
    property bool editable: true
    property var tools: []
    signal parametersEdited(var changes)
    signal interactionChanged(bool active)
    function toolClicked(key, modifiers) {}

    readonly property var stored: moduleState ? moduleState.values : ({})
    // The values in use: the module's, or the ones just dragged until the engine reports them.
    property var local: null
    readonly property var p: local || ({
        cx: Number(stored["center.x"] || 0), cy: Number(stored["center.y"] || 0),
        scale: stored.scale !== undefined ? Number(stored.scale) : 80,
        falloff: stored.falloff_scale !== undefined ? Number(stored.falloff_scale) : 50,
        whratio: stored.whratio !== undefined ? Number(stored.whratio) : 1,
        autoratio: Number(stored.autoratio || 0) > 0.5 })
    property int grab: 0
    readonly property real grabRadius: 8

    // vignette.c:393–451 in displayed pixels.
    function geometry(v) {
        const wd = width, ht = height
        const big = Math.max(wd, ht), small = Math.min(wd, ht)
        const g = { x: (v.cx + 1) * 0.5 * wd, y: (v.cy + 1) * 0.5 * ht,
                    w: v.scale * 0.01 * 0.5 * wd, h: v.scale * 0.01 * 0.5 * ht }
        g.fx = g.w + v.falloff * 0.01 * 0.5 * wd
        g.fy = g.h + v.falloff * 0.01 * 0.5 * ht
        if (!v.autoratio) {
            const f1 = big / small
            if (wd >= ht) {
                const f2 = (2.0 - v.whratio) * f1
                if (v.whratio <= 1) { g.h *= f1; g.w *= v.whratio; g.fx *= v.whratio; g.fy *= f1 }
                else { g.h *= f2; g.fy *= f2 }
            } else {
                const f2 = v.whratio * f1
                if (v.whratio <= 1) { g.w *= f2; g.fx *= f2 }
                else { g.w *= f1; g.h *= (2.0 - v.whratio); g.fx *= f1; g.fy *= (2.0 - v.whratio) }
            }
        }
        return g
    }
    // _get_grab (vignette.c:274): handles relative to the centre.
    function grabAt(x, y) {
        const g = geometry(p), px = x - g.x, py = y - g.y, r2 = grabRadius * grabRadius
        if ((px - g.w) * (px - g.w) + py * py <= r2) return 2
        if (px * px + (py + g.h) * (py + g.h) <= r2) return 4
        if (px * px + py * py <= r2) return 1
        if ((px - g.fx) * (px - g.fx) + py * py <= r2) return 8
        if (px * px + (py + g.fy) * (py + g.fy) <= r2) return 16
        return 0
    }
    function clamp(v, a, b) { return Math.max(a, Math.min(b, v)) }
    // mouse_moved while the primary button is down (vignette.c:550–636).
    function dragTo(x, y, ctrl) {
        const v = Object.assign({}, p), g = geometry(v)
        const wd = width, ht = height, big = Math.max(wd, ht)
        const pzx = x / wd, pzy = y / ht
        const changes = {}
        if (grab === 1) {
            v.cx = clamp(pzx * 2.0 - 1.0, -1, 1); v.cy = clamp(pzy * 2.0 - 1.0, -1, 1)
            changes["center.x"] = v.cx; changes["center.y"] = v.cy
        } else if (grab === 2) {
            const max = 0.5 * (v.whratio <= 1.0 ? big * v.whratio : big)
            const w = Math.min(big, Math.max(0.1, pzx * wd - g.x))
            const ratio = w / g.h, scale = clamp(100.0 * w / max, 0, 200)
            if (ratio <= 1.0) {
                if (ctrl) { v.scale = scale; changes.scale = scale }
                else { v.whratio = clamp(ratio, 0, 2); changes.whratio = v.whratio }
            } else {
                v.scale = scale; changes.scale = scale
                if (!ctrl) { v.whratio = clamp(2.0 - 1.0 / ratio, 0, 2); changes.whratio = v.whratio }
            }
        } else if (grab === 4) {
            const h = Math.min(big, Math.max(0.1, g.y - pzy * ht))
            const ratio = h / g.w
            const max = 0.5 * (ratio <= 1.0 ? big * (2.0 - v.whratio) : big)
            if (ratio <= 1.0) {
                if (ctrl) { v.scale = clamp(100.0 * h / max, 0, 200); changes.scale = v.scale }
                else { v.whratio = clamp(2.0 - ratio, 0, 2); changes.whratio = v.whratio }
            } else {
                v.scale = clamp(100.0 * h / max, 0, 200); changes.scale = v.scale
                if (!ctrl) { v.whratio = clamp(1.0 / ratio, 0, 2); changes.whratio = v.whratio }
            }
        } else if (grab === 8) {
            const max = 0.5 * (v.whratio <= 1.0 ? big * v.whratio : big)
            const dx = Math.min(2.0 * max, Math.max(0.0, (pzx * wd - g.x) - g.w))
            v.falloff = clamp(100.0 * dx / max, 0, 200); changes.falloff_scale = v.falloff
        } else if (grab === 16) {
            const max = 0.5 * (v.whratio > 1.0 ? big * (2.0 - v.whratio) : big)
            const dy = Math.min(2.0 * max, Math.max(0.0, (g.y - pzy * ht) - g.h))
            v.falloff = clamp(100.0 * dy / max, 0, 200); changes.falloff_scale = v.falloff
        }
        local = v
        if (Object.keys(changes).length) parametersEdited(changes)
    }
    onStoredChanged: if (!area.pressed) local = null
    onPChanged: paint.requestPaint()
    onWidthChanged: paint.requestPaint()
    onHeightChanged: paint.requestPaint()
    onGrabChanged: paint.requestPaint()

    Canvas {
        id: paint
        anchors.fill: parent
        onPaint: {
            const ctx = getContext("2d")
            ctx.clearRect(0, 0, width, height)
            const g = root.geometry(root.p)
            const ellipse = (a, b) => {
                const pts = []
                for (let i = 0; i <= 96; ++i) {
                    const t = i * Math.PI * 2 / 96
                    pts.push([g.x + a * Math.cos(t), g.y + b * Math.sin(t)])
                }
                return pts
            }
            Draw.cross(ctx, g.x, g.y, 10)
            Draw.stroke(ctx, ellipse(g.w, g.h), true, false)
            Draw.stroke(ctx, ellipse(g.fx, g.fy), true, false, null, true)
            const handles = [[1, 0, 0], [2, g.w, 0], [4, 0, -g.h], [8, g.fx, 0], [16, 0, -g.fy]]
            for (const h of handles)
                Draw.handle(ctx, g.x + h[1], g.y + h[2], root.grab === h[0] ? 6 : 4, root.grab === h[0], false)
        }
    }
    MouseArea {
        id: area
        anchors.fill: parent
        enabled: root.editable
        hoverEnabled: true
        preventStealing: true
        cursorShape: root.grab === 1 ? Qt.SizeAllCursor : root.grab === 2 || root.grab === 8 ? Qt.SizeHorCursor
                   : root.grab === 4 || root.grab === 16 ? Qt.SizeVerCursor : Qt.ArrowCursor
        onPositionChanged: mouse => {
            if (pressed) root.dragTo(mouse.x, mouse.y, mouse.modifiers & Qt.ControlModifier)
            else root.grab = root.grabAt(mouse.x, mouse.y)
        }
        onPressed: mouse => {
            root.grab = root.grabAt(mouse.x, mouse.y)
            // Away from the handles the photograph pans, as in darktable.
            if (!root.grab || mouse.button !== Qt.LeftButton) { mouse.accepted = false; return }
            root.local = Object.assign({}, root.p)
            root.interactionChanged(true)
        }
        onReleased: { root.interactionChanged(false) }
        onCanceled: root.interactionChanged(false)
        onExited: if (!pressed) root.grab = 0
    }
}
