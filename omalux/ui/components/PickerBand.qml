import QtQuick

// The values an active picker marks without writing them, as darktable draws them: a band from
// min to max and a line at the mean over the axis's colour gradient. relight's "center"
// gradient slider shows the picked lightness (dtgtk_gradient_slider_set_picker_meanminmax,
// relight.c:224), color equalizer's graph the picked hue (_draw_color_picker, colorequal.c:2257).
// `band` is { axis: "lightness" | "colorequal_hue", min, mean, max } in 0..1 of the axis; a
// max below min wraps around (a hue band across the axis end).
Canvas {
    id: root
    required property var theme
    property var band: null
    visible: !!band
    implicitHeight: 6
    onBandChanged: requestPaint()
    onWidthChanged: requestPaint()
    function hueColor(x) {
        // colorequal's axis starts 20° below red (ANGLE_SHIFT, colorequal.c:437).
        return Qt.hsla(((x + 20 / 360) % 1 + 1) % 1, .65, .5, 1)
    }
    onPaint: {
        const c = getContext("2d")
        c.clearRect(0, 0, width, height)
        const b = band
        if (!b) return
        const g = c.createLinearGradient(0, 0, width, 0)
        if (b.axis === "colorequal_hue") for (let i = 0; i <= 12; ++i) g.addColorStop(i / 12, hueColor(i / 12))
        else { g.addColorStop(0, "#000000"); g.addColorStop(1, "#808080") }
        c.fillStyle = g
        c.fillRect(0, 1, width, height - 2)
        c.fillStyle = Qt.rgba(1, 1, 1, .3)
        const x0 = b.min * width, x1 = b.max * width
        if (x1 >= x0) c.fillRect(x0, 0, Math.max(1, x1 - x0), height)
        else { c.fillRect(0, 0, x1, height); c.fillRect(x0, 0, width - x0, height) }
        c.strokeStyle = Qt.rgba(1, 1, 1, .9)
        c.lineWidth = 1
        const x = Math.round(b.mean * width) + .5
        c.beginPath(); c.moveTo(x, 0); c.lineTo(x, height); c.stroke()
    }
}
