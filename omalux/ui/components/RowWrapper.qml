import QtQuick
import QtQuick.Controls

// Frame for a choice or switch row among sliders: its content is shifted so the label lines
// up with slider labels and its value with the slider values (the right-hand disclosure
// column stays free). Right-click offers the reset, as on a slider.
Item {
    id: root
    required property var theme
    property string label: ""
    property bool resetEnabled: true
    signal resetRequested()
    default property alias content: holder.data
    implicitHeight: holder.childrenRect.height
    Item {
        id: holder
        x: -8
        width: root.width - 28 + 16
        height: childrenRect.height
    }
    TapHandler {
        acceptedButtons: Qt.RightButton
        onTapped: (eventPoint, button) => menu.get().popup(eventPoint.position.x, eventPoint.position.y)
    }
    // Built on first use, not with every row.
    OnDemand {
        id: menu
        parent: root
        Menu {
            MenuItem {
                text: "Reset " + root.label
                enabled: root.resetEnabled
                onTriggered: root.resetRequested()
            }
        }
    }
}
