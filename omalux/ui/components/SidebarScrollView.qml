import QtQuick
import QtQuick.Controls

ScrollView {
    id: root
    clip: true
    contentWidth: availableWidth
    ScrollBar.horizontal.policy: ScrollBar.AlwaysOff
    SidebarWheelHandler { flickable: root.contentItem }
    // Scroll.hold() keeps the pane in place with a bottom margin when content closes; the
    // margin shrinks again as soon as the person scrolls up or the content grows.
    Connections {
        target: root.contentItem
        function release() {
            const flick = root.contentItem
            if (flick.bottomMargin <= 0) return
            const needed = Math.max(0, flick.contentY + flick.height - flick.originY - flick.contentHeight)
            if (needed < flick.bottomMargin) flick.bottomMargin = needed
        }
        function onContentYChanged() { release() }
        function onContentHeightChanged() { release() }
    }
    // Direct wheel/touchpad handling; never pan sideways or overshoot.
    Binding {
        target: root.contentItem
        property: "flickableDirection"
        value: Flickable.VerticalFlick
    }
    Binding {
        target: root.contentItem
        property: "boundsBehavior"
        value: Flickable.StopAtBounds
    }
}
