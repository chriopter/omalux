import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

// darktable's multi-instance button in a module heading (develop/imageop.c,
// _gui_multiinstance_callback): click opens the menu "new instance", "duplicate instance",
// "move up", "move down", "delete", "rename"; right-click creates a new instance. Entries are
// enabled as darktable's _get_multi_show decides (moduleState.canNew, canMoveUp, …). Move up
// means later in the pipeline, as in darktable's right-hand panel. Rename opens a field; an
// empty name gives the module back its automatic label. Actions leave through
// instanceRequested(action, name); nothing is changed here.
ToolButton {
    id: root
    required property var theme
    property var moduleState
    property string title: ""
    property bool editable: true
    property string navGroup: ""
    signal instanceRequested(string action, string name)

    readonly property bool ready: root.editable && !!root.moduleState
    implicitWidth: 22
    implicitHeight: 20
    padding: 0
    hoverEnabled: true
    enabled: ready
    onClicked: { nav.claim(); menu.popup(0, root.height) }
    Accessible.name: "multiple instances actions for " + root.title
    ToolTip.visible: hovered
    ToolTip.delay: 900
    ToolTip.text: "multiple instances actions\nright-click creates new instance"
    TapHandler {
        acceptedButtons: Qt.RightButton
        enabled: root.ready && root.moduleState.canNew
        onTapped: root.instanceRequested("new", "")
    }
    // The heading button is not an ↑/↓ stop (it would sit between every heading and its rows);
    // the keyboard reaches the same menu through InstanceFooter at the end of an open module.
    NavTarget {
        id: nav
        navId: root.navGroup + "/@instances-button"
        label: "multiple instances actions"
        kind: "button"
        group: root.navGroup
        enabled: root.ready
        listed: false
        activateLabel: "INSTANCES"
        onActivate: root.openMenu()
    }
    function openMenu() { menu.popup(0, root.height) }
    // darktable's multi-instance icon: two overlapping frames.
    contentItem: Canvas {
        property color stroke: root.hovered || nav.current ? root.theme.accent : root.theme.muted
        onStrokeChanged: requestPaint()
        onPaint: {
            const c = getContext("2d")
            c.clearRect(0, 0, width, height)
            c.strokeStyle = stroke; c.lineWidth = 1.2
            const s = Math.min(width, height) * .5, x = (width - s * 1.35) / 2, y = (height - s * 1.35) / 2
            c.strokeRect(x + .5, y + .5, s, s)
            c.strokeRect(x + s * .35 + .5, y + s * .35 + .5, s, s)
        }
    }
    background: Rectangle { color: "transparent"; radius: 3; border.color: nav.current ? root.theme.accent : "transparent" }

    Menu {
        id: menu
        objectName: "instance-menu-" + root.navGroup
        MenuItem { text: "new instance"; enabled: root.ready && root.moduleState.canNew; onTriggered: root.instanceRequested("new", "") }
        MenuItem { text: "duplicate instance"; enabled: root.ready && root.moduleState.canNew; onTriggered: root.instanceRequested("duplicate", "") }
        MenuItem { text: "move up"; enabled: root.ready && root.moduleState.canMoveUp; onTriggered: root.instanceRequested("up", "") }
        MenuItem { text: "move down"; enabled: root.ready && root.moduleState.canMoveDown; onTriggered: root.instanceRequested("down", "") }
        MenuItem { text: "delete"; enabled: root.ready && root.moduleState.canDelete; onTriggered: root.instanceRequested("delete", "") }
        MenuSeparator {}
        MenuItem { text: "rename"; enabled: root.ready; onTriggered: root.rename() }
    }
    function rename() {
        // dt_iop_gui_rename_module: the field starts with the current name, empty for an
        // unnamed first instance.
        field.text = root.moduleState ? root.moduleState.instanceLabel : ""
        renamer.open()
        field.forceActiveFocus()
        field.selectAll()
    }
    Popup {
        id: renamer
        objectName: "instance-rename-" + root.navGroup
        x: root.width - width
        y: root.height
        padding: 6
        property bool accepted: false
        onOpened: accepted = false
        // A field that loses focus keeps its name, as darktable's rename entry does.
        onClosed: if (!accepted && field.text !== (root.moduleState ? root.moduleState.instanceLabel : "")) root.instanceRequested("rename", field.text)
        TextField {
            id: field
            width: 180
            maximumLength: 127
            color: root.theme.ink
            font: root.theme.textFont
            placeholderText: "name of this instance"
            Accessible.name: "rename " + root.title
            onAccepted: { renamer.accepted = true; root.instanceRequested("rename", text); renamer.close() }
            Keys.onEscapePressed: event => { renamer.accepted = true; renamer.close(); event.accepted = true }
            NavTarget { navId: root.navGroup + "/@rename"; label: "rename"; kind: "search"; input: field; listed: false }
            background: Rectangle { color: root.theme.surface; border.color: field.activeFocus ? root.theme.accent : root.theme.line; radius: 3 }
        }
    }
}
