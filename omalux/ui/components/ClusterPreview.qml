import QtQuick

// color mapping's cluster swatches (colormapping.c cluster_preview_draw, line 831): one block
// per cluster, a 3 × 3 grid of a, b = mean − σ … mean + σ at L 53.39, converted from Lab to
// sRGB, on a dark ground. `means` and `sigmas` are [[a, b], …], `count` the number of clusters.
Canvas {
    id: root
    required property var theme
    property var means: []
    property var sigmas: []
    property int count: 0
    implicitHeight: Math.round(width / 3)
    onMeansChanged: requestPaint()
    onSigmasChanged: requestPaint()
    onCountChanged: requestPaint()
    onWidthChanged: requestPaint()
    // Lab (D50) → XYZ → linear sRGB (Bradford-adapted D50 matrix) → sRGB, clipped.
    function labToRgb(L, a, b) {
        const fy = (L + 16) / 116, fx = fy + a / 500, fz = fy - b / 200
        const inv = t => t > 6 / 29 ? t * t * t : 3 * (6 / 29) * (6 / 29) * (t - 4 / 29)
        const X = 0.9642 * inv(fx), Y = inv(fy), Z = 0.8249 * inv(fz)
        const lin = [3.1338561 * X - 1.6168667 * Y - 0.4906146 * Z,
                     -0.9787684 * X + 1.9161415 * Y + 0.0334540 * Z,
                     0.0719453 * X - 0.2289914 * Y + 1.4052427 * Z]
        return lin.map(v => { v = Math.max(0, Math.min(1, v)); return v <= 0.0031308 ? 12.92 * v : 1.055 * Math.pow(v, 1 / 2.4) - 0.055 })
    }
    onPaint: {
        const c = getContext("2d")
        c.fillStyle = Qt.rgba(.2, .2, .2, 1)
        c.fillRect(0, 0, width, height)
        const n = Math.max(1, count), inset = 5, sep = 2
        const w = width - 2 * inset, h = height - 2 * inset
        const qwd = (w - (n - 1) * sep) / n
        for (let cl = 0; cl < count; ++cl) {
            const m = means[cl] || [0, 0], s = sigmas[cl] || [0, 0]
            const x0 = inset + cl * (qwd + sep)
            for (let j = -1; j <= 1; ++j)
                for (let i = -1; i <= 1; ++i) {
                    const rgb = labToRgb(53.390011, Number(m[0]) + i * Number(s[0]), Number(m[1]) + j * Number(s[1]))
                    c.fillStyle = Qt.rgba(rgb[0], rgb[1], rgb[2], 1)
                    c.fillRect(x0 + qwd * (i + 1) / 3, inset + h * (j + 1) / 3, qwd / 3 - .5, h / 3 - .5)
                }
        }
    }
}
