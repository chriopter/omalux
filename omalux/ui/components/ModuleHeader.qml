import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

// The heading of a module block, shared by the curated blocks (FilterModule) and the generated
// modules (GeneratedModule), closed and open alike:
//
//   ● icon  name • instance …………………………  reset  instances  ⌄
//
// The state mark, icon and name switch the module on and off (a ring while it is off and the
// module is open). The rest of the heading and the chevron fold and unfold it; the chevron sits
// in the disclosure column, where the collapsed rows have theirs. Reset is offered while the
// module is open; the instances button is darktable's multi-instance menu. Right-click offers
// the same actions. The heading also draws the block behind the module (`blockHeight`): the
// raised surface, outlined while the module is open.
Item {
    id: root
    required property var theme
    required property string operation
    required property string name
    property string instanceLabel: ""
    property string purpose: ""
    property bool deprecated: false
    property var moduleState
    property bool moduleEnabled: false
    // The heading acts (a photograph is open and the engine knows the module).
    property bool ready: true
    property bool expanded: false
    property bool hasDetails: true
    property bool showDisclosure: hasDetails
    property bool showInstances: true
    // Height of the block behind the module, from the heading's top.
    property real blockHeight: height
    // Object names of the parts ("<prefix>module-toggle-<suffix>" …).
    property string namePrefix: ""
    property string nameSuffix: operation
    property string instancesSuffix: nameSuffix
    property string navId: ""
    property string navGroup: ""
    property alias navTarget: headerNav
    property alias instanceButton: instanceButton
    readonly property string title: name + (instanceLabel !== "" ? " • " + instanceLabel : "")
    signal toggleRequested()
    signal expansionRequested()
    signal resetRequested()
    signal instanceRequested(string action, string name)

    x: -14
    width: parent ? parent.width + 14 : 0
    implicitHeight: Math.max(30, nameText.contentHeight + 12)

    // Keyboard stop: Enter or ←/→ fold and unfold, E switches the module, Shift+R resets it.
    NavTarget {
        id: headerNav
        navId: root.navId
        label: root.title
        kind: "module"
        group: root.navGroup
        enabled: root.ready
        groupActions: true
        adjustLabel: root.hasDetails ? "COLLAPSE/EXPAND" : ""
        activateLabel: root.hasDetails ? (root.expanded ? "COLLAPSE" : "EXPAND") : ""
        onActivate: if (root.hasDetails) root.expansionRequested()
        onAdjust: steps => { if (root.hasDetails && (steps > 0) !== root.expanded) root.expansionRequested() }
        onToggleGroup: root.toggleRequested()
        onResetGroup: root.resetRequested()
    }
    Rectangle {
        id: block
        objectName: root.namePrefix + "module-block-" + root.nameSuffix
        z: -1
        width: parent.width
        height: root.blockHeight
        color: root.theme.surface
        radius: root.expanded ? 6 : 0
        border.width: root.expanded ? 1 : 0
        border.color: headerNav.current ? root.theme.accent : root.theme.line
    }
    // Everything of the heading that is not a button folds and unfolds the module.
    MouseArea {
        id: foldArea
        objectName: root.namePrefix + "module-fold-" + root.nameSuffix
        anchors.fill: parent
        enabled: root.hasDetails
        hoverEnabled: true
        acceptedButtons: Qt.LeftButton
        onClicked: { headerNav.claim(); root.expansionRequested() }
    }
    TapHandler {
        acceptedButtons: Qt.RightButton
        onTapped: (eventPoint, button) => menu.get().popup(eventPoint.position.x, eventPoint.position.y)
    }
    // Built on first use, not with every module.
    OnDemand {
        id: menu
        parent: root
        Menu {
            MenuItem {
                text: (root.moduleEnabled ? "Disable " : "Enable ") + root.title
                enabled: root.ready
                onTriggered: root.toggleRequested()
            }
            MenuItem {
                text: "Reset " + root.title
                enabled: root.ready
                onTriggered: root.resetRequested()
            }
            MenuSeparator {}
            MenuItem {
                text: "new instance"
                enabled: root.ready && !!root.moduleState && root.moduleState.canNew
                onTriggered: root.instanceRequested("new", "")
            }
            MenuItem {
                text: "duplicate instance"
                enabled: root.ready && !!root.moduleState && root.moduleState.canNew
                onTriggered: root.instanceRequested("duplicate", "")
            }
        }
    }
    RowLayout {
        id: headerContent
        anchors.fill: parent
        anchors.leftMargin: 8; anchors.rightMargin: 28
        spacing: 4
        readonly property real reserved: 28 + (resetButton.visible ? 26 : 0) + (instanceButton.visible ? 26 : 0)
        ToolButton {
            id: heading
            objectName: root.namePrefix + "module-toggle-" + root.nameSuffix
            // As wide as its content while there is room (whole pixels, or the name would be
            // cut by a fraction), narrower when the instance name is long.
            Layout.fillWidth: true
            Layout.minimumWidth: 40
            // A long name wraps rather than fill the heading: a stretch that folds stays free.
            Layout.maximumWidth: Math.min(Math.ceil(implicitWidth) + 1, headerContent.width - headerContent.reserved)
            Layout.alignment: Qt.AlignVCenter
            padding: 0
            enabled: root.ready
            hoverEnabled: true
            onClicked: { headerNav.claim(); root.toggleRequested() }
            Accessible.name: "Enable " + root.title
            Accessible.checkable: true; Accessible.checked: root.moduleEnabled
            // The purpose is read before using the heading; a click puts it away until the
            // pointer comes back, so it does not cover the rows.
            property bool tipDismissed: false
            onPressedChanged: if (pressed) tipDismissed = true
            onHoveredChanged: if (!hovered) tipDismissed = false
            ToolTip.visible: hovered && !tipDismissed && root.purpose !== ""
            ToolTip.delay: 900
            ToolTip.text: root.purpose
            contentItem: RowLayout {
                spacing: 7
                // On: a filled mark. Off: nothing on a closed card, a ring on an open module,
                // where the mark is the switch one looks for.
                Rectangle {
                    Layout.preferredWidth: 7; Layout.preferredHeight: 7
                    radius: 3.5
                    color: root.moduleEnabled ? root.theme.ink : "transparent"
                    border.width: !root.moduleEnabled && root.expanded ? 1 : 0
                    border.color: root.theme.muted
                }
                ModuleIcon {
                    moduleKey: root.operation
                    opacity: root.moduleEnabled ? 1 : .6
                }
                Text {
                    id: nameText
                    Layout.fillWidth: true
                    Layout.minimumWidth: Math.min(110, Math.ceil(implicitWidth))
                    Layout.maximumWidth: Math.ceil(implicitWidth) + 1
                    wrapMode: Text.WordWrap
                    text: root.name
                    color: headerNav.current || heading.activeFocus ? root.theme.accent
                         : root.moduleEnabled || heading.hovered ? root.theme.ink : root.theme.muted
                    font: root.theme.moduleHeadingFont
                }
                // The instance name (often a preset name) in a quieter tone, cut where the
                // heading ends, so it never wraps the block.
                Text {
                    Layout.fillWidth: true
                    visible: root.instanceLabel !== ""
                    text: "• " + root.instanceLabel
                    color: root.theme.muted
                    font: root.theme.textFont
                    elide: Text.ElideRight
                }
                Text {
                    visible: root.deprecated
                    text: "deprecated"
                    color: root.theme.muted; font: root.theme.textFont
                }
            }
            background: Rectangle {
                x: -4; y: -3; width: parent.width + 8; height: parent.height + 6
                radius: 4
                color: heading.pressed ? root.theme.active : heading.hovered ? root.theme.hover : "transparent"
                border.width: heading.activeFocus ? 1 : 0
                border.color: root.theme.accent
            }
        }
        Item { Layout.fillWidth: true }
        ToolButton {
            id: resetButton
            objectName: root.namePrefix + "module-reset-" + root.nameSuffix
            visible: root.expanded
            implicitWidth: 22; implicitHeight: 20
            padding: 0
            hoverEnabled: true
            enabled: root.ready
            onClicked: { headerNav.claim(); root.resetRequested() }
            Accessible.name: "Reset " + root.title
            ToolTip.visible: hovered
            ToolTip.delay: 900
            ToolTip.text: "reset parameters"
            // darktable's reset icon: an open circle with an arrow head.
            contentItem: Canvas {
                property color stroke: resetButton.hovered ? root.theme.ink : root.theme.muted
                onStrokeChanged: requestPaint()
                onPaint: {
                    const c = getContext("2d")
                    c.clearRect(0, 0, width, height)
                    c.strokeStyle = stroke; c.fillStyle = stroke; c.lineWidth = 1.2
                    const cx = width / 2, cy = height / 2, r = 4.5
                    c.beginPath(); c.arc(cx, cy, r, -Math.PI * .35, Math.PI * 1.25); c.stroke()
                    const ax = cx + r * Math.cos(-Math.PI * .35), ay = cy + r * Math.sin(-Math.PI * .35)
                    c.beginPath(); c.moveTo(ax + 2.6, ay + 1.2); c.lineTo(ax - 2.2, ay + 1.6); c.lineTo(ax + .6, ay - 3); c.closePath(); c.fill()
                }
            }
            background: Rectangle {
                radius: 3
                color: resetButton.pressed ? root.theme.active : resetButton.hovered ? root.theme.hover : "transparent"
            }
        }
        InstanceButton {
            id: instanceButton
            objectName: root.namePrefix + "module-instances-" + root.instancesSuffix
            visible: root.showInstances && !!root.moduleState
            theme: root.theme
            moduleState: root.moduleState
            title: root.title
            editable: root.ready
            navGroup: root.navGroup
            onInstanceRequested: (action, name) => root.instanceRequested(action, name)
        }
    }
    DisclosureButton {
        objectName: root.namePrefix + "module-details-" + root.nameSuffix
        visible: root.showDisclosure
        anchors.right: parent.right
        anchors.verticalCenter: parent.verticalCenter
        theme: root.theme
        expanded: root.expanded
        hot: foldArea.containsMouse
        onClicked: root.expansionRequested()
        Accessible.name: (root.expanded ? "Hide details for " : "Details for ") + root.title
    }
}
