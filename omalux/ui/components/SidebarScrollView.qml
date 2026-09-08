import QtQuick
import QtQuick.Controls

ScrollView {
    id: root
    clip: true
    contentWidth: availableWidth
    ScrollBar.horizontal.policy: ScrollBar.AlwaysOff
    SidebarWheelHandler { flickable: root.contentItem }
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
