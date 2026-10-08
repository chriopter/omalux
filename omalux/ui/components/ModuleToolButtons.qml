import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

// One row of a module's pickers and buttons, as darktable puts them side by side (the three
// level pickers, auto and auto region, the four flip buttons). A picker shows darktable's
// pipette and stays highlighted while it is active; a button runs once, or opens its menu
// (agx "reset primaries"). `entries` holds
// [{ label, kind: "area"|"point"|"pointarea"|"button", icon, hint, active, menu: [{ label }] }];
// the component only reports which entry (and menu item, else -1) was chosen.
Item {
    id: root
    required property var theme
    property var entries: []
    property bool editable: true
    property string navPrefix: "tools"
    property string navGroup: ""
    // A short value darktable shows next to the picker (e.g. the picked input lightness).
    property string report: ""
    signal triggered(int index, int choice)

    implicitHeight: Math.max(28, layout.implicitHeight + 4)
    RowLayout {
        id: layout
        x: 0
        width: root.width - 28
        anchors.verticalCenter: parent.verticalCenter
        spacing: 6
        Repeater {
            model: root.entries
            delegate: AbstractButton {
                id: button
                required property var modelData
                required property int index
                readonly property bool picker: modelData.kind !== "button"
                readonly property bool hasMenu: !!modelData.menu && modelData.menu.length > 0
                function run() {
                    if (hasMenu) menu.popup(button, 0, button.height)
                    else root.triggered(index, -1)
                }
                objectName: "module-tool-" + root.navPrefix + "-" + index
                Layout.fillWidth: true
                Layout.preferredWidth: 1
                implicitHeight: 26
                enabled: root.editable
                hoverEnabled: true
                onClicked: { nav.claim(); run() }
                Accessible.name: (modelData.label || modelData.hint || "") + (picker ? " picker" : "")
                Accessible.checkable: picker
                Accessible.checked: !!modelData.active
                ToolTip.visible: hovered && !!modelData.hint
                ToolTip.delay: 600
                ToolTip.text: modelData.hint || ""
                NavTarget {
                    id: nav
                    navId: root.navPrefix + "/" + button.index
                    label: button.modelData.label || "button"
                    kind: "button"
                    group: root.navGroup
                    enabled: root.editable
                    activateLabel: button.picker ? (button.modelData.active ? "STOP PICKING" : "PICK")
                                                 : (button.modelData.label || "run").toUpperCase()
                    onActivate: button.run()
                }
                Menu {
                    id: menu
                    Repeater {
                        model: button.hasMenu ? button.modelData.menu : []
                        MenuItem {
                            required property var modelData
                            required property int index
                            text: modelData.label
                            onTriggered: root.triggered(button.index, index)
                        }
                    }
                }
                contentItem: RowLayout {
                    spacing: 6
                    Item { Layout.fillWidth: true }
                    Canvas {
                        visible: button.picker || camera || wand
                        Layout.preferredWidth: 12; Layout.preferredHeight: 12
                        property color stroke: label.color
                        property bool camera: button.modelData.icon === "camera"
                        property bool wand: button.modelData.icon === "wand"
                        onStrokeChanged: requestPaint()
                        onPaint: {
                            const c = getContext("2d")
                            c.clearRect(0, 0, width, height)
                            c.strokeStyle = stroke; c.fillStyle = stroke; c.lineWidth = 1.3
                            if (camera) {
                                // darktable's camera glyph (dtgtk_cairo_paint_camera)
                                c.strokeRect(1, 3.5, 10, 7)
                                c.fillRect(3.5, 1.5, 4, 2)
                                c.beginPath(); c.arc(6, 7, 2, 0, Math.PI * 2); c.stroke()
                                return
                            }
                            if (wand) {
                                // darktable's magic wand (dtgtk_cairo_paint_wand): a stick and a spark
                                c.beginPath(); c.moveTo(1.5, 10.5); c.lineTo(7.5, 4.5); c.stroke()
                                c.beginPath(); c.moveTo(9, 0.5); c.lineTo(9, 5.5); c.moveTo(6.5, 3); c.lineTo(11.5, 3); c.stroke()
                                return
                            }
                            // darktable's pipette: a slanted dropper with a bulb.
                            c.beginPath(); c.moveTo(1.5, 10.5); c.lineTo(7, 5); c.stroke()
                            c.beginPath(); c.moveTo(5.5, 3.5); c.lineTo(8.5, 6.5); c.stroke()
                            c.beginPath(); c.arc(9, 3, 2.2, 0, Math.PI * 2); c.fill()
                        }
                    }
                    Text {
                        id: label
                        visible: text !== ""
                        text: (button.modelData.icon && ["camera", "wand"].indexOf(button.modelData.icon) < 0 ? button.modelData.icon + " " : "")
                              + (button.modelData.label || "") + (button.hasMenu ? " ▾" : "")
                        color: !button.enabled ? root.theme.muted
                             : button.modelData.active || nav.current || button.hovered ? root.theme.accent : root.theme.ink
                        font: root.theme.textFont
                        elide: Text.ElideRight
                        Layout.maximumWidth: button.width - (button.picker ? 26 : 8)
                    }
                    Item { Layout.fillWidth: true }
                }
                background: Rectangle {
                    radius: 5
                    color: button.modelData.active ? Qt.rgba(root.theme.accent.r, root.theme.accent.g, root.theme.accent.b, 0.18)
                         : button.pressed ? root.theme.active : button.hovered ? root.theme.hover : "transparent"
                    border.width: 1
                    border.color: button.modelData.active || nav.current ? root.theme.accent : root.theme.line
                }
            }
        }
        Text {
            visible: root.report !== ""
            text: root.report
            color: root.theme.muted
            font: root.theme.textFont
        }
    }
}
