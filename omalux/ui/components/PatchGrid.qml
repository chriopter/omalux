import QtQuick
import QtQuick.Controls

// The colour checker patches of "color look up table" as darktable draws them (checker_draw,
// colorchecker.c:1293-1379): 6 × 4 cells for up to 24 patches, 7 × 7 beyond, each filled with
// its source colour; a light frame marks patches whose target differs, an outline the selected
// one. Click selects a patch, double-click resets it to its source, right-click removes it
// (checker_button_press :1421-1479). The grid only reports; the host edits.
Item {
    id: root
    required property var theme
    property int count: 0
    property var colors: []        // [[r, g, b]] display sRGB per patch
    property var changed: []       // [bool] per patch
    property var lightness: []     // source L per patch
    property int current: 0
    property bool editable: true
    signal patchSelected(int index)
    signal patchReset(int index)
    signal patchRemoved(int index)
    property alias navTarget: navTarget

    readonly property int cellsX: count > 24 ? 7 : 6
    readonly property int cellsY: count > 24 ? 7 : 4
    implicitWidth: 260
    implicitHeight: Math.round(width * (count > 24 ? 1 : 2 / 3))

    // Keyboard: ←/→ move the selected patch, R resets it.
    NavTarget {
        id: navTarget
        navId: "patches"
        label: "patch"
        kind: "choice"
        enabled: root.count > 0
        adjustLabel: "PATCH"
        onAdjust: steps => root.patchSelected(Math.max(0, Math.min(root.count - 1, root.current + Math.sign(steps))))
        onReset: if (root.editable) root.patchReset(root.current)
    }

    Rectangle {
        anchors.fill: parent
        color: Qt.rgba(.2, .2, .2, 1)
        radius: 2
    }
    Repeater {
        model: root.count
        Item {
            id: cell
            required property int index
            readonly property var rgb: root.colors[index] || [0.5, 0.5, 0.5]
            x: Math.round(root.width * (index % root.cellsX) / root.cellsX)
            y: Math.round(root.height * Math.floor(index / root.cellsX) / root.cellsY)
            width: Math.round(root.width / root.cellsX) - 1
            height: Math.round(root.height / root.cellsY) - 1
            Rectangle {
                anchors.fill: parent
                color: Qt.rgba(Math.max(0, Math.min(1, cell.rgb[0])), Math.max(0, Math.min(1, cell.rgb[1])),
                               Math.max(0, Math.min(1, cell.rgb[2])), 1)
            }
            Rectangle {
                visible: !!root.changed[cell.index]
                anchors.fill: parent
                anchors.margins: 1
                color: "transparent"
                border.width: 2
                border.color: Qt.rgba(.8, .8, .8, 1)
                Rectangle {
                    anchors.fill: parent
                    anchors.margins: 1
                    color: "transparent"
                    border.width: 1
                    border.color: Qt.rgba(.2, .2, .2, 1)
                }
            }
            Rectangle {
                visible: cell.index === root.current
                anchors.fill: parent
                anchors.margins: 5
                color: "transparent"
                border.width: 2
                // darktable outlines light patches in black (source L > 80).
                border.color: (root.lightness[cell.index] || 0) > 80 ? "black" : "white"
            }
        }
    }
    Rectangle {
        anchors.fill: parent
        color: "transparent"
        radius: 2
        border.color: navTarget.current ? root.theme.accent : "transparent"
    }
    function patchAt(x, y) {
        const i = Math.floor(Math.max(0, Math.min(root.width - 1, x)) * cellsX / root.width)
                + cellsX * Math.floor(Math.max(0, Math.min(root.height - 1, y)) * cellsY / root.height)
        return i < root.count ? i : -1
    }
    MouseArea {
        anchors.fill: parent
        acceptedButtons: Qt.LeftButton | Qt.RightButton
        enabled: root.editable
        onClicked: mouse => {
            navTarget.claim()
            const i = root.patchAt(mouse.x, mouse.y)
            if (i < 0) return
            if (mouse.button === Qt.RightButton) root.patchRemoved(i)
            else root.patchSelected(i)
        }
        onDoubleClicked: mouse => {
            const i = root.patchAt(mouse.x, mouse.y)
            if (i >= 0 && mouse.button === Qt.LeftButton) root.patchReset(i)
        }
    }
}
