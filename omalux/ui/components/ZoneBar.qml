import QtQuick

// The zone bar of zone system (zonesystem.c dt_iop_zonesystem_bar_draw 482, button press 574,
// scroll 625, motion 653): the reference zones above (30 % of the height) and the mapped zones
// below, with darktable's zonemap (_iop_zonesystem_calculate_zonemap 128: unset control points,
// -1, are spread linearly between the set ones). Pressing a zone boundary sets its control
// point and dragging moves it between its neighbours; right-click clears it; the wheel changes
// the number of zones (4…24, scrolling up adds one) and clears the old last point. Keys with the
// focus: ←/→ pick a boundary, ↑/↓ move it by 0.01 (Shift ×10), +/- change the number of zones.
// Edits leave through edited({"size": n} or {"zone[k]": value}).
Item {
    id: root
    required property var theme
    property int size: 10
    property var zones: []
    property bool editable: true
    property alias navTarget: nav
    signal edited(var changes)
    signal interactionChanged(bool active)
    property int current: -1
    implicitHeight: 64
    readonly property int inset: 5
    function zone(k) { const v = Number((root.zones || [])[k]); return isFinite(v) ? v : -1 }
    readonly property var zonemap: {
        const n = root.size, map = [], z = []
        for (let k = 0; k <= n; ++k) z.push(root.zone(k))
        let steps = 0, pk = 0
        for (let k = 0; k < n; ++k) {
            if (k > 0 && k < n - 1 && z[k] === -1) steps++
            else {
                map[k] = k === 0 ? 0 : k === n - 1 ? 1 : z[k]
                for (let l = 1; l <= steps; ++l) map[pk + l] = map[pk] + (map[k] - map[pk]) / (steps + 1) * l
                pk = k; steps = 0
            }
        }
        return map
    }
    function boundaryAt(px) {
        const w = width - 2 * inset, x = Math.max(0, Math.min(w, px - inset)) / w, map = zonemap
        let k = root.size - 1
        for (let i = 0; i < root.size - 1; ++i) if (map[i + 1] >= x) { k = i; break }
        if (x > map[k] + (map[k + 1] - map[k]) / 2) k++
        return k
    }
    function setZone(k, value) { const ch = {}; ch["zone[" + k + "]"] = value; root.edited(ch) }
    function changeSize(delta) {
        const cs = Math.max(4, Math.min(24, root.size))
        const ch = { size: Math.max(4, Math.min(24, root.size + delta)) }
        ch["zone[" + cs + "]"] = -1
        root.edited(ch)
    }
    NavTarget {
        id: nav
        navId: "zonebar"
        label: "zones"
        kind: "graph"
        focusItem: root
        enabled: root.editable
        activateLabel: "EDIT ZONES"
    }
    Keys.onPressed: event => {
        const f = (event.modifiers & Qt.ShiftModifier) ? 10 : 1
        if (event.key === Qt.Key_Left || event.key === Qt.Key_Right) {
            const next = (root.current < 1 ? 1 : root.current) + (event.key === Qt.Key_Right ? 1 : -1)
            root.current = Math.max(1, Math.min(root.size - 2, next)); canvas.requestPaint(); event.accepted = true
        } else if ((event.key === Qt.Key_Up || event.key === Qt.Key_Down) && root.current > 0) {
            const map = root.zonemap, k = root.current
            const v = Math.max(map[k - 1] + .001, Math.min(map[k + 1] - .001, map[k] + (event.key === Qt.Key_Up ? .01 : -.01) * f))
            root.setZone(k, v); event.accepted = true
        } else if (event.key === Qt.Key_Plus || event.key === Qt.Key_Minus) {
            root.changeSize(event.key === Qt.Key_Plus ? 1 : -1); event.accepted = true
        }
    }
    Canvas {
        id: canvas
        anchors.fill: parent
        onWidthChanged: requestPaint()
        Connections { target: root; function onZonemapChanged() { canvas.requestPaint() } function onCurrentChanged() { canvas.requestPaint() } }
        onPaint: {
            const c = getContext("2d")
            c.fillStyle = Qt.rgba(.15, .15, .15, 1); c.fillRect(0, 0, width, height)
            const w = width - 2 * root.inset, h = height - 2 * root.inset, n = root.size, map = root.zonemap
            const s = 1 / (n - 2), split = .3
            for (let i = 0; i < n - 1; ++i) {
                const z = s * i
                c.fillStyle = Qt.rgba(z, z, z, 1)
                c.fillRect(root.inset + w * i / (n - 1), root.inset, w / (n - 1), h * split)
                c.fillRect(root.inset + w * map[i], root.inset + h * split, w * (map[i + 1] - map[i]), h * (1 - split))
            }
            c.strokeStyle = Qt.rgba(.1, .1, .1, 1); c.lineWidth = 1; c.strokeRect(root.inset + .5, root.inset + .5, w, h)
            const arrw = 7
            for (let k = 1; k < n - 1; ++k) {
                const set = root.zone(k) !== -1
                if (!set && k !== root.current && !(hover.hovered && root.boundaryAt(hover.point.position.x) === k)) continue
                const x = root.inset + w * map[k]
                c.beginPath(); c.moveTo(x - arrw / 2, height - 1); c.lineTo(x, height - 1 - arrw); c.lineTo(x + arrw / 2, height - 1); c.closePath()
                c.fillStyle = k === root.current ? root.theme.accent : Qt.rgba(.6, .6, .6, 1)
                c.strokeStyle = c.fillStyle
                if (set || k === root.current) c.fill(); else c.stroke()
            }
            if (nav.current || root.activeFocus) { c.strokeStyle = root.theme.accent; c.strokeRect(.5, .5, width - 1, height - 1) }
        }
    }
    HoverHandler { id: hover; onPointChanged: canvas.requestPaint() }
    MouseArea {
        anchors.fill: parent
        enabled: root.editable
        acceptedButtons: Qt.LeftButton | Qt.RightButton
        preventStealing: true
        property int dragging: -1
        onPressed: mouse => {
            nav.claim()
            const k = root.boundaryAt(mouse.x)
            if (k <= 0 || k >= root.size - 1) return
            if (mouse.button === Qt.RightButton) { root.setZone(k, -1); return }
            root.current = k
            if (root.zone(k) === -1) root.setZone(k, root.zonemap[k])
            dragging = k
            root.interactionChanged(true)
        }
        onPositionChanged: mouse => {
            if (dragging < 0) return
            const w = width - 2 * root.inset, x = Math.max(0, Math.min(w, mouse.x - root.inset)) / w, map = root.zonemap
            if (x > map[dragging - 1] && x < map[dragging + 1]) root.setZone(dragging, x)
        }
        onReleased: { if (dragging >= 0) root.interactionChanged(false); dragging = -1 }
        onWheel: wheel => { const d = wheel.angleDelta.y > 0 ? 1 : wheel.angleDelta.y < 0 ? -1 : 0; if (d) root.changeSize(d) }
    }
}
