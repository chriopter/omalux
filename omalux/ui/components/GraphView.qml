import QtQuick
import QtQuick.Controls
import "CurveMath.js" as CurveMath

// Response graph over fixed x positions: tone equalizer bands, wavelet (atrous, denoise)
// levels, blur or filter previews. Read-only unless `editable` and `bars` are set; then each
// point is a vertical bar from `yZero` that can be dragged up and down (or moved with the
// arrow keys after Left/Right picks a bar). x positions never change. Values are in the
// module's displayed unit; `formatValue` formats the readout. Right-click offers the reset.
FocusScope {
    id: root
    required property var theme
    property var xs: []                          // fixed abscissas, ascending
    property var ys: []                          // one value per x
    property real yMin: 0
    property real yMax: 1
    property real yZero: Math.max(yMin, Math.min(yMax, 0))   // bar origin and emphasised grid line
    property real xMin: xs.length ? xs[0] : 0
    property real xMax: xs.length ? xs[xs.length - 1] : 1
    property bool editable: false
    property bool bars: editable                 // draw draggable bars at each x
    property var labels: []                      // per-point labels under the plot ("" skips one)
    property var axisLabels: []                  // optional [left, right] captions, e.g. ["coarse", "fine"]
    property string interpolation: "catmull"     // line through the points: CurveMath types
    property string title: ""
    property color curveColor: theme.ink
    property color accentColor: theme.accent
    property real aspectRatio: 0.5
    property real step: (yMax - yMin) / 100
    property var formatValue: v => Number(v).toFixed(2)
    signal valueEdited(int index, real value)
    signal interactionChanged(bool active)
    signal resetRequested()
    // Keyboard: the sidebar selection lends its keys to the graph on Enter (arrows edit
    // points as below); Escape gives them back (see NavTarget.focusItem).
    property alias navTarget: navTarget
    NavTarget {
        id: navTarget
        navId: "graph"
        label: root.title || "graph"
        kind: "graph"
        focusItem: root
        enabled: root.editable && root.bars
        onReset: root.resetRequested()
    }

    property int activeIndex: -1
    property int hoverIndex: -1
    property bool dragging: false
    readonly property int inset: 8
    readonly property bool hasLabels: labels.some(l => l !== "") || axisLabels.length > 0
    readonly property var normalized: xs.map((x, i) => ({ x: (x - xMin) / Math.max(1e-9, xMax - xMin),
                                                          y: (ys[i] - yMin) / Math.max(1e-9, yMax - yMin) }))
    readonly property var spline: CurveMath.prepare(normalized, interpolation, false, 2)

    implicitWidth: 260
    implicitHeight: header.height + 4 + plot.height + (hasLabels ? 18 : 0)

    function toPx(i) { return inset + normalized[i].x * (plot.width - 2 * inset) }
    function toPy(v) { return inset + (1 - (v - yMin) / (yMax - yMin)) * (plot.height - 2 * inset) }
    function fromPy(py) { return yMin + Math.max(0, Math.min(1, 1 - (py - inset) / (plot.height - 2 * inset))) * (yMax - yMin) }
    function nearest(px) {
        let best = -1, bestD = 1e9
        for (let i = 0; i < xs.length; ++i) { const d = Math.abs(toPx(i) - px); if (d < bestD) { bestD = d; best = i } }
        return best
    }
    function setValue(i, v) {
        if (i < 0 || i >= ys.length) return
        root.valueEdited(i, Math.max(yMin, Math.min(yMax, v)))
    }

    Item {
        id: header
        width: parent.width
        height: 18
        Text {
            anchors.left: parent.left
            anchors.verticalCenter: parent.verticalCenter
            width: parent.width - readout.implicitWidth - 8
            text: root.title
            color: root.theme.muted
            font: root.theme.textFont
            elide: Text.ElideRight
        }
        Text {
            id: readout
            anchors.right: parent.right
            anchors.verticalCenter: parent.verticalCenter
            readonly property int index: root.hoverIndex >= 0 ? root.hoverIndex : root.activeIndex
            text: index >= 0 && index < root.ys.length
                  ? (root.labels[index] ? root.labels[index] + "  " : "") + root.formatValue(root.ys[index]) : ""
            color: root.theme.ink
            font: root.theme.textFont
        }
    }

    Rectangle {
        id: plot
        y: header.height + 4
        width: parent.width
        height: Math.round(width * root.aspectRatio)
        radius: 4
        color: root.theme.well
        border.color: root.activeFocus || navTarget.current ? root.theme.accent : root.theme.line
        clip: true

        Canvas {
            anchors.fill: parent
            renderStrategy: Canvas.Cooperative
            property var deps: [root.spline, root.ys, root.hoverIndex, root.activeIndex, root.bars, root.curveColor, root.yZero]
            onDepsChanged: requestPaint()
            onWidthChanged: requestPaint()
            onHeightChanged: requestPaint()
            onPaint: {
                const c = getContext("2d")
                const w = width, h = height, i0 = root.inset, iw = w - 2 * i0
                c.reset()
                c.lineWidth = 1
                c.strokeStyle = root.theme.line
                c.globalAlpha = 0.55
                c.beginPath()
                for (let k = 1; k < 4; ++k) { const gy = Math.round(i0 + (h - 2 * i0) * k / 4) + 0.5; c.moveTo(i0, gy); c.lineTo(w - i0, gy) }
                for (let k = 0; k < root.xs.length; ++k) { const gx = Math.round(root.toPx(k)) + 0.5; c.moveTo(gx, i0); c.lineTo(gx, h - i0) }
                c.stroke()
                c.globalAlpha = 1
                const zy = Math.round(root.toPy(root.yZero)) + 0.5
                c.strokeStyle = root.theme.muted; c.globalAlpha = 0.6
                c.beginPath(); c.moveTo(i0, zy); c.lineTo(w - i0, zy); c.stroke()
                c.globalAlpha = 1
                if (root.xs.length < 1) return
                // Bars from the origin to each value.
                if (root.bars) {
                    for (let k = 0; k < root.ys.length; ++k) {
                        const active = k === root.activeIndex || k === root.hoverIndex
                        const bx = root.toPx(k), by = root.toPy(root.ys[k])
                        c.fillStyle = active ? root.accentColor : root.theme.muted
                        c.globalAlpha = active ? 0.45 : 0.28
                        c.fillRect(bx - 3, Math.min(by, zy), 6, Math.abs(by - zy))
                        c.globalAlpha = 1
                    }
                }
                // The response line through the points.
                c.strokeStyle = root.curveColor; c.lineWidth = 2; c.lineJoin = "round"
                c.beginPath()
                const x0 = root.toPx(0), x1 = root.toPx(root.xs.length - 1)
                for (let px = Math.floor(x0); px <= Math.ceil(x1); ++px) {
                    const u = (px - i0) / iw
                    const v = root.yMin + CurveMath.evaluate(root.spline, u) * (root.yMax - root.yMin)
                    if (px === Math.floor(x0)) c.moveTo(px, root.toPy(v)); else c.lineTo(px, root.toPy(v))
                }
                c.stroke()
                for (let k = 0; k < root.ys.length; ++k) {
                    const active = k === root.activeIndex, hover = k === root.hoverIndex
                    c.beginPath(); c.arc(root.toPx(k), root.toPy(root.ys[k]), active || hover ? 4.5 : 3.5, 0, Math.PI * 2)
                    c.fillStyle = active ? root.accentColor : root.theme.well; c.fill()
                    c.lineWidth = 1.5; c.strokeStyle = active || hover ? root.accentColor : root.curveColor; c.stroke()
                }
            }
        }

        MouseArea {
            anchors.fill: parent
            enabled: root.editable && root.bars
            hoverEnabled: true
            preventStealing: true
            acceptedButtons: Qt.LeftButton | Qt.RightButton
            cursorShape: Qt.SizeVerCursor
            onExited: if (!root.dragging) root.hoverIndex = -1
            onPressed: mouse => {
                root.forceActiveFocus()
                if (mouse.button === Qt.RightButton) { menu.popup(mouse.x, mouse.y); return }
                root.activeIndex = root.nearest(mouse.x)
                root.dragging = true
                root.interactionChanged(true)
                root.setValue(root.activeIndex, root.fromPy(mouse.y))
            }
            onPositionChanged: mouse => {
                if (root.dragging) root.setValue(root.activeIndex, root.fromPy(mouse.y))
                else root.hoverIndex = root.nearest(mouse.x)
            }
            onReleased: { if (root.dragging) { root.dragging = false; root.interactionChanged(false) } }
            onCanceled: { if (root.dragging) { root.dragging = false; root.interactionChanged(false) } }
        }
    }

    Item {
        visible: root.hasLabels
        y: plot.y + plot.height + 2
        width: parent.width
        height: 16
        Repeater {
            model: root.labels
            Text {
                required property string modelData
                required property int index
                visible: modelData !== "" && index < root.xs.length
                x: Math.max(0, Math.min(parent.width - width, root.toPx(index) - width / 2))
                text: modelData
                color: index === root.activeIndex ? root.theme.ink : root.theme.muted
                font: root.theme.textFont
            }
        }
        Text {
            visible: root.axisLabels.length > 0
            anchors.left: parent.left
            text: root.axisLabels[0] || ""
            color: root.theme.muted; font: root.theme.textFont
        }
        Text {
            visible: root.axisLabels.length > 1
            anchors.right: parent.right
            text: root.axisLabels[1] || ""
            color: root.theme.muted; font: root.theme.textFont
        }
    }

    Menu {
        id: menu
        parent: plot
        MenuItem { text: "Reset " + (root.title || "graph"); onTriggered: root.resetRequested() }
    }

    Keys.onPressed: event => {
        if (!root.editable || !root.bars || root.ys.length === 0) return
        const n = root.ys.length
        const step = root.step * ((event.modifiers & Qt.ShiftModifier) ? 10 : (event.modifiers & Qt.ControlModifier) ? 0.1 : 1)
        if (event.key === Qt.Key_Left || event.key === Qt.Key_Right)
            root.activeIndex = root.activeIndex < 0 ? 0 : (root.activeIndex + (event.key === Qt.Key_Right ? 1 : n - 1)) % n
        else if (root.activeIndex >= 0 && event.key === Qt.Key_Up) root.setValue(root.activeIndex, root.ys[root.activeIndex] + step)
        else if (root.activeIndex >= 0 && event.key === Qt.Key_Down) root.setValue(root.activeIndex, root.ys[root.activeIndex] - step)
        else return
        event.accepted = true
    }
    activeFocusOnTab: root.editable && root.bars
    Accessible.role: Accessible.Graphic
    Accessible.name: root.title || "graph"
}
