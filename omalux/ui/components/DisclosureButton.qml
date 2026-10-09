import QtQuick
import QtQuick.Controls

// The chevron in the right-hand disclosure column: › collapsed, ⌄ expanded.
ToolButton {
    id: root
    required property var theme
    property bool expanded: false
    // The pointer is over the heading this chevron belongs to: it answers as if hovered.
    property bool hot: false
    width: 24; height: 22; padding: 0
    hoverEnabled: true
    contentItem: Item {
        Canvas {
            id: chevron
            anchors.centerIn: parent
            width: 10; height: 10
            rotation: root.expanded ? 90 : 0
            Behavior on rotation { NumberAnimation { duration: 90 } }
            property color stroke: root.hovered || root.visualFocus ? root.theme.accent
                                 : root.hot ? root.theme.ink
                                 : root.expanded ? root.theme.ink : root.theme.muted
            onStrokeChanged: requestPaint()
            onPaint: {
                const c = getContext("2d")
                c.clearRect(0, 0, width, height)
                c.strokeStyle = stroke; c.lineWidth = 1.4
                c.beginPath(); c.moveTo(3, 1); c.lineTo(7, 5); c.lineTo(3, 9); c.stroke()
            }
        }
    }
    background: Rectangle {
        radius: 4
        color: root.pressed ? root.theme.active : root.hovered || root.hot ? root.theme.hover : "transparent"
        border.width: root.visualFocus ? 1 : 0
        border.color: root.theme.accent
    }
}
