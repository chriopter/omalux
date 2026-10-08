.pragma library

// Curve interpolation as darktable draws it, for CurveEditor and GraphView.
//
// Types follow darktable/src/common/curve_tools.h: CUBIC_SPLINE ("cubic", natural cubic
// spline), CATMULL_ROM ("catmull", Hermite with central-difference tangents, which is what
// darktable calls Catmull-Rom) and MONOTONE_HERMITE ("monotone"). "linear" is a plain
// polyline for data darktable itself connects straight.
//
// Two samplers exist upstream and they differ at the edges:
//   version 1  common/curve_tools.c (tonecurve, basecurve): flat outside the first/last node,
//              monotone tangents without the sign test.
//   version 2  common/splines.cpp (rgbcurve, colorzones): linear extrapolation with the end
//              tangent, optional periodic boundary (colorzones hue axis).
// Both clamp the result to [0, 1].

function sortedNodes(nodes, periodic) {
    const pts = []
    for (let i = 0; i < nodes.length; ++i) {
        let x = Number(nodes[i].x)
        if (periodic) { x = x % 1; if (x < 0) x += 1 }
        pts.push({ x: x, y: Number(nodes[i].y), dy: 0 })
    }
    pts.sort((a, b) => a.x - b.x)
    return pts
}

function solve(A, b) {
    // Dense Gaussian elimination with partial pivoting; n is at most 20.
    const n = b.length
    for (let c = 0; c < n; ++c) {
        let p = c
        for (let r = c + 1; r < n; ++r) if (Math.abs(A[r][c]) > Math.abs(A[p][c])) p = r
        if (p !== c) { const t = A[p]; A[p] = A[c]; A[c] = t; const u = b[p]; b[p] = b[c]; b[c] = u }
        const d = A[c][c]
        if (Math.abs(d) < 1e-12) continue
        for (let r = c + 1; r < n; ++r) {
            const f = A[r][c] / d
            if (f === 0) continue
            for (let k = c; k < n; ++k) A[r][k] -= f * A[c][k]
            b[r] -= f * b[c]
        }
    }
    for (let r = n - 1; r >= 0; --r) {
        let s = b[r]
        for (let k = r + 1; k < n; ++k) s -= A[r][k] * b[k]
        b[r] = Math.abs(A[r][r]) < 1e-12 ? 0 : s / A[r][r]
    }
    return b
}

function tangentsCubic(p, periodic) {
    const N = p.length
    const dx = [], dy = []
    for (let i = 0; i < N - 1; ++i) { dx.push(p[i + 1].x - p[i].x); dy.push(p[i + 1].y - p[i].y) }
    if (periodic) { dx.push(p[0].x - p[N - 1].x + 1); dy.push(p[0].y - p[N - 1].y) }
    const A = [], b = []
    for (let i = 0; i < N; ++i) { A.push(new Array(N).fill(0)); b.push(0) }
    for (let i = 1; i < N - 1; ++i) {
        A[i][i - 1] = dx[i - 1] / 6
        A[i][i] = (dx[i - 1] + dx[i]) / 3
        A[i][i + 1] = dx[i] / 6
        b[i] = dy[i] / dx[i] - dy[i - 1] / dx[i - 1]
    }
    if (periodic) {
        A[0][0] = (dx[N - 1] + dx[0]) / 3
        A[N - 1][N - 1] = (dx[N - 2] + dx[N - 1]) / 3
        b[0] = dy[0] / dx[0] - dy[N - 1] / dx[N - 1]
        b[N - 1] = dy[N - 1] / dx[N - 1] - dy[N - 2] / dx[N - 2]
        if (N > 2) {
            A[0][1] = dx[0] / 6
            A[N - 1][N - 2] = dx[N - 2] / 6
            A[0][N - 1] = A[N - 1][0] = dx[N - 1] / 6
        } else {
            A[0][1] = A[1][0] = (dx[0] + dx[1]) / 6
        }
    } else {
        A[0][0] = 1; A[N - 1][N - 1] = 1
    }
    solve(A, b)
    let c = 0
    for (let i = 0; i < N - 1; ++i) {
        c = dy[i] / dx[i] - dx[i] / 6 * (b[i + 1] - b[i])
        p[i].dy = -dx[i] * b[i] / 2 + c
    }
    p[N - 1].dy = periodic ? dx[N - 2] * b[N - 1] / 2 + c : c
}

function tangentsCatmull(p, periodic) {
    const N = p.length
    if (periodic) {
        p[0].dy = (p[1].y - p[N - 1].y) / (p[1].x - p[N - 1].x + 1)
        p[N - 1].dy = (p[0].y - p[N - 2].y) / (p[0].x - p[N - 2].x + 1)
    } else {
        p[0].dy = (p[1].y - p[0].y) / (p[1].x - p[0].x)
        p[N - 1].dy = (p[N - 1].y - p[N - 2].y) / (p[N - 1].x - p[N - 2].x)
    }
    for (let i = 1; i < N - 1; ++i) p[i].dy = (p[i + 1].y - p[i - 1].y) / (p[i + 1].x - p[i - 1].x)
}

function limitMonotone(p, delta, i, j) {
    if (Math.abs(delta[i]) < 1.1920929e-7) { p[i].dy = 0; p[j].dy = 0; return }
    const a = p[i].dy / delta[i], b = p[j].dy / delta[i], tau = a * a + b * b
    if (tau > 9) { const s = 3 * delta[i] / Math.sqrt(tau); p[i].dy = a * s; p[j].dy = b * s }
}

function tangentsMonotone(p, periodic, version) {
    const N = p.length
    const delta = []
    for (let i = 0; i < N - 1; ++i) delta.push((p[i + 1].y - p[i].y) / (p[i + 1].x - p[i].x))
    if (version === 1) {
        // curve_tools.c: averaged slopes, last segment slope repeated, no sign test.
        delta.push(delta[N - 2])
        const m = new Array(N + 1).fill(0)
        m[0] = delta[0]; m[N - 1] = delta[N - 1]
        for (let i = 1; i < N - 1; ++i) m[i] = (delta[i - 1] + delta[i]) / 2
        for (let i = 0; i < N; ++i) {
            if (Math.abs(delta[i]) < 1.1920929e-7) { m[i] = 0; m[i + 1] = 0 }
            else {
                const a = m[i] / delta[i], b = m[i + 1] / delta[i], tau = a * a + b * b
                if (tau > 9) { const s = 3 * delta[i] / Math.sqrt(tau); m[i] = a * s; m[i + 1] = b * s }
            }
        }
        for (let i = 0; i < N; ++i) p[i].dy = m[i]
        return
    }
    const avg = (a, b) => a * b <= 0 ? 0 : (a + b) / 2
    if (periodic) {
        delta.push((p[0].y - p[N - 1].y) / (p[0].x - p[N - 1].x + 1))
        p[0].dy = avg(delta[N - 1], delta[0])
        for (let i = 1; i < N; ++i) p[i].dy = avg(delta[i - 1], delta[i])
        for (let i = 0; i < N; ++i) limitMonotone(p, delta, i, i + 1 < N ? i + 1 : 0)
    } else {
        p[0].dy = delta[0]
        for (let i = 1; i < N - 1; ++i) p[i].dy = avg(delta[i - 1], delta[i])
        p[N - 1].dy = delta[N - 2]
        for (let i = 0; i < N - 1; ++i) limitMonotone(p, delta, i, i + 1)
    }
}

// Returns a prepared spline: { points, type, periodic, version }.
function prepare(nodes, type, periodic, version) {
    const p = sortedNodes(nodes || [], periodic)
    const s = { points: p, type: type || "cubic", periodic: !!periodic, version: version === 1 && !periodic ? 1 : 2 }
    if (p.length < 2) return s
    for (let i = 0; i < p.length - 1; ++i) if (p[i + 1].x <= p[i].x) p[i + 1].x = p[i].x + 1e-6
    if (s.type === "catmull") tangentsCatmull(p, s.periodic)
    else if (s.type === "monotone") tangentsMonotone(p, s.periodic, s.version)
    else if (s.type === "cubic") tangentsCubic(p, s.periodic)
    return s
}

function hermite(p0, p1, h, x) {
    const t = (x - p0.x) / h, t2 = t * t, t3 = t2 * t
    return (2 * t3 - 3 * t2 + 1) * p0.y + (t3 - 2 * t2 + t) * h * p0.dy
         + (-2 * t3 + 3 * t2) * p1.y + (t3 - t2) * h * p1.dy
}

function clamp01(v) { return Math.max(0, Math.min(1, v)) }

function evaluate(s, x) {
    const p = s.points, N = p.length
    if (N === 0) return x
    if (N === 1) return clamp01(p[0].y)
    let y
    if (s.periodic) {
        x = x % 1; if (x < 0) x += 1
        if (x < p[0].x) x += 1
        let n0 = N - 1
        for (let i = 0; i < N; ++i) if (p[i].x > x) { n0 = i - 1; break }
        if (n0 < 0) n0 = N - 1
        const n1 = n0 + 1 < N ? n0 + 1 : 0
        const h = n1 > n0 ? p[n1].x - p[n0].x : p[n1].x + 1 - p[n0].x
        const b = { x: p[n0].x + h, y: p[n1].y, dy: p[n1].dy }
        y = s.type === "linear" ? p[n0].y + (b.y - p[n0].y) * (x - p[n0].x) / h : hermite(p[n0], b, h, x)
        return clamp01(y)
    }
    x = clamp01(x)
    if (x <= p[0].x || x >= p[N - 1].x) {
        const e = x <= p[0].x ? p[0] : p[N - 1]
        y = s.version === 1 || s.type === "linear" ? e.y : e.y + (x - e.x) * e.dy
        return clamp01(y)
    }
    let n0 = 0
    while (n0 < N - 2 && x >= p[n0 + 1].x) ++n0
    const h = p[n0 + 1].x - p[n0].x
    y = s.type === "linear" ? p[n0].y + (p[n0 + 1].y - p[n0].y) * (x - p[n0].x) / h
                            : hermite(p[n0], p[n0 + 1], h, x)
    return clamp01(y)
}

// darktable tonecurve's "scale for graph": log(x·base + 1) / log(base + 1); base 0 is linear.
function toLog(v, base) { return base > 0 ? Math.log(v * base + 1) / Math.log(base + 1) : v }
function toLin(v, base) { return base > 0 ? (Math.pow(base + 1, v) - 1) / base : v }
