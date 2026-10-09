import QtQuick

// The rows of a module block under its heading. When rows are added (the module or a section
// inside it opens) the height follows in one quick movement instead of a jump; the rows are
// clipped only while it runs. Rows that go away take their height with them at once: what
// lies below must not slide under a pointer that is about to click it. The body spans the
// block (14 px left of the rows), so nothing that reaches into the margin is cut.
Item {
    id: root
    default property alias content: column.data
    property alias spacing: column.spacing
    property bool animate: true
    readonly property bool moving: motion.running
    readonly property real wantedHeight: column.implicitHeight
    property real shownHeight: 0
    property bool ready: false
    Component.onCompleted: { shownHeight = wantedHeight; ready = true }
    onWantedHeightChanged: {
        if (!ready) return
        if (animate && wantedHeight > shownHeight) {
            motion.from = shownHeight; motion.to = wantedHeight; motion.restart()
        } else {
            motion.stop(); shownHeight = wantedHeight
        }
    }
    NumberAnimation { id: motion; target: root; property: "shownHeight"; duration: 120; easing.type: Easing.OutCubic }
    x: -14
    width: parent ? parent.width + 14 : 0
    implicitHeight: shownHeight
    clip: motion.running
    Column {
        id: column
        x: 14
        width: root.width - 14
    }
}
