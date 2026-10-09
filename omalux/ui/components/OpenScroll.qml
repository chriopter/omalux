import QtQuick
import "Scroll.js" as Scroll

// Keeps the sidebar pane steady around a block that opens or closes (`open`) under the
// pointer: what opens is scrolled into view while it grows, with its top kept on screen; what
// closes leaves the pane where it was, so the heading stays under the pointer (Scroll.js).
// Nothing happens for blocks that open by themselves (restored state, a search): `active`.
Item {
    id: root
    visible: false
    required property Item target
    property bool open: false
    property bool active: true
    property real born: 0
    property var release: null
    property real until: 0
    Component.onCompleted: born = Date.now()
    onOpenChanged: {
        if (release) { release(); release = null }
        until = 0
        if (!active || !target || !target.visible || Date.now() - born < 400) return
        if (!open) release = Scroll.hold(target)
        settle.interval = open ? 30 : 80
        settle.restart()
    }
    Timer {
        id: settle
        onTriggered: {
            if (root.release) { root.release(); root.release = null; return }
            root.until = Date.now() + 1200
            root.follow()
        }
    }
    // The block grows for a moment (the animation, rows built over a few frames): keep it in
    // view, but give way as soon as the person scrolls.
    property real at: 0
    function follow() {
        Scroll.reveal(root.target, 0, root.target.height)
        at = Scroll.position(root.target)
    }
    Connections {
        target: root.target
        function onHeightChanged() {
            if (Date.now() >= root.until) return
            if (Scroll.position(root.target) !== root.at) root.until = 0
            else root.follow()
        }
    }
    Component.onDestruction: if (release) release()
}
