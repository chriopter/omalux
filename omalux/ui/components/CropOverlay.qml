import QtQuick
import "CropMath.js" as Crop

// The crop frame on the photo, with darktable's pointer handling (src/iop/crop.c
// _gui_get_grab 1467, button_pressed 1872, mouse_moved 1672, _aspect_apply 771):
//   a band of 30 px inside each edge resizes (the corners both edges); while the frame is the
//   whole photo the bands reach 45 % in, so a drag from anywhere near a side starts a crop
//   Shift while resizing keeps the centre and the proportions
//   inside the bands the frame moves; Shift only vertically, Ctrl only horizontally
//   a right-click resets the frame to the whole photo
// The modifiers are read at the press, as darktable does. `crop` is in fractions of the photo,
// `aspectRatio` the wanted width over height in photo pixels (0: freehand).
Item {
    id: overlay
    required property var crop
    property real aspectRatio: 0
    signal cropChangedByUser(var rect)

    // Whole pixels: fractional edges leave a seam between the dimmed areas.
    readonly property real frameLeft: Math.round(crop.x * width)
    readonly property real frameTop: Math.round(crop.y * height)
    readonly property real frameWidth: Math.round((crop.x + crop.width) * width) - frameLeft
    readonly property real frameHeight: Math.round((crop.y + crop.height) * height) - frameTop
    readonly property real minimum: Crop.MINIMUM
    readonly property real band: 30
    // _grab_region_t as bits; 0 inside (move), -1 outside the frame.
    property int hovered: -1
    property int grabbed: -1
    property bool pressedInside: false
    property var previous: null
    property point down
    property point handle
    property bool shiftHold: false
    property bool ctrlHold: false

    function isFull(c) { return !(c.x || c.y || c.width !== 1 || c.height !== 1) }
    // _gui_get_grab: which part of the frame is at a point (fractions of the photo).
    function grabAt(px, py) {
        const c = crop
        if (px < c.x || px > c.x + c.width || py < c.y || py > c.y + c.height) return -1
        let hb = band / Math.max(1, width), vb = band / Math.max(1, height)
        if (isFull(c)) hb = vb = 0.45
        let grab = 0
        if (px >= c.x && px < c.x + hb && px - c.x < 0.5 * c.width) grab |= Crop.LEFT
        else if (px <= c.x + c.width && px > c.x + c.width - hb) grab |= Crop.RIGHT
        if (py >= c.y && py < c.y + vb && py - c.y < 0.5 * c.height) grab |= Crop.TOP
        else if (py <= c.y + c.height && py > c.y + c.height - vb) grab |= Crop.BOTTOM
        return grab
    }
    function aspectApply(c, grab) { return Crop.aspectApply(c, grab, aspectRatio, width, height) }
    // button_pressed: px, py in fractions.
    function press(px, py, rightButton, modifiers) {
        if (rightButton) {
            cropChangedByUser(aspectApply({ x: 0, y: 0, width: 1, height: 1 }, Crop.BOTTOM | Crop.RIGHT))
            return
        }
        const c = crop
        down = Qt.point(px, py)
        previous = { x: c.x, y: c.y, width: c.width, height: c.height }
        shiftHold = !!(modifiers & Qt.ShiftModifier)
        ctrlHold = !!(modifiers & Qt.ControlModifier)
        grabbed = grabAt(px, py)
        pressedInside = grabbed === 0
        if (grabbed === 0) handle = Qt.point(c.x, c.y)
        else if (grabbed > 0) {
            let hx = 0, hy = 0
            if (grabbed & Crop.LEFT) hx = px - c.x
            if (grabbed & Crop.TOP) hy = py - c.y
            if (grabbed & Crop.RIGHT) hx = px - (c.width + c.x)
            if (grabbed & Crop.BOTTOM) hy = py - (c.height + c.y)
            handle = Qt.point(hx, hy)
        }
    }
    // mouse_moved with the primary button down.
    function moveTo(px, py) {
        if (grabbed < 0 || !previous) return
        let x = crop.x, y = crop.y, w = crop.width, h = crop.height
        if (pressedInside) {
            if (!shiftHold) x = Math.min(1 - w, Math.max(0, handle.x + px - down.x))
            if (!ctrlHold) y = Math.min(1 - h, Math.max(0, handle.y + py - down.y))
            cropChangedByUser({ x: x, y: y, width: w, height: h })
            return
        }
        const p = previous
        if (shiftHold) {
            let ratio = 0
            if (grabbed & (Crop.LEFT | Crop.RIGHT)) {
                const xx = (grabbed & Crop.LEFT) ? (px - down.x) : (down.x - px)
                ratio = (p.width - 2 * xx) / p.width
            }
            if (grabbed & (Crop.TOP | Crop.BOTTOM)) {
                const yy = (grabbed & Crop.TOP) ? (py - down.y) : (down.y - py)
                ratio = Math.max(ratio, (p.height - 2 * yy) / p.height)
            }
            if (p.width * ratio < minimum) ratio = minimum / p.width
            if (p.height * ratio < minimum) ratio = minimum / p.height
            if (p.width * ratio > 1) ratio = 1 / p.width
            if (p.height * ratio > 1) ratio = 1 / p.height
            w = p.width * ratio; h = p.height * ratio
            x = Math.min(Math.max(p.x - (w - p.width) / 2, 0), 1 - w)
            y = Math.min(Math.max(p.y - (h - p.height) / 2, 0), 1 - h)
        } else {
            if (grabbed & Crop.LEFT) {
                const old = x
                x = Math.min(Math.max(0, px - handle.x), x + w - minimum)
                w = old + w - x
            }
            if (grabbed & Crop.TOP) {
                const old = y
                y = Math.min(Math.max(0, py - handle.y), y + h - minimum)
                h = old + h - y
            }
            if (grabbed & Crop.RIGHT) w = Math.max(minimum, Math.min(1, px - x - handle.x))
            if (grabbed & Crop.BOTTOM) h = Math.max(minimum, Math.min(1, py - y - handle.y))
        }
        if (x + w > 1) w = 1 - x
        if (y + h > 1) h = 1 - y
        cropChangedByUser(aspectApply({ x: x, y: y, width: w, height: h }, grabbed))
    }
    function release() { grabbed = -1; pressedInside = false; previous = null; shiftHold = false; ctrlHold = false }
    function cursorFor(grab) {
        if (grab < 0) return Qt.ArrowCursor
        if (grab === 0) return Qt.SizeAllCursor
        if (grab === Crop.LEFT || grab === Crop.RIGHT) return Qt.SizeHorCursor
        if (grab === Crop.TOP || grab === Crop.BOTTOM) return Qt.SizeVerCursor
        return grab === (Crop.TOP | Crop.LEFT) || grab === (Crop.BOTTOM | Crop.RIGHT) ? Qt.SizeFDiagCursor : Qt.SizeBDiagCursor
    }
    readonly property int shown: grabbed >= 0 && !pressedInside ? grabbed : pressedInside ? 0 : hovered

    Rectangle { width: parent.width; height: overlay.frameTop; color: "#99000000" }
    Rectangle { y: overlay.frameTop+overlay.frameHeight; width: parent.width; height: parent.height-y; color: "#99000000" }
    Rectangle { y: overlay.frameTop; width: overlay.frameLeft; height: overlay.frameHeight; color: "#99000000" }
    Rectangle { x: overlay.frameLeft+overlay.frameWidth; y: overlay.frameTop; width: parent.width-x; height: overlay.frameHeight; color: "#99000000" }
    Item {
        id: frame
        objectName: "crop-frame"
        x: overlay.frameLeft; y: overlay.frameTop
        width: overlay.frameWidth; height: overlay.frameHeight
        // Thirds, a dark line beside each light one so they read on clouds and on shadows.
        Repeater {
            model: 2
            Item {
                required property int index
                x: Math.round((index+1)*frame.width/3); width: 2; height: frame.height
                Rectangle { width: 1; height: parent.height; color: "#b0ffffff" }
                Rectangle { x: 1; width: 1; height: parent.height; color: "#70000000" }
            }
        }
        Repeater {
            model: 2
            Item {
                required property int index
                y: Math.round((index+1)*frame.height/3); height: 2; width: frame.width
                Rectangle { height: 1; width: parent.width; color: "#b0ffffff" }
                Rectangle { y: 1; height: 1; width: parent.width; color: "#70000000" }
            }
        }
        Rectangle { anchors.fill: parent; anchors.margins: -1; color: "transparent"; border.color: "#90000000"; border.width: 1 }
        Rectangle { anchors.fill: parent; color: "transparent"; border.color: "white"; border.width: 1 }
        Rectangle { anchors.fill: parent; anchors.margins: 1; color: "transparent"; border.color: "#70000000"; border.width: 1 }
    }
    // Marks on the corners and edges; the one under the pointer or being dragged is larger.
    Repeater {
        model: [ [3,0,0], [2,0.5,0], [6,1,0], [4,1,0.5], [12,1,1], [8,0.5,1], [9,0,1], [1,0,0.5] ]
        Rectangle {
            required property var modelData
            readonly property bool hot: overlay.shown === modelData[0]
            objectName: "crop-handle-" + modelData[0]
            width: (modelData[1] === 0.5 ? 24 : 9) + (hot ? 4 : 0)
            height: (modelData[2] === 0.5 ? 24 : 9) + (hot ? 4 : 0)
            x: overlay.frameLeft + modelData[1]*overlay.frameWidth - width/2
            y: overlay.frameTop + modelData[2]*overlay.frameHeight - height/2
            radius: 2; color: "white"; border.color: "#c0000000"; border.width: 1
        }
    }
    MouseArea {
        id: area
        objectName: "crop-area"
        anchors.fill: parent
        hoverEnabled: true
        preventStealing: true
        acceptedButtons: Qt.LeftButton | Qt.RightButton
        cursorShape: pressed && overlay.pressedInside ? Qt.ClosedHandCursor : overlay.cursorFor(overlay.shown)
        onPressed: mouse => overlay.press(mouse.x / width, mouse.y / height, mouse.button === Qt.RightButton, mouse.modifiers)
        onPositionChanged: mouse => {
            if (pressed && (mouse.buttons & Qt.LeftButton)) overlay.moveTo(mouse.x / width, mouse.y / height)
            else overlay.hovered = overlay.grabAt(mouse.x / width, mouse.y / height)
        }
        onReleased: mouse => { overlay.release(); overlay.hovered = overlay.grabAt(mouse.x / width, mouse.y / height) }
        onCanceled: overlay.release()
        onExited: if (!pressed) overlay.hovered = -1
    }
}
