import QtQuick

// color calibration's colour checker on the photo (channelmixerrgb.c gui_post_expose 2682,
// mouse_moved / button_pressed / button_released 2530-2680): the chart's four corners (top left,
// top right, bottom right, bottom left, fractions of the displayed image) with the patch squares
// drawn through the perspective they span, each at darktable's patch scale (safety margin).
// Drag a corner to move it, drag inside to move the whole chart; the corners are reported on
// release (boxEdited) and the module then measures the patches again.
Item {
    id: root
    property var box: [0.01, 0.01, 0.99, 0.01, 0.99, 0.99, 0.01, 0.99]
    property var chart: null          // { patches: [[x, y], …] in the unit square, ratio, radius }
    property real safety: 0.5
    signal boxEdited(var box)
    property var draft: null
    readonly property var shown: draft || box

    // The projective map of the unit square onto the four corners (Heckbert's square-to-quad).
    function homography(q) {
        const x0 = q[0], y0 = q[1], x1 = q[2], y1 = q[3], x2 = q[4], y2 = q[5], x3 = q[6], y3 = q[7]
        const dx1 = x1 - x2, dx2 = x3 - x2, dy1 = y1 - y2, dy2 = y3 - y2
        const sx = x0 - x1 + x2 - x3, sy = y0 - y1 + y2 - y3
        const den = dx1 * dy2 - dx2 * dy1
        const g = den !== 0 ? (sx * dy2 - dx2 * sy) / den : 0
        const h = den !== 0 ? (dx1 * sy - sx * dy1) / den : 0
        return [x1 - x0 + g * x1, x3 - x0 + h * x3, x0, y1 - y0 + g * y1, y3 - y0 + h * y3, y0, g, h, 1]
    }
    function map(H, u, v) {
        const w = H[6] * u + H[7] * v + H[8]
        return [(H[0] * u + H[1] * v + H[2]) / w * width, (H[3] * u + H[4] * v + H[5]) / w * height]
    }
    Canvas {
        id: canvas
        anchors.fill: parent
        onWidthChanged: requestPaint()
        onHeightChanged: requestPaint()
        Connections { target: root; function onShownChanged() { canvas.requestPaint() } function onChartChanged() { canvas.requestPaint() }
                      function onSafetyChanged() { canvas.requestPaint() } }
        onPaint: {
            const c = getContext("2d")
            c.clearRect(0, 0, width, height)
            const q = root.shown, H = root.homography(q)
            const outline = (pts, color, w) => {
                c.strokeStyle = color; c.lineWidth = w
                c.beginPath(); c.moveTo(pts[0][0], pts[0][1])
                for (let i = 1; i < pts.length; ++i) c.lineTo(pts[i][0], pts[i][1])
                c.closePath(); c.stroke()
            }
            const frame = [[0, 0], [1, 0], [1, 1], [0, 1]].map(p => root.map(H, p[0], p[1]))
            outline(frame, "#b0000000", 3); outline(frame, "white", 1)
            const ch = root.chart
            if (ch && ch.patches) {
                // _extract_patches: radius_x = radius × hypot(1, ratio) × safety, radius_y = radius_x / ratio
                const rx = ch.radius * Math.hypot(1, ch.ratio) * root.safety, ry = rx / ch.ratio
                for (const p of ch.patches) {
                    const corners = [[p[0] - rx, p[1] - ry], [p[0] + rx, p[1] - ry], [p[0] + rx, p[1] + ry], [p[0] - rx, p[1] + ry]]
                    outline(corners.map(k => root.map(H, k[0], k[1])), "white", 1)
                }
            }
            for (const f of frame) { c.fillStyle = "white"; c.strokeStyle = "#b0000000"; c.lineWidth = 1
                                     c.fillRect(f[0] - 4, f[1] - 4, 8, 8); c.strokeRect(f[0] - 4, f[1] - 4, 8, 8) }
        }
    }
    MouseArea {
        anchors.fill: parent
        preventStealing: true
        property int corner: -1
        property var start: null
        onPressed: mouse => {
            const q = root.shown
            let best = -1, dist = 20
            for (let i = 0; i < 4; ++i) {
                const d = Math.hypot(q[2 * i] * width - mouse.x, q[2 * i + 1] * height - mouse.y)
                if (d < dist) { dist = d; best = i }
            }
            corner = best
            start = [mouse.x / width, mouse.y / height, q.slice()]
            root.draft = q.slice()
        }
        onPositionChanged: mouse => {
            if (!start) return
            const next = start[2].slice(), dx = mouse.x / width - start[0], dy = mouse.y / height - start[1]
            const clamp = v => Math.max(0, Math.min(1, v))
            if (corner >= 0) { next[2 * corner] = clamp(start[2][2 * corner] + dx); next[2 * corner + 1] = clamp(start[2][2 * corner + 1] + dy) }
            else for (let i = 0; i < 4; ++i) { next[2 * i] = clamp(start[2][2 * i] + dx); next[2 * i + 1] = clamp(start[2][2 * i + 1] + dy) }
            root.draft = next
        }
        onReleased: { const b = root.draft; root.draft = null; start = null; if (b) root.boxEdited(b) }
        onCanceled: { root.draft = null; start = null }
    }
}
