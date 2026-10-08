import QtQuick

// The active colour picker's area or point on the photo, as darktable draws it in the center
// view. Area: a drag anywhere draws a new area, a drag at a corner resizes it (darkroom.c
// button_pressed; the default area covers almost the whole photo, so moving it is not offered). Point: the cross follows the pointer. Point-or-area (darktable's POINT_AREA pickers): a
// click picks a point, a drag draws an area. The box is normalised to the displayed image and
// reported with the keyboard modifiers once the pointer is released (the module applies the
// picker then; Ctrl/Shift select darktable's positive/negative "create curve").
Item {
    id: root
    property string kind: "area"               // area · point · pointarea
    property var box: [0.02, 0.02, 0.98, 0.98]  // x0, y0, x1, y1
    signal boxEdited(var box, int modifiers)

    property var draft: null
    readonly property var shown: draft || box
    readonly property real boxLeft: Math.min(shown[0], shown[2]) * width
    readonly property real boxTop: Math.min(shown[1], shown[3]) * height
    readonly property real boxRight: Math.max(shown[0], shown[2]) * width
    readonly property real boxBottom: Math.max(shown[1], shown[3]) * height
    // A box of (nearly) no size is a point.
    readonly property bool showsArea: kind === "area" || (kind === "pointarea" && boxRight - boxLeft >= 2 && boxBottom - boxTop >= 2)

    function clamp(v) { return Math.max(0, Math.min(1, v)) }

    // Area box: a dark and a light outline so it reads on any photo.
    Rectangle {
        visible: root.showsArea
        x: root.boxLeft - 1; y: root.boxTop - 1
        width: root.boxRight - root.boxLeft + 2; height: root.boxBottom - root.boxTop + 2
        color: "transparent"; border.color: "#b0000000"; border.width: 3
    }
    Rectangle {
        visible: root.showsArea
        x: root.boxLeft; y: root.boxTop
        width: root.boxRight - root.boxLeft; height: root.boxBottom - root.boxTop
        color: "transparent"; border.color: "white"; border.width: 1
    }
    Repeater {
        model: root.showsArea && root.kind === "area" ? 4 : 0
        Rectangle {
            required property int index
            width: 7; height: 7
            x: (index % 2 ? root.boxRight : root.boxLeft) - 3.5
            y: (index > 1 ? root.boxBottom : root.boxTop) - 3.5
            color: "white"; border.color: "#b0000000"
        }
    }
    // Point: a cross with a gap at the sampled pixel.
    Item {
        visible: !root.showsArea
        x: root.boxLeft; y: root.boxTop
        Repeater {
            model: [[-12, -1, 8, 2], [4, -1, 8, 2], [-1, -12, 2, 8], [-1, 4, 2, 8]]
            Rectangle {
                required property var modelData
                x: modelData[0]; y: modelData[1]; width: modelData[2]; height: modelData[3]
                color: "white"; border.color: "#b0000000"; border.width: 0.5
            }
        }
    }
    MouseArea {
        id: area
        objectName: "picker-overlay"
        anchors.fill: parent
        preventStealing: true
        cursorShape: Qt.CrossCursor
        property string mode: "new"
        property point start
        property var startBox
        onPressed: mouse => {
            start = Qt.point(mouse.x, mouse.y)
            startBox = root.shown.slice()
            const px = root.clamp(mouse.x / width), py = root.clamp(mouse.y / height)
            if (root.kind !== "area") { mode = root.kind === "point" ? "point" : "new"; root.draft = [px, py, px, py]; return }
            const near = (x, y) => Math.abs(mouse.x - x) < 10 && Math.abs(mouse.y - y) < 10
            if (near(root.boxLeft, root.boxTop)) mode = "nw"
            else if (near(root.boxRight, root.boxTop)) mode = "ne"
            else if (near(root.boxLeft, root.boxBottom)) mode = "sw"
            else if (near(root.boxRight, root.boxBottom)) mode = "se"
            else mode = "new"
        }
        onPositionChanged: mouse => {
            if (!pressed) return
            const x = root.clamp(mouse.x / width), y = root.clamp(mouse.y / height)
            const b = [Math.min(startBox[0], startBox[2]), Math.min(startBox[1], startBox[3]),
                       Math.max(startBox[0], startBox[2]), Math.max(startBox[1], startBox[3])]
            if (mode === "point") root.draft = [x, y, x, y]
            else if (mode === "new") root.draft = [root.clamp(start.x / width), root.clamp(start.y / height), x, y]
            else {
                root.draft = [mode.indexOf("w") >= 0 ? x : b[0], mode.indexOf("n") >= 0 ? y : b[1],
                              mode.indexOf("e") >= 0 ? x : b[2], mode.indexOf("s") >= 0 ? y : b[3]]
            }
        }
        onReleased: mouse => {
            if (!root.draft) return
            const d = root.draft
            let out = [Math.min(d[0], d[2]), Math.min(d[1], d[3]), Math.max(d[0], d[2]), Math.max(d[1], d[3])]
            const tiny = (out[2] - out[0]) * width < 3 && (out[3] - out[1]) * height < 3
            // A click: a point for point pickers, a small area around the pointer for area ones.
            if (tiny && root.kind !== "area") { const cx = (out[0] + out[2]) / 2, cy = (out[1] + out[3]) / 2; out = [cx, cy, cx, cy] }
            else if (tiny) out = [root.clamp(out[0] - .02), root.clamp(out[1] - .02), root.clamp(out[2] + .02), root.clamp(out[3] + .02)]
            root.draft = null
            root.boxEdited(out, mouse.modifiers)
        }
    }
}
