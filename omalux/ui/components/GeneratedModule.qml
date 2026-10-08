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

    readonly property bool moduleEnabled: !!moduleState && moduleState.enabled
    readonly property bool nameMatched: term !== "" && catalogModel.moduleNameMatches(module, term)
    readonly property string title: module.name + (instance > 0 ? " " + (instance + 1) : "")
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
            onToggleGroup: root.enableRequested(!root.moduleEnabled)
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
                onClicked: { headerNav.claim(); root.enableRequested(!root.moduleEnabled) }
                Accessible.name: "Enable " + root.title
                Accessible.checkable: true; Accessible.checked: root.moduleEnabled
                ToolTip.visible: hovered && !!root.module.purpose
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
        }
        TapHandler {
            acceptedButtons: Qt.RightButton
            onTapped: (eventPoint, button) => menu.popup(eventPoint.position.x, eventPoint.position.y)
        }
        Menu {
            id: menu
            MenuItem {
                text: (root.moduleEnabled ? "Disable " : "Enable ") + root.title
                enabled: root.editable && !!root.moduleState
                onTriggered: root.enableRequested(!root.moduleEnabled)
            }
            MenuItem {
                text: "Reset " + root.title
                enabled: root.editable && !!root.moduleState
                onTriggered: root.resetRequested()
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
}
