import QtQuick
import QtQuick.Controls

// The last section of an open module: "multiple instances", darktable's multi-instance menu
// for the keyboard and for those who look for it among the sections. Enter (or a click) opens
// the menu of the heading's InstanceButton, passed as `button`; `count` instances exist.
SectionRow {
    id: root
    property Item button: null
    property string navGroup: ""
    property bool active: true
    property int count: 0
    objectName: "instance-footer-" + root.navGroup
    label: "multiple instances"
    summary: root.count > 1 ? root.count + " instances" : ""
    menu: true
    enabled: root.active && !!root.button
    navTarget.navId: root.navGroup + "/@instances"
    navTarget.group: root.navGroup
    navTarget.activateLabel: "INSTANCE MENU"
    onRequested: root.button.openMenu()
    Accessible.name: "multiple instances actions"
}
