import QtQuick
import QtQuick.Controls
import QtCore
import "../components"

// A module pane built like the Filters pane: on top a short summary of the controls a
// photographer reaches for first (layout.json "panes", ModuleCatalog.summaryFor), each row with
// its module icon, a plain label, the value and the chevron that opens the whole module in
// place; below, "Advanced" lists every other module of the pane as a compact card. The Tone,
// Color, Detail and Effects panes are this pane with their own tab. Modules curated in the
// Filters pane live there only and are not shown here. While searching, the summary steps aside
// and every matching module of the pane opens, as before.
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

    readonly property string shownTerm: visible ? term : ""
    // Rebuilt only when the layout or the active tone mapper changes: rebuilding on every edit
    // would recreate the row being dragged.
    readonly property string summaryState: { root.states; return JSON.stringify(Object.keys(root.catalogModel.panes)) + root.catalogModel.pickState(root.tab) }
    property var summary: []
    onSummaryStateChanged: summary = catalogModel.summaryFor(tab)
    // The modules the summary stands for are not repeated under Advanced.
    readonly property var summaryModules: summary.map(b => b.module.operation)
    readonly property var groups: shownTerm !== "" ? catalogModel.groupsForTab(tab)
                                                   : catalogModel.advancedFor(tab, summaryModules)

    property var expanded: ({})
    property bool restoring: true
    Settings {
        id: preferences
        category: "PaneSummary-" + root.tab
        property string expanded: "{}"
    }
    Component.onCompleted: {
        try { expanded = JSON.parse(preferences.expanded) } catch (e) {}
        restoring = false
        summary = catalogModel.summaryFor(tab)
    }
    onExpandedChanged: if (!restoring) preferences.expanded = JSON.stringify(expanded)
    function toggle(key) {
        const next = Object.assign({}, expanded); next[key] = !next[key]; expanded = next
    }

    Column {
        width: root.availableWidth
        padding: 18
        spacing: 0
        Column {
            id: summaryColumn
            objectName: "pane-summary-" + root.tab
            visible: root.shownTerm === ""
            width: parent.width - 36
            spacing: 8
            Repeater {
                model: summaryColumn.visible ? root.summary : []
                delegate: Column {
                    id: entry
                    required property var modelData
                    readonly property string operation: modelData.module.operation
                    readonly property bool open: !!root.expanded[operation]
                    readonly property var instanceList: root.catalogModel.instances[operation] || [0]
                    width: summaryColumn.width
                    spacing: 8
                    OpenScroll { target: entry; open: entry.open }
                    // The summary rows stay where they are when the module unfolds; it grows
                    // downward from them, inside one outlined block.
                    Column {
                        id: block
                        width: entry.width
                        spacing: 4
                        bottomPadding: entry.open ? 10 : 0
                        Item {
                            width: block.width
                            implicitHeight: rowsBlock.implicitHeight
                            Rectangle {
                                objectName: "module-block-" + entry.operation
                                visible: entry.open
                                z: -1
                                x: -14; y: -7
                                width: parent.width + 14
                                height: block.height + 7
                                color: root.theme.surface
                                radius: 6
                                border.width: 1
                                border.color: root.theme.line
                            }
                            GeneratedSummary {
                                id: rowsBlock
                                width: block.width
                                theme: root.theme
                                block: entry.modelData
                                moduleState: root.states[entry.operation + "/0"]
                                catalogModel: root.catalogModel
                                overrides: root.overrides
                                editable: root.editable
                                activeControl: root.activeControl
                                expanded: entry.open
                                onExpansionRequested: root.toggle(entry.operation)
                                onChangesRequested: changes => root.changesRequested(entry.operation, 0, changes)
                                onEnableRequested: on => root.enableRequested(entry.operation, 0, on)
                                onResetRequested: root.resetRequested(entry.operation, 0, entry.modelData.module)
                                onInteractionChanged: active => root.interactionChanged(active)
                                onControlSelected: id => root.controlSelected(id)
                            }
                        }
                        // The whole module beneath its rows, built when it is unfolded.
                        Loader {
                            width: block.width
                            active: entry.open
                            visible: active
                            sourceComponent: GeneratedModule {
                                objectName: "generated-module-" + entry.operation
                                width: block.width
                                theme: root.theme
                                module: root.catalogModel.moduleForInstance(entry.modelData.module, 0)
                                instance: 0
                                moduleState: root.states[entry.operation + "/0"]
                                catalogModel: root.catalogModel
                                overrides: root.overrides
                                editable: root.editable
                                compact: true
                                attached: true
                                linkedPaths: entry.modelData.rows.map(e => e.row.path)
                                expanded: true
                                moreOpen: !!root.expanded[entry.operation + "-more"]
                                activeControl: root.activeControl
                                onExpansionRequested: root.toggle(entry.operation)
                                onMoreRequested: root.toggle(entry.operation + "-more")
                                onChangesRequested: changes => root.changesRequested(entry.operation, 0, changes)
                                onEnableRequested: on => root.enableRequested(entry.operation, 0, on)
                                onResetRequested: root.resetRequested(entry.operation, 0, entry.modelData.module)
                                onInteractionChanged: active => root.interactionChanged(active)
                                onControlSelected: id => root.controlSelected(id)
                            }
                        }
                    }
                    // Further instances, as cards of their own that open on their chevron.
                    Repeater {
                        model: entry.open ? entry.instanceList.filter(i => i > 0) : []
                        delegate: GeneratedModule {
                            required property int modelData
                            objectName: "generated-module-" + entry.operation + "-" + modelData
                            width: entry.width
                            theme: root.theme
                            module: root.catalogModel.moduleForInstance(entry.modelData.module, modelData)
                            instance: modelData
                            moduleState: root.states[entry.operation + "/" + modelData]
                            catalogModel: root.catalogModel
                            overrides: root.overrides
                            editable: root.editable
                            compact: true
                            expanded: !!root.expanded[entry.operation + "/" + modelData]
                            moreOpen: !!root.expanded[entry.operation + "-more"]
                            activeControl: root.activeControl
                            onExpansionRequested: root.toggle(entry.operation + "/" + modelData)
                            onMoreRequested: root.toggle(entry.operation + "-more")
                            onChangesRequested: changes => root.changesRequested(entry.operation, modelData, changes)
                            onEnableRequested: on => root.enableRequested(entry.operation, modelData, on)
                            onResetRequested: root.resetRequested(entry.operation, modelData, entry.modelData.module)
                            onInteractionChanged: active => root.interactionChanged(active)
                            onControlSelected: id => root.controlSelected(id)
                        }
                    }
                }
            }
        }
        ModuleList {
            id: list
            width: parent.width - 36
            theme: root.theme
            groups: root.groups
            compact: true
            catalogModel: root.catalogModel
            states: root.states
            overrides: root.overrides
            editable: root.editable
            // Only the pane on screen opens its matches; the others follow when shown.
            term: root.shownTerm
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
