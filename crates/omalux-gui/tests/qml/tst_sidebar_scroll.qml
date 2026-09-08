import QtQuick
import QtQuick.Controls
import QtTest
import "../../qml/components"
Item {
    width: 300; height: 300
    ScrollView {
        id: view
        anchors.fill: parent
        contentWidth: availableWidth
        SidebarScrollHandler { id: handler; flickable: view.contentItem as Flickable }
        Column {
            width: view.availableWidth
            Repeater { model: 50; Slider { width: 250; height: 40 } }
        }
    }
    TestCase {
        name: "SidebarScrolling"
        when: windowShown
        function test_wheelOverSlider() {
            mouseWheel(view, 100, 100, 0, -120)
            tryCompare(view.contentItem, "contentY", 144)
            handler.scroll(-0.5, -120)
            compare(view.contentItem.contentY, 145.5)
            handler.scroll(10000, 0)
            compare(view.contentItem.contentY, view.contentItem.originY)
            handler.scroll(-10000, 0)
            compare(view.contentItem.contentY, view.contentItem.originY + view.contentItem.contentHeight - view.contentItem.height)
        }

    }
}
