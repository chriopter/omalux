import QtQuick
import QtQuick.Controls
import "../components"

// A sidebar pane listing darktable modules by group, built from the generated layout. The
// Tone, Color, Detail and Effects panes are this pane with their own groups.
SidebarScrollView {
    id: root
    required property var theme
    required property var catalogModel
    required property var states
    required property string tab
    property var overrides: ({})
    property bool editable: true
    property string term: ""
    property string activeControl: ""
    signal changesRequested(string operation, int instance, var changes)
    signal enableRequested(string operation, int instance, bool enabled)
    signal resetRequested(string operation, int instance, var module)
    signal interactionChanged(bool active)
    signal controlSelected(string id)

    readonly property var groups: catalogModel.groupsForTab(tab)

    Column {
        width: root.availableWidth
        padding: 18
        spacing: 0
        ModuleList {
            id: list
            width: parent.width - 36
            theme: root.theme
            groups: root.groups
            catalogModel: root.catalogModel
            states: root.states
            overrides: root.overrides
            editable: root.editable
            // Only the pane on screen opens its matches; the others follow when shown.
            term: root.visible ? root.term : ""
            activeControl: root.activeControl
            settingsKey: root.tab
            onChangesRequested: (operation, instance, changes) => root.changesRequested(operation, instance, changes)
            onEnableRequested: (operation, instance, enabled) => root.enableRequested(operation, instance, enabled)
            onResetRequested: (operation, instance, module) => root.resetRequested(operation, instance, module)
            onInteractionChanged: active => root.interactionChanged(active)
            onControlSelected: id => root.controlSelected(id)
        }
        Text {
            visible: list.term !== "" && list.matchCount === 0
            width: parent.width - 36
            topPadding: 12
            text: "No module here matches “" + root.term + "”."
            color: root.theme.muted; font: root.theme.textFont; wrapMode: Text.WordWrap
        }
    }
}
