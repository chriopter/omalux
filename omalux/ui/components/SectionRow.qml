import QtQuick
import QtQuick.Controls

// The heading of a sub-section inside an open module ("more", "blending", "multiple
// instances"): a hairline, the label, what the closed section holds (`summary`, in the value
// column of the slider rows) and the chevron in the disclosure column, as on the module rows.
// The whole row is the click target. `menu` sections open a menu instead of rows and show a
// menu mark (three dots) in place of the chevron.
AbstractButton {
    id: root
    required property var theme
    property string label: ""
    property string summary: ""
    property bool open: false
    property bool menu: false
    // Keyboard stop (see NavTarget): Enter toggles, ←/→ close and open.
    property alias navTarget: nav
    signal requested()

    implicitHeight: 30
    padding: 0
    hoverEnabled: true
    onClicked: { nav.claim(); root.requested() }
    Accessible.name: root.label
    Accessible.description: root.menu ? "Open menu" : root.open ? "Collapse section" : "Expand section"
    NavTarget {
        id: nav
        label: root.label
        kind: "button"
        enabled: root.enabled
        adjustLabel: root.menu ? "" : "COLLAPSE/EXPAND"
        activateLabel: root.menu ? "OPEN MENU" : root.open ? "COLLAPSE" : "EXPAND"
        onActivate: root.requested()
        onAdjust: steps => { if (!root.menu && (steps > 0) !== root.open) root.requested() }
    }
    readonly property bool marked: nav.current || root.visualFocus
    background: Item {
        // Separates the section from the rows above it, across the module block.
        Rectangle { x: -8; y: 3; width: parent.width + 8; height: 1; color: root.theme.line; opacity: .55 }
        Rectangle {
            x: -8; y: 6; width: parent.width + 8; height: parent.height - 6
            radius: 4
            color: root.pressed ? root.theme.active : root.hovered ? root.theme.hover : "transparent"
            border.width: root.marked ? 1 : 0
            border.color: root.theme.accent
        }
    }
    contentItem: Item {
        Text {
            id: caption
            anchors.left: parent.left
            y: 6 + (parent.height - 6 - height) / 2
            text: root.label
            color: root.marked ? root.theme.accent : root.open || root.hovered ? root.theme.ink : root.theme.muted
            font: root.theme.settingsFont
        }
        Text {
            anchors.left: caption.right; anchors.leftMargin: 12
            anchors.right: parent.right; anchors.rightMargin: 28
            anchors.verticalCenter: caption.verticalCenter
            visible: !root.open && root.summary !== ""
            text: root.summary
            horizontalAlignment: Text.AlignRight
            elide: Text.ElideRight
            color: root.theme.muted
            font: root.theme.textFont
        }
        Canvas {
            id: chevron
            x: parent.width - 24 + 7
            anchors.verticalCenter: caption.verticalCenter
            width: 10; height: 10
            rotation: !root.menu && root.open ? 90 : 0
            Behavior on rotation { NumberAnimation { duration: 90 } }
            property color stroke: root.marked ? root.theme.accent : root.open || root.hovered ? root.theme.ink : root.theme.muted
            onStrokeChanged: requestPaint()
            onPaint: {
                const c = getContext("2d")
                c.clearRect(0, 0, width, height)
                c.strokeStyle = stroke; c.fillStyle = stroke; c.lineWidth = 1.4
                if (root.menu) {
                    // A menu, not rows: three dots.
                    for (const x of [1.5, 5, 8.5]) { c.beginPath(); c.arc(x, 5, 1, 0, 2 * Math.PI); c.fill() }
                    return
                }
                c.beginPath(); c.moveTo(3, 1); c.lineTo(7, 5); c.lineTo(3, 9); c.stroke()
            }
        }
    }
}
