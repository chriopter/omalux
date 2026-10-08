import QtQuick
import QtQuick.Controls

// The keyboard's way to darktable's multi-instance menu: a quiet "multiple instances" line at
// the end of an open module. Enter (or a click) opens the menu of the heading's InstanceButton,
// passed as `button`.
Item {
    id: root
    required property var theme
    property Item button: null
    property string navGroup: ""
    property bool enabled: true
    implicitHeight: 22
    ToolButton {
        id: link
        objectName: "instance-footer-" + root.navGroup
        anchors.left: parent.left
        padding: 0
        hoverEnabled: true
        enabled: root.enabled && !!root.button
        onClicked: { nav.claim(); root.button.openMenu() }
        Accessible.name: "multiple instances actions"
        NavTarget {
            id: nav
            navId: root.navGroup + "/@instances"
            label: "multiple instances"
            group: root.navGroup
            enabled: link.enabled
            activateLabel: "INSTANCE MENU"
            onActivate: root.button.openMenu()
        }
        contentItem: Text {
            text: "multiple instances"
            color: link.hovered || link.visualFocus || nav.current ? root.theme.accent : root.theme.muted
            font: root.theme.textFont
        }
        background: Item {}
    }
}
