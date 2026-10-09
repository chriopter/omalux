import QtQuick
import "Scroll.js" as Scroll

// Keeps the sidebar pane steady around a block that opens or closes (`open`) under the
// pointer. The row or heading that was clicked never moves: what opens grows downward and is
// not scrolled into view (the person scrolls on when they want the rest); what closes leaves
// the pane where it was, so the click target stays under the pointer (Scroll.hold).
// Nothing happens for blocks that change by themselves (restored state, a search): `active`.
Item {
    id: root
    visible: false
    required property Item target
    property bool open: false
    property bool active: true
    property real born: 0
    property var release: null
    Component.onCompleted: born = Date.now()
    onOpenChanged: {
        if (release) { release(); release = null }
        if (open || !active || !target || !target.visible || Date.now() - born < 400) return
        release = Scroll.hold(target)
        settle.restart()
    }
    Timer {
        id: settle
        interval: 80
        onTriggered: if (root.release) { root.release(); root.release = null }
    }
    Component.onDestruction: if (release) release()
}
