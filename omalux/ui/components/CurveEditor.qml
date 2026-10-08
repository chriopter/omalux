import QtQuick
import QtQuick.Controls
import "CurveMath.js" as CurveMath

// Node curve editor for darktable's curve modules (tone curve, rgb curve, base curve, color
// zones, rgb levels). Nodes are {x, y} in 0..1 as stored in the module parameters; the drawn
// curve uses darktable's own interpolation (CurveMath.js), including the colour zones hue
// wrap. Interaction follows darktable: drag a node (it never passes its neighbours),
// Ctrl+click adds a node on the curve, right-click a node removes it. Additionally a
// double-click on empty space adds a node there and a double-click on a node removes it.
// Right-click on empty space offers the reset. With focus, arrows move the active node
// (Shift coarse, Ctrl fine), Alt+Left/Right picks the neighbouring node, Delete removes it.
// The component never writes `nodes` itself: it reports edits through nodesEdited and keeps
// showing the dragged shape until the owner passes the new nodes back.
FocusScope {
    id: root
    required property var theme
    property var nodes: [{ x: 0, y: 0 }, { x: 1, y: 1 }]
    property string interpolation: "cubic"      // "cubic" | "catmull" | "monotone" | "linear"
    // 1 = curve_tools.c sampling (tone curve, base curve: flat beyond the end nodes),
    // 2 = splines.cpp (rgb curve, color zones: linear extrapolation, periodic support).
    property int splineVersion: 2
    property int minNodes: 2
    property int maxNodes: 20
    property bool periodic: false               // x wraps around (color zones hue axis)
    property real xLog: 0                       // darktable "scale for graph" base, 0 = linear
    property real yLog: 0
    property string background: "none"          // "none" | "gradient-luma" | "gradient-hue" | "gradient"
    property var gradientColors: []             // stops for background "gradient", left to right
    property string referenceLine: periodic ? "center" : "diagonal"   // "diagonal" | "center" | "none"
    property var histogram: []                  // optional bin counts, any length
    property bool readOnly: false               // draw the curve only, no handles
    property bool editable: true                // false while no image is loaded (dimmed)
    property color curveColor: theme.ink
    property color accentColor: theme.accent
    property string curveLabel: ""
    property real aspectRatio: 1                // plot height / width
    property real minGap: 0.005                 // smallest x distance between nodes
    property var formatX: v => (v * 100).toFixed(1)
    property var formatY: v => (v * 100).toFixed(1)
    signal nodesEdited(var nodes)
    signal interactionChanged(bool active)
    signal resetRequested()

    property int activeIndex: -1
    property int hoverIndex: -1
    property bool dragging: false
    property var _work: null
    property real _mouseX: -1
    readonly property var shownNodes: {
        const src = _work || nodes || []
        const out = src.map(n => ({ x: Number(n.x), y: Number(n.y) }))
        out.sort((a, b) => a.x - b.x)
        return out
    }
    readonly property var spline: CurveMath.prepare(shownNodes, interpolation, periodic, splineVersion)
    readonly property bool interactive: editable && !readOnly
    readonly property int inset: 6

    onNodesChanged: if (!dragging) _work = null
    implicitWidth: 260
    implicitHeight: header.height + 4 + Math.round(width * aspectRatio)

    function toPx(x) { return inset + CurveMath.toLog(x, xLog) * (plot.width - 2 * inset) }
    function toPy(y) { return inset + (1 - CurveMath.toLog(y, yLog)) * (plot.height - 2 * inset) }
    function fromPx(px) { return CurveMath.toLin(Math.max(0, Math.min(1, (px - inset) / (plot.width - 2 * inset))), xLog) }
    function fromPy(py) { return CurveMath.toLin(Math.max(0, Math.min(1, 1 - (py - inset) / (plot.height - 2 * inset))), yLog) }
    function hit(px, py) {
        let best = -1, bestD = 10 * 10
        for (let i = 0; i < shownNodes.length; ++i) {
            const dx = toPx(shownNodes[i].x) - px, dy = toPy(shownNodes[i].y) - py
            if (dx * dx + dy * dy < bestD) { bestD = dx * dx + dy * dy; best = i }
        }
        return best
    }
    function commit(list) { _work = list; root.nodesEdited(list.map(n => ({ x: n.x, y: n.y }))) }
    function moveNode(i, x, y) {
        const list = shownNodes.slice()
        const lo = i > 0 ? list[i - 1].x + minGap : 0
        const hi = i < list.length - 1 ? list[i + 1].x - minGap : 1
        list[i] = { x: Math.max(lo, Math.min(hi, x)), y: Math.max(0, Math.min(1, y)) }
        commit(list)
    }
    function addNode(x, y) {
        if (shownNodes.length >= maxNodes) return -1
        if (shownNodes.some(n => Math.abs(n.x - x) < minGap)) return -1
        const list = shownNodes.slice()
        list.push({ x: x, y: Math.max(0, Math.min(1, y)) })
        list.sort((a, b) => a.x - b.x)
        commit(list)
        return list.findIndex(n => n.x === x)
    }
    function removeNode(i) {
        if (i < 0 || shownNodes.length <= minNodes) return
        const list = shownNodes.slice()
        list.splice(i, 1)
        activeIndex = -1; hoverIndex = -1
        commit(list)
    }

    Item {
        id: header
        width: parent.width
        height: 18
        Text {
            anchors.left: parent.left
            anchors.verticalCenter: parent.verticalCenter
            width: parent.width - readout.implicitWidth - 8
            text: root.curveLabel
            color: root.theme.muted
            font: root.theme.textFont
            elide: Text.ElideRight
        }
        Text {
            id: readout
            anchors.right: parent.right
            anchors.verticalCenter: parent.verticalCenter
            readonly property int index: root.hoverIndex >= 0 ? root.hoverIndex : root.activeIndex
            readonly property bool onNode: index >= 0 && index < root.shownNodes.length
            readonly property real rx: onNode ? root.shownNodes[index].x : root._mouseX
            readonly property real ry: onNode ? root.shownNodes[index].y : CurveMath.evaluate(root.spline, rx)
            text: rx < 0 ? "" : root.formatX(rx) + " → " + root.formatY(ry)
            color: onNode ? root.theme.ink : root.theme.muted
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
        border.color: root.activeFocus ? root.theme.accent : root.theme.line
        opacity: root.editable ? 1 : 0.55
        clip: true

        Canvas {
            id: canvas
            anchors.fill: parent
            renderStrategy: Canvas.Cooperative
            property var deps: [root.spline, root.hoverIndex, root.activeIndex, root.histogram, root.background,
                                root.xLog, root.yLog, root.curveColor, root.accentColor, root.readOnly, root.referenceLine]
            onDepsChanged: requestPaint()
            onWidthChanged: requestPaint()
            onHeightChanged: requestPaint()
            function stops(grad, colors) {
                for (let i = 0; i < colors.length; ++i) grad.addColorStop(i / Math.max(1, colors.length - 1), colors[i])
            }
            onPaint: {
                const c = getContext("2d")
                const w = width, h = height, i0 = root.inset, iw = w - 2 * i0, ih = h - 2 * i0
                c.reset()
                // Background tint along x.
                let colors = []
                if (root.background === "gradient-hue")
                    colors = ["#c84a5a", "#c88a3a", "#a8a83a", "#4aa85a", "#3aa8a8", "#4a6ac8", "#8a4ac8", "#c84a5a"]
                else if (root.background === "gradient-luma")
                    colors = ["#000000", "#ffffff"]
                else if (root.background === "gradient")
                    colors = root.gradientColors
                if (colors.length > 1) {
                    const g = c.createLinearGradient(i0, 0, w - i0, 0)
                    stops(g, colors)
                    c.globalAlpha = root.background === "gradient-luma" ? 0.10 : 0.16
                    c.fillStyle = g; c.fillRect(0, 0, w, h)
                    c.globalAlpha = 0.9
                    c.fillRect(i0, h - 4, iw, 3)
                    c.globalAlpha = 1
                }
                // Grid in quarters, in display space like darktable.
                c.strokeStyle = root.theme.line; c.lineWidth = 1
                c.globalAlpha = 0.55
                c.beginPath()
                for (let k = 1; k < 4; ++k) {
                    const gx = Math.round(i0 + iw * k / 4) + 0.5, gy = Math.round(i0 + ih * k / 4) + 0.5
                    c.moveTo(gx, i0); c.lineTo(gx, h - i0); c.moveTo(i0, gy); c.lineTo(w - i0, gy)
                }
                c.stroke()
                c.globalAlpha = 1
                // Histogram.
                const hist = root.histogram || []
                if (hist.length > 1) {
                    let max = 0
                    for (let k = 0; k < hist.length; ++k) max = Math.max(max, hist[k])
                    if (max > 0) {
                        c.fillStyle = root.theme.muted; c.globalAlpha = 0.22
                        c.beginPath(); c.moveTo(i0, h - i0)
                        for (let k = 0; k < hist.length; ++k)
                            c.lineTo(i0 + iw * CurveMath.toLog(k / (hist.length - 1), root.xLog), h - i0 - ih * hist[k] / max)
                        c.lineTo(w - i0, h - i0); c.closePath(); c.fill()
                        c.globalAlpha = 1
                    }
                }
                // Reference line: identity or the neutral middle.
                if (root.referenceLine !== "none") {
                    c.strokeStyle = root.theme.muted; c.globalAlpha = 0.5; c.lineWidth = 1
                    c.setLineDash ? c.setLineDash([3, 3]) : null
                    c.beginPath()
                    if (root.referenceLine === "center") { c.moveTo(i0, h / 2); c.lineTo(w - i0, h / 2) }
                    else {
                        for (let px = 0; px <= iw; px += 4) {
                            const x = CurveMath.toLin(px / iw, root.xLog)
                            const py = root.toPy(x)
                            if (px === 0) c.moveTo(i0 + px, py); else c.lineTo(i0 + px, py)
                        }
                    }
                    c.stroke()
                    c.setLineDash ? c.setLineDash([]) : null
                    c.globalAlpha = 1
                }
                // The interpolated curve, one sample per pixel in display space.
                c.strokeStyle = root.curveColor; c.lineWidth = 2; c.lineJoin = "round"
                c.beginPath()
                for (let px = 0; px <= iw; ++px) {
                    const x = CurveMath.toLin(px / iw, root.xLog)
                    const py = root.toPy(CurveMath.evaluate(root.spline, x))
                    if (px === 0) c.moveTo(i0 + px, py); else c.lineTo(i0 + px, py)
                }
                c.stroke()
                if (root.readOnly) return
                // Nodes: hollow, hovered outlined in the accent, active filled.
                const nodes = root.shownNodes
                for (let k = 0; k < nodes.length; ++k) {
                    const nx = root.toPx(nodes[k].x), ny = root.toPy(nodes[k].y)
                    const active = k === root.activeIndex, hover = k === root.hoverIndex
                    c.beginPath(); c.arc(nx, ny, active || hover ? 5 : 4, 0, Math.PI * 2)
                    c.fillStyle = active ? root.accentColor : root.theme.well
                    c.fill()
                    c.lineWidth = 1.5
                    c.strokeStyle = active || hover ? root.accentColor : root.curveColor
                    c.stroke()
                }
            }
        }

        MouseArea {
            id: mouse
            anchors.fill: parent
            enabled: root.interactive
            hoverEnabled: true
            preventStealing: true
            acceptedButtons: Qt.LeftButton | Qt.RightButton
            cursorShape: root.hoverIndex >= 0 ? (root.dragging ? Qt.ClosedHandCursor : Qt.OpenHandCursor) : Qt.CrossCursor
            property real grabDx: 0
            property real grabDy: 0
            onExited: { if (!root.dragging) { root.hoverIndex = -1; root._mouseX = -1 } }
            onPressed: mouse => {
                root.forceActiveFocus()
                const i = root.hit(mouse.x, mouse.y)
                if (mouse.button === Qt.RightButton) {
                    if (i >= 0) root.removeNode(i)
                    else menu.popup(mouse.x, mouse.y)
                    return
                }
                if (i < 0 && (mouse.modifiers & Qt.ControlModifier)) {
                    const x = root.fromPx(mouse.x)
                    root.activeIndex = root.addNode(x, CurveMath.evaluate(root.spline, x))
                    return
                }
                root.activeIndex = i
                if (i >= 0) {
                    grabDx = root.toPx(root.shownNodes[i].x) - mouse.x
                    grabDy = root.toPy(root.shownNodes[i].y) - mouse.y
                    root.dragging = true
                    root.interactionChanged(true)
                }
            }
            onPositionChanged: mouse => {
                if (root.dragging && root.activeIndex >= 0) {
                    root.moveNode(root.activeIndex, root.fromPx(mouse.x + grabDx), root.fromPy(mouse.y + grabDy))
                    root.hoverIndex = root.activeIndex
                } else {
                    root.hoverIndex = root.hit(mouse.x, mouse.y)
                    root._mouseX = root.fromPx(mouse.x)
                }
            }
            onReleased: { if (root.dragging) { root.dragging = false; root.interactionChanged(false) } }
            onCanceled: { if (root.dragging) { root.dragging = false; root.interactionChanged(false) } }
            onDoubleClicked: mouse => {
                if (mouse.button !== Qt.LeftButton) return
                const i = root.hit(mouse.x, mouse.y)
                if (i >= 0) root.removeNode(i)
                else root.activeIndex = root.addNode(root.fromPx(mouse.x), root.fromPy(mouse.y))
            }
        }
    }

    Menu {
        id: menu
        parent: plot
        MenuItem {
            text: "Reset " + (root.curveLabel || "curve")
            onTriggered: root.resetRequested()
        }
    }

    Keys.onPressed: event => {
        if (!root.interactive) return
        const n = root.shownNodes.length
        if (n === 0) return
        const step = (event.modifiers & Qt.ShiftModifier) ? 0.05 : (event.modifiers & Qt.ControlModifier) ? 0.001 : 0.005
        if (root.activeIndex < 0 && [Qt.Key_Left, Qt.Key_Right, Qt.Key_Up, Qt.Key_Down].includes(event.key)) {
            root.activeIndex = 0; event.accepted = true; return
        }
        const i = root.activeIndex, p = i >= 0 ? root.shownNodes[i] : null
        if ((event.modifiers & Qt.AltModifier) && (event.key === Qt.Key_Left || event.key === Qt.Key_Right)) {
            root.activeIndex = (i + (event.key === Qt.Key_Right ? 1 : n - 1)) % n
        } else if (!p) {
            return
        } else if (event.key === Qt.Key_Left) root.moveNode(i, p.x - step, p.y)
        else if (event.key === Qt.Key_Right) root.moveNode(i, p.x + step, p.y)
        else if (event.key === Qt.Key_Up) root.moveNode(i, p.x, p.y + step)
        else if (event.key === Qt.Key_Down) root.moveNode(i, p.x, p.y - step)
        else if (event.key === Qt.Key_Delete || event.key === Qt.Key_Backspace) root.removeNode(i)
        else if (event.key === Qt.Key_Escape) root.activeIndex = -1
        else return
        event.accepted = true
    }
    activeFocusOnTab: root.interactive
    Accessible.role: Accessible.Graphic
    Accessible.name: (root.curveLabel || "curve") + ", " + root.shownNodes.length + " nodes"
}
