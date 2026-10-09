import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

// The one entry to darktable's multi-instance actions (develop/imageop.c,
// _gui_multiinstance_callback): a quiet "⋯" button in a module's heading or strip. Its menu
// lists, under the caption "instances", darktable's "new instance", "duplicate instance",
// "move up", "move down", "delete" and "rename"; right-click creates a new instance. Entries are
// enabled as darktable's _get_multi_show decides (moduleState.canNew, canMoveUp, …). Move up
// means later in the pipeline, as in darktable's right-hand panel. Rename opens a field; an
// empty name gives the module back its automatic label. Actions leave through
// instanceRequested(action, name); nothing is changed here. From the keyboard, I opens the menu
// of the selected module (KeyboardNavigator.menuGroup).
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
    onClicked: { nav.claim(); root.openMenu() }
    Accessible.name: "more actions for " + root.title + ": instances"
    // Read before the click; the menu then takes its place.
    property bool tipDismissed: false
    onPressedChanged: if (pressed) tipDismissed = true
    onHoveredChanged: if (!hovered) tipDismissed = false
    property bool menuShown: false
    ToolTip.visible: hovered && !tipDismissed && !menuShown
    ToolTip.delay: 900
    ToolTip.text: "more: run this module a second time, reorder, rename (I)"
    TapHandler {
        acceptedButtons: Qt.RightButton
        enabled: root.ready && root.moduleState.canNew
        onTapped: root.instanceRequested("new", "")
    }
    // Not an ↑/↓ stop (it would sit between every heading and its rows): I opens the menu.
    NavTarget {
        id: nav
        navId: root.navGroup + "/@instances-button"
        label: "instances"
        kind: "button"
        group: root.navGroup
        enabled: root.ready
        listed: false
        activateLabel: "INSTANCES"
        onActivate: root.openMenu()
    }
    function openMenu() { menuShown = true; menu.get().popup(0, root.height) }
    // An overflow mark: three dots.
    contentItem: Canvas {
        property color stroke: nav.current ? root.theme.accent : root.hovered ? root.theme.ink : root.theme.muted
        onStrokeChanged: requestPaint()
        onPaint: {
            const c = getContext("2d")
            c.clearRect(0, 0, width, height)
            c.fillStyle = stroke
            const cx = Math.round(width / 2), cy = Math.round(height / 2)
            for (const dx of [-4, 0, 4]) { c.beginPath(); c.arc(cx + dx, cy, 1.1, 0, 2 * Math.PI); c.fill() }
        }
    }
    background: Rectangle {
        radius: 3
        color: root.pressed ? root.theme.active : root.hovered ? root.theme.hover : "transparent"
        border.color: nav.current ? root.theme.accent : "transparent"
    }

    // The menu and the rename field are built on first use, not with every module heading.
    OnDemand {
        id: menu
        parent: root
        Menu {
            objectName: "instance-menu-" + root.navGroup
            onClosed: root.menuShown = false
            // What the entries are about, in darktable's word for it.
            Text {
                text: "instances"
                color: root.theme.muted
                font: root.theme.textFont
                leftPadding: 12; topPadding: 6; bottomPadding: 4
            }
            MenuItem { text: "new instance"; enabled: root.ready && root.moduleState.canNew; onTriggered: root.instanceRequested("new", "") }
            MenuItem { text: "duplicate instance"; enabled: root.ready && root.moduleState.canNew; onTriggered: root.instanceRequested("duplicate", "") }
            MenuItem { text: "move up"; enabled: root.ready && root.moduleState.canMoveUp; onTriggered: root.instanceRequested("up", "") }
            MenuItem { text: "move down"; enabled: root.ready && root.moduleState.canMoveDown; onTriggered: root.instanceRequested("down", "") }
            MenuItem { text: "delete"; enabled: root.ready && root.moduleState.canDelete; onTriggered: root.instanceRequested("delete", "") }
            MenuSeparator {}
            MenuItem { text: "rename"; enabled: root.ready; onTriggered: root.rename() }
        }
    }
    function rename() {
        // dt_iop_gui_rename_module: the field starts with the current name, empty for an
        // unnamed first instance.
        renamer.get().edit(root.moduleState ? root.moduleState.instanceLabel : "")
    }
    OnDemand {
        id: renamer
        parent: root
        Popup {
            id: renamePopup
            objectName: "instance-rename-" + root.navGroup
            x: root.width - width
            y: root.height
            padding: 6
            property bool accepted: false
            onOpened: accepted = false
            function edit(text) { field.text = text; open(); field.forceActiveFocus(); field.selectAll() }
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
                onAccepted: { renamePopup.accepted = true; root.instanceRequested("rename", text); renamePopup.close() }
                Keys.onEscapePressed: event => { renamePopup.accepted = true; renamePopup.close(); event.accepted = true }
                NavTarget { navId: root.navGroup + "/@rename"; label: "rename"; kind: "search"; input: field; listed: false }
                background: Rectangle { color: root.theme.surface; border.color: field.activeFocus ? root.theme.accent : root.theme.line; radius: 3 }
            }
        }
    }
}
