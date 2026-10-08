.pragma library
// Drawing and hit testing for the on-canvas tools. Points are [x, y] in item pixels; null
// breaks a polyline. Lines are drawn like darktable's overlays (dt_draw_set_color_overlay):
// a dark halo under a light line, so they read on bright and dark photos alike.

function trace(ctx, pts, closed) {
    ctx.beginPath()
    let pen = false
    for (const p of pts) {
        if (!p) { pen = false; continue }
        if (!pen) { ctx.moveTo(p[0], p[1]); pen = true } else ctx.lineTo(p[0], p[1])
    }
    if (closed && pen) ctx.closePath()
}

function stroke(ctx, pts, closed, selected, color, dashed) {
    if (!pts || !pts.length) return
    for (let pass = 0; pass < 2; ++pass) {
        trace(ctx, pts, closed)
        ctx.setLineDash(dashed ? [4, 4] : [])
        ctx.lineWidth = pass ? (selected ? 2 : 1) : (selected ? 5 : 3)
        ctx.strokeStyle = pass ? (color || "rgba(255,255,255,0.92)") : "rgba(0,0,0,0.5)"
        ctx.stroke()
    }
    ctx.setLineDash([])
}

function handle(ctx, x, y, radius, selected, filled) {
    ctx.beginPath()
    ctx.arc(x, y, radius, 0, Math.PI * 2)
    ctx.lineWidth = 3
    ctx.strokeStyle = "rgba(0,0,0,0.5)"
    ctx.stroke()
    ctx.lineWidth = selected ? 2 : 1
    ctx.strokeStyle = "rgba(255,255,255,0.95)"
    ctx.stroke()
    if (filled) {
        ctx.fillStyle = selected ? "rgba(255,255,255,0.95)" : "rgba(255,255,255,0.55)"
        ctx.fill()
    }
}

function cross(ctx, x, y, size) {
    stroke(ctx, [[x - size, y], [x + size, y], null, [x, y - size], [x, y + size]], false, false)
}

// A small arrow head at b, pointing away from a.
function arrow(ctx, a, b, size, selected) {
    const angle = Math.atan2(b[1] - a[1], b[0] - a[0])
    const left = [b[0] - size * Math.cos(angle - 0.45), b[1] - size * Math.sin(angle - 0.45)]
    const right = [b[0] - size * Math.cos(angle + 0.45), b[1] - size * Math.sin(angle + 0.45)]
    stroke(ctx, [a, b, null, left, b, right], false, selected)
}

function scale(pts, w, h) {
    return (pts || []).map(p => p ? [p[0] * w, p[1] * h] : null)
}

function inside(pts, x, y) {
    let result = false
    const n = pts.length
    for (let i = 0, j = n - 1; i < n; j = i++) {
        const a = pts[i], b = pts[j]
        if (!a || !b) continue
        if (((a[1] > y) !== (b[1] > y)) && (x < (b[0] - a[0]) * (y - a[1]) / (b[1] - a[1]) + a[0])) result = !result
    }
    return result
}

function segmentDistance(a, b, x, y) {
    const dx = b[0] - a[0], dy = b[1] - a[1]
    const l = dx * dx + dy * dy
    const t = l > 0 ? Math.max(0, Math.min(1, ((x - a[0]) * dx + (y - a[1]) * dy) / l)) : 0
    return Math.hypot(a[0] + t * dx - x, a[1] + t * dy - y)
}

function distance(pts, x, y, closed) {
    let best = Infinity
    for (let i = 1; i < pts.length; ++i)
        if (pts[i - 1] && pts[i]) best = Math.min(best, segmentDistance(pts[i - 1], pts[i], x, y))
    if (closed && pts.length > 2 && pts[0] && pts[pts.length - 1])
        best = Math.min(best, segmentDistance(pts[pts.length - 1], pts[0], x, y))
    return best
}

function translate(pts, dx, dy) {
    return (pts || []).map(p => p ? [p[0] + dx, p[1] + dy] : null)
}

function scaleAround(pts, c, factor) {
    return (pts || []).map(p => p ? [c[0] + (p[0] - c[0]) * factor, c[1] + (p[1] - c[1]) * factor] : null)
}

function rotateAround(pts, c, angle) {
    const s = Math.sin(angle), k = Math.cos(angle)
    return (pts || []).map(p => p ? [c[0] + (p[0] - c[0]) * k - (p[1] - c[1]) * s,
                                     c[1] + (p[0] - c[0]) * s + (p[1] - c[1]) * k] : null)
}
// The point of the line through a and b nearest to p (a border handle sliding along its line).
function project(a, b, p) {
    const dx = b[0] - a[0], dy = b[1] - a[1], l2 = dx * dx + dy * dy
    if (l2 <= 0) return [a[0], a[1]]
    const t = ((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / l2
    return [a[0] + t * dx, a[1] + t * dy]
}
