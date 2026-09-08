import QtQuick

Item {
    id: overlay
    required property var crop
    property real aspectRatio: 0
    signal cropChangedByUser(var rect)
    readonly property real frameLeft: crop.x * width
    readonly property real frameTop: crop.y * height
    readonly property real frameWidth: crop.width * width
    readonly property real frameHeight: crop.height * height
    property var startCrop
    property point startPoint
    property string handle: "move"

    function begin(handleName, x, y) {
        handle = handleName
        startCrop = { x: crop.x, y: crop.y, width: crop.width, height: crop.height }
        startPoint = Qt.point(x, y)
    }
    function dragTo(x, y) {
        if (!startCrop || width <= 0 || height <= 0) return
        const dx = (x - startPoint.x) / width
        const dy = (y - startPoint.y) / height
        const r = startCrop
        if (handle === "move") {
            cropChangedByUser({ x: Math.max(0, Math.min(1-r.width, r.x+dx)),
                y: Math.max(0, Math.min(1-r.height, r.y+dy)), width: r.width, height: r.height })
            return
        }
        const minW = Math.min(0.05, 24/width), minH = Math.min(0.05, 24/height)
        let l = r.x, t = r.y, rr = r.x+r.width, b = r.y+r.height
        const west = handle.includes("w"), east = handle.includes("e")
        const north = handle.includes("n"), south = handle.includes("s")
        if (west) l = Math.max(0, Math.min(rr-minW, l+dx))
        if (east) rr = Math.min(1, Math.max(l+minW, rr+dx))
        if (north) t = Math.max(0, Math.min(b-minH, t+dy))
        if (south) b = Math.min(1, Math.max(t+minH, b+dy))
        if (aspectRatio > 0) {
            const ratio = aspectRatio * height/width
            const cx = r.x+r.width/2, cy = r.y+r.height/2
            let w = rr-l, h = b-t
            if (west || east) h = w/ratio
            else w = h*ratio
            const maxW = west ? r.x+r.width : east ? 1-r.x : 2*Math.min(cx, 1-cx)
            const maxH = north ? r.y+r.height : south ? 1-r.y : 2*Math.min(cy, 1-cy)
            w = Math.min(w, maxW, maxH*ratio)
            h = w/ratio
            l = west ? r.x+r.width-w : east ? r.x : cx-w/2
            t = north ? r.y+r.height-h : south ? r.y : cy-h/2
            rr = l+w; b = t+h
        }
        cropChangedByUser({x:l, y:t, width:rr-l, height:b-t})
    }

    Rectangle { width: parent.width; height: overlay.frameTop; color: "#99000000" }
    Rectangle { y: overlay.frameTop+overlay.frameHeight; width: parent.width; height: parent.height-y; color: "#99000000" }
    Rectangle { y: overlay.frameTop; width: overlay.frameLeft; height: overlay.frameHeight; color: "#99000000" }
    Rectangle { x: overlay.frameLeft+overlay.frameWidth; y: overlay.frameTop; width: parent.width-x; height: overlay.frameHeight; color: "#99000000" }
    Rectangle {
        x: overlay.frameLeft; y: overlay.frameTop
        width: overlay.frameWidth; height: overlay.frameHeight
        color: "transparent"; border.color: "white"; border.width: 1
        Repeater {
            model: 2
            Rectangle { required property int index; x: (index+1)*parent.width/3; width: 1; height: parent.height; color: "#90ffffff" }
        }
        Repeater {
            model: 2
            Rectangle { required property int index; y: (index+1)*parent.height/3; height: 1; width: parent.width; color: "#90ffffff" }
        }
        MouseArea {
            anchors.fill: parent
            cursorShape: pressed ? Qt.ClosedHandCursor : Qt.OpenHandCursor
            preventStealing: true
            onPressed: mouse => { const p = mapToItem(overlay, mouse.x, mouse.y); overlay.begin("move", p.x, p.y) }
            onPositionChanged: mouse => { if (pressed) { const p = mapToItem(overlay, mouse.x, mouse.y); overlay.dragTo(p.x, p.y) } }
        }
    }
    Repeater {
        model: [ ["nw",0,0], ["n",0.5,0], ["ne",1,0], ["e",1,0.5], ["se",1,1], ["s",0.5,1], ["sw",0,1], ["w",0,0.5] ]
        Item {
            required property var modelData
            x: overlay.frameLeft + modelData[1]*overlay.frameWidth - width/2
            y: overlay.frameTop + modelData[2]*overlay.frameHeight - height/2
            width: 28; height: 28
            Rectangle {
                anchors.centerIn: parent
                width: modelData[1] === 0.5 ? 22 : 8
                height: modelData[2] === 0.5 ? 22 : 8
                radius: 2; color: "white"; border.color: "#88000000"
            }
            MouseArea {
                anchors.fill: parent
                preventStealing: true
                cursorShape: modelData[1] === 0.5 ? Qt.SizeVerCursor : modelData[2] === 0.5 ? Qt.SizeHorCursor : modelData[1] === modelData[2] ? Qt.SizeFDiagCursor : Qt.SizeBDiagCursor
                onPressed: mouse => { const p = mapToItem(overlay, mouse.x, mouse.y); overlay.begin(modelData[0], p.x, p.y) }
                onPositionChanged: mouse => { if (pressed) { const p = mapToItem(overlay, mouse.x, mouse.y); overlay.dragTo(p.x, p.y) } }
            }
        }
    }
}
