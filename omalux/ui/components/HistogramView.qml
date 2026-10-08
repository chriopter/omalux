import QtQuick

// The module input histogram darktable draws behind levels and rgb levels (iop/levels.c and
// iop/rgblevels.c, *_area_draw): 256 bins per channel, logarithmic like darktable's default
// histogram scale, with the black, gray and white handles as vertical lines. Display only;
// the handles are edited by the sliders below it.
Item {
    id: root
    required property var theme
    property var histogram: null          // { channels: [[256 counts] × 4], max: [4] }
    property var channels: [0]            // which channels to draw
    property var markers: []              // handle positions 0…1
    property bool linear: false
    readonly property var channelColors: ["#e05555", "#5ac06a", "#5a8ad0", "#7f849c"]

    implicitHeight: 64
    onHistogramChanged: canvas.requestPaint()
    onChannelsChanged: canvas.requestPaint()
    onMarkersChanged: canvas.requestPaint()
    Rectangle {
        anchors.fill: parent
        color: root.theme.well
        border.color: root.theme.line
        radius: 3
    }
    Canvas {
        id: canvas
        anchors.fill: parent
        anchors.margins: 1
        onPaint: {
            const c = getContext("2d")
            c.clearRect(0, 0, width, height)
            const h = root.histogram
            if (h && h.channels) {
                let max = 0
                for (const ch of root.channels) max = Math.max(max, Number(h.max[ch]) || 0)
                const scale = v => root.linear ? v / max : Math.log(1 + v) / Math.log(1 + max)
                for (const ch of root.channels) {
                    const bins = h.channels[ch] || []
                    if (!bins.length || max <= 0) continue
                    c.beginPath()
                    c.moveTo(0, height)
                    for (let i = 0; i < bins.length; ++i)
                        c.lineTo(i * width / (bins.length - 1), height - scale(Number(bins[i])) * (height - 2))
                    c.lineTo(width, height)
                    c.closePath()
                    c.fillStyle = root.channels.length > 1 ? root.channelColors[ch] : root.theme.muted
                    c.globalAlpha = root.channels.length > 1 ? 0.35 : 0.5
                    c.fill()
                }
                c.globalAlpha = 1
            }
            c.strokeStyle = root.theme.ink
            c.lineWidth = 1
            for (const m of root.markers) {
                const x = Math.round(Math.max(0, Math.min(1, m)) * (width - 1)) + .5
                c.beginPath(); c.moveTo(x, 0); c.lineTo(x, height); c.stroke()
            }
        }
    }
}
