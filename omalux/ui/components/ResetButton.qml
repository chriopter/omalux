import QtQuick
import QtQuick.Controls

// "reset parameters" of a module, in its heading (ModuleHeader) or in the strip under a kept
// main row (ModuleStrip): darktable's reset icon, an open circle with an arrow head.
ToolButton {
    id: root
    required property var theme
    property string title: ""
    implicitWidth: 22; implicitHeight: 20
    padding: 0
    hoverEnabled: true
    Accessible.name: "Reset " + root.title
    ToolTip.visible: hovered
    ToolTip.delay: 900
    ToolTip.text: "reset parameters"
    contentItem: Canvas {
        property color stroke: root.hovered ? root.theme.ink : root.theme.muted
        onStrokeChanged: requestPaint()
        onPaint: {
            const c = getContext("2d")
            c.clearRect(0, 0, width, height)
            c.strokeStyle = stroke; c.fillStyle = stroke; c.lineWidth = 1.2
            const cx = width / 2, cy = height / 2, r = 4.5
            c.beginPath(); c.arc(cx, cy, r, -Math.PI * .35, Math.PI * 1.25); c.stroke()
            const ax = cx + r * Math.cos(-Math.PI * .35), ay = cy + r * Math.sin(-Math.PI * .35)
            c.beginPath(); c.moveTo(ax + 2.6, ay + 1.2); c.lineTo(ax - 2.2, ay + 1.6); c.lineTo(ax + .6, ay - 3); c.closePath(); c.fill()
        }
    }
    background: Rectangle {
        radius: 3
        color: root.pressed ? root.theme.active : root.hovered ? root.theme.hover : "transparent"
    }
}
