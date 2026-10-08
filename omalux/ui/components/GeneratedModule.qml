import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

// One darktable module built from the generated layout, in the curated block's visual
// language: the heading carries the module name and an accent dot for its enabled state
// (click toggles it, right-click offers reset), primary rows stay visible while collapsed,
// the chevron opens the detail rows and "more" the advanced ones.
Column {
    id: root
    required property var theme
    required property var module
    required property var moduleState
    required property var catalogModel
    property int instance: 0
    property var overrides: ({})
    property bool editable: true
    property bool expanded: false
    property bool moreOpen: false
    property string term: ""
    property string activeControl: ""
    signal expansionRequested()
    signal moreRequested()
    signal changesRequested(var changes)
    signal enableRequested(bool enabled)
    signal resetRequested()
    signal interactionChanged(bool active)
    signal controlSelected(string id)

    // A click shows the new state at once; the engine's answer can take a while (a slow module
    // renders first), and without feedback a second click would switch it back.
    readonly property bool engineEnabled: !!moduleState && moduleState.enabled
    property var requestedEnabled: undefined
    readonly property bool moduleEnabled: requestedEnabled !== undefined ? requestedEnabled : engineEnabled
    onEngineEnabledChanged: requestedEnabled = undefined
    function requestEnabled(on) {
        requestedEnabled = on
        enableSettle.restart()
        enableRequested(on)
    }
    Timer { id: enableSettle; interval: 30000; onTriggered: root.requestedEnabled = undefined }
    readonly property bool nameMatched: term !== "" && catalogModel.moduleNameMatches(module, term)
    // darktable's heading: the module name and, after a dot, the instance name
    // (_iop_panel_name; moduleState.instanceLabel from module_instances.c).
    readonly property string instanceLabel: moduleState && moduleState.instanceLabel !== undefined
                                            ? moduleState.instanceLabel : (instance > 0 ? String(instance) : "")
    readonly property string title: module.name + (instanceLabel !== "" ? " • " + instanceLabel : "")
    // Multi-instance actions and drawn-shape requests travel with the edits as action keys
    // (EditorSidebar.changeParameters): {"@instance": action, "@name": name}, {"@drawn": type}.
    function requestInstance(action, name) { root.changesRequested({ "@instance": action, "@name": name }) }
    spacing: 4
    topPadding: 12
    bottomPadding: 8

    Item {
        id: moduleHeader
        x: -14
        width: parent.width + 14
        implicitHeight: Math.max(30, headerContent.implicitHeight + 10)
        // Keyboard stop: Enter or ←/→ open and close the details, E switches the module,
        // Shift+R resets it.
        NavTarget {
            id: headerNav
            navId: "module:" + root.module.operation + "/" + root.instance
            label: root.title
            kind: "module"
            group: root.module.operation + "/" + root.instance
            enabled: root.editable && !!root.moduleState
            groupActions: true
            adjustLabel: rows.hasDetails && root.term === "" ? "COLLAPSE/EXPAND" : ""
            activateLabel: rows.hasDetails && root.term === "" ? (root.expanded ? "COLLAPSE" : "EXPAND") : ""
            onActivate: if (rows.hasDetails) root.expansionRequested()
            onAdjust: steps => { if (rows.hasDetails && (steps > 0) !== root.expanded) root.expansionRequested() }
            onToggleGroup: root.requestEnabled(!root.moduleEnabled)
            onResetGroup: root.resetRequested()
        }
        Rectangle {
            z: -1
            width: parent.width
            height: root.height - moduleHeader.y
            color: root.theme.surface
        }
        RowLayout {
            id: headerContent
            anchors.fill: parent
            anchors.leftMargin: 8; anchors.rightMargin: 28
            anchors.topMargin: 5; anchors.bottomMargin: 5
            spacing: 6
            ToolButton {
                id: heading
                objectName: "module-toggle-" + root.module.operation
                Layout.fillWidth: true
                padding: 0
                enabled: root.editable && !!root.moduleState
                hoverEnabled: true
                onClicked: { headerNav.claim(); root.requestEnabled(!root.moduleEnabled) }
                Accessible.name: "Enable " + root.title
                Accessible.checkable: true; Accessible.checked: root.moduleEnabled
                // The purpose is read before using the heading; a click puts it away until the
                // pointer comes back, so it does not cover the rows.
                property bool tipDismissed: false
                onPressedChanged: if (pressed) tipDismissed = true
                onHoveredChanged: if (!hovered) tipDismissed = false
                ToolTip.visible: hovered && !tipDismissed && !!root.module.purpose
                ToolTip.delay: 900
                ToolTip.text: root.module.purpose || ""
                contentItem: RowLayout {
                    spacing: 7
                    Rectangle { opacity: root.moduleEnabled ? 1 : 0; width: 7; height: 7; radius: 3.5; color: root.theme.ink }
                    Text {
                        Layout.fillWidth: true
                        text: root.title
                        color: heading.hovered || heading.activeFocus || headerNav.current ? root.theme.accent : (root.moduleEnabled ? root.theme.ink : root.theme.muted)
                        font: root.theme.moduleHeadingFont; wrapMode: Text.WordWrap
                    }
                    Text {
                        visible: !!root.module.deprecated
                        text: "deprecated"
                        color: root.theme.muted; font: root.theme.textFont
                    }
                }
                background: Rectangle { color: "transparent"; border.color: heading.activeFocus || headerNav.current ? root.theme.accent : "transparent" }
            }
            InstanceButton {
                id: instanceButton
                objectName: "module-instances-" + root.module.operation + (root.instance ? "-" + root.instance : "")
                visible: !!root.moduleState && root.term === ""
                theme: root.theme
                moduleState: root.moduleState
                title: root.title
                editable: root.editable
                navGroup: root.module.operation + "/" + root.instance
                onInstanceRequested: (action, name) => root.requestInstance(action, name)
            }
        }
        TapHandler {
            acceptedButtons: Qt.RightButton
            onTapped: (eventPoint, button) => menu.get().popup(eventPoint.position.x, eventPoint.position.y)
        }
        // Built on first use, not with every module.
        OnDemand {
            id: menu
            parent: moduleHeader
            Menu {
                MenuItem {
                    text: (root.moduleEnabled ? "Disable " : "Enable ") + root.title
                    enabled: root.editable && !!root.moduleState
                    onTriggered: root.requestEnabled(!root.moduleEnabled)
                }
                MenuItem {
                    text: "Reset " + root.title
                    enabled: root.editable && !!root.moduleState
                    onTriggered: root.resetRequested()
                }
                MenuSeparator {}
                MenuItem {
                    text: "new instance"
                    enabled: root.editable && !!root.moduleState && root.moduleState.canNew
                    onTriggered: root.requestInstance("new", "")
                }
                MenuItem {
                    text: "duplicate instance"
                    enabled: root.editable && !!root.moduleState && root.moduleState.canNew
                    onTriggered: root.requestInstance("duplicate", "")
                }
            }
        }
        DisclosureButton {
            objectName: "module-details-" + root.module.operation
            visible: rows.hasDetails && root.term === ""
            anchors.right: parent.right
            anchors.verticalCenter: headerContent.verticalCenter
            theme: root.theme
            expanded: root.expanded
            onClicked: root.expansionRequested()
            Accessible.name: (root.expanded ? "Hide details for " : "Details for ") + root.title
        }
    }
    GeneratedRows {
        id: rows
        width: parent.width
        theme: root.theme
        module: root.module
        moduleState: root.moduleState
        catalogModel: root.catalogModel
        instance: root.instance
        overrides: root.overrides
        overridePrefix: root.module.operation + "/" + root.instance + "/"
        editable: root.editable && !!root.moduleState
        expanded: root.expanded
        moreOpen: root.moreOpen
        term: root.term
        nameMatched: root.nameMatched
        activeControl: root.activeControl
        onChangesRequested: changes => root.changesRequested(changes)
        onInteractionChanged: active => root.interactionChanged(active)
        onControlSelected: id => root.controlSelected(id)
        onMoreRequested: root.moreRequested()
    }
    // The blend section of this instance, under the expanded module (blend_gui.c).
    Loader {
        id: blendLoader
        width: parent.width
        // Built once the expansion and the search term have both settled (see GeneratedRows).
        readonly property bool wanted: root.expanded && root.term === "" && !!root.moduleState && !!root.moduleState.blend
        active: false
        visible: active
        Component.onCompleted: active = wanted
        onWantedChanged: if (wanted) Qt.callLater(blendLoader.settle); else active = false
        function settle() { active = wanted }
        sourceComponent: BlendSection {
            theme: root.theme
            moduleState: root.moduleState
            catalogModel: root.catalogModel
            overrides: root.overrides
            overridePrefix: root.module.operation + "/" + root.instance + "/"
            editable: root.editable && !!root.moduleState
            moduleEnabled: root.moduleEnabled
            navGroup: root.module.operation + "/" + root.instance
            onChangesRequested: changes => root.changesRequested(changes)
            onInteractionChanged: active => root.interactionChanged(active)
            onDrawnShapeRequested: shape => root.changesRequested({ "@drawn": shape })
        }
    }
    InstanceFooter {
        visible: root.expanded && root.term === "" && !!root.moduleState
        width: parent.width - 28
        theme: root.theme
        button: instanceButton
        navGroup: root.module.operation + "/" + root.instance
        active: root.editable
    }
}
