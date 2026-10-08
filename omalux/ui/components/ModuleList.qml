import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import QtCore

// darktable modules under collapsible group headings (darktable's group names). Each module
// is a GeneratedModule; extra instances follow their base instance. Open groups, expanded
// modules and "more" are remembered per list. While a search term is set, groups and
// matching modules open by themselves and everything else is hidden; the stored state is
// left untouched.
Column {
    id: root
    required property var theme
    required property var groups            // ModuleCatalog.groupsForTab(...)
    required property var catalogModel
    required property var states            // catalogModel.states, passed so bindings follow it
    property var overrides: ({})
    property bool editable: true
    property string term: ""
    property string activeControl: ""
    property string settingsKey: "default"
    // A list under another pane's content (Styles, Crop) starts with a caption instead of a gap.
    property string caption: ""
    signal changesRequested(string operation, int instance, var changes)
    signal enableRequested(string operation, int instance, bool enabled)
    signal resetRequested(string operation, int instance, var module)
    signal interactionChanged(bool active)
    signal controlSelected(string id)

    property var openGroups: ({})
    property var expandedModules: ({})
    property var moreModules: ({})
    property bool restoring: true
    Settings {
        id: preferences
        category: "ModuleGroups-" + root.settingsKey
        property string groups: "{}"
        property string modules: "{}"
        property string more: "{}"
    }
    Component.onCompleted: {
        try { openGroups = JSON.parse(preferences.groups) } catch (e) {}
        try { expandedModules = JSON.parse(preferences.modules) } catch (e) {}
        try { moreModules = JSON.parse(preferences.more) } catch (e) {}
        restoring = false
    }
    onOpenGroupsChanged: if (!restoring) preferences.groups = JSON.stringify(openGroups)
    onExpandedModulesChanged: if (!restoring) preferences.modules = JSON.stringify(expandedModules)
    onMoreModulesChanged: if (!restoring) preferences.more = JSON.stringify(moreModules)
    function toggle(property, key, fallback) {
        const next = Object.assign({}, root[property])
        next[key] = !(key in next ? next[key] : fallback)
        root[property] = next
    }
    function groupOpen(g) { return term !== "" || caption !== "" || (g.id in openGroups ? openGroups[g.id] : !g.quiet) }
    function expand(operation) {
        const next = Object.assign({}, expandedModules); next[operation] = true; expandedModules = next
    }
    function visibleCount(g) {
        let n = 0
        for (const m of g.modules) if (catalogModel.moduleShown(m) && catalogModel.moduleMatches(m, term)) ++n
        return n
    }
    readonly property int matchCount: {
        states; term
        let n = 0
        for (const g of groups) n += visibleCount(g)
        return n
    }
    spacing: 4

    Text {
        visible: root.caption !== "" && root.matchCount > 0
        text: root.caption
        color: root.theme.ink; font.bold: true; font.letterSpacing: 2
        topPadding: 14; bottomPadding: 2
    }
    Repeater {
        model: root.groups
        delegate: Column {
            id: group
            required property var modelData
            readonly property int count: { root.states; root.term; return root.visibleCount(modelData) }
            readonly property bool open: root.groupOpen(modelData)
            width: root.width
            visible: count > 0
            spacing: 8
            ToolButton {
                id: groupHeading
                objectName: "module-group-" + group.modelData.id
                // Under a caption (Styles, Crop) the few modules read as one flat list.
                visible: root.caption === ""
                width: parent.width
                padding: 0
                topPadding: 12; bottomPadding: 2
                hoverEnabled: true
                onClicked: root.toggle("openGroups", group.modelData.id, !group.modelData.quiet)
                Accessible.name: group.modelData.label
                Accessible.description: group.open ? "Collapse group" : "Expand group"
                contentItem: RowLayout {
                    spacing: 6
                    Text {
                        Layout.fillWidth: true
                        text: group.modelData.label
                        color: groupHeading.hovered || groupHeading.visualFocus ? root.theme.accent : root.theme.muted
                        opacity: group.modelData.quiet && !groupHeading.hovered ? .75 : 1
                        font: root.theme.settingsFont
                    }
                    Text {
                        text: group.count
                        color: root.theme.muted; font: root.theme.textFont
                        opacity: .75
                    }
                    Canvas {
                        Layout.preferredWidth: 24; Layout.preferredHeight: 14
                        rotation: group.open ? 90 : 0
                        property color stroke: groupHeading.hovered ? root.theme.accent : root.theme.muted
                        onStrokeChanged: requestPaint()
                        onPaint: {
                            const c = getContext("2d")
                            c.clearRect(0, 0, width, height)
                            c.strokeStyle = stroke; c.lineWidth = 1.4
                            c.beginPath(); c.moveTo(10, 2); c.lineTo(14, 7); c.lineTo(10, 12); c.stroke()
                        }
                    }
                }
                background: Rectangle { color: "transparent"; border.color: groupHeading.visualFocus ? root.theme.accent : "transparent" }
            }
            Loader {
                width: parent.width
                active: group.open
                visible: group.open
                sourceComponent: Column {
                    width: group.width
                    spacing: 8
                    Repeater {
                        model: group.modelData.modules
                        delegate: Column {
                            id: moduleEntry
                            required property var modelData
                            readonly property var instanceList: (root.catalogModel.instances[modelData.operation] || [0])
                            width: group.width
                            spacing: 8
                            visible: { root.states; return root.catalogModel.moduleShown(modelData) && root.catalogModel.moduleMatches(modelData, root.term) }
                            Repeater {
                                model: moduleEntry.instanceList
                                delegate: GeneratedModule {
                                    required property int modelData
                                    readonly property string key: moduleEntry.modelData.operation + "/" + modelData
                                    objectName: "generated-module-" + moduleEntry.modelData.operation + (modelData ? "-" + modelData : "")
                                    width: group.width
                                    theme: root.theme
                                    module: moduleEntry.modelData
                                    instance: modelData
                                    moduleState: root.states[key]
                                    catalogModel: root.catalogModel
                                    overrides: root.overrides
                                    editable: root.editable
                                    term: root.term
                                    expanded: root.term !== "" || !!root.expandedModules[moduleEntry.modelData.operation]
                                    moreOpen: !!root.moreModules[moduleEntry.modelData.operation]
                                    activeControl: root.activeControl
                                    onExpansionRequested: root.toggle("expandedModules", moduleEntry.modelData.operation, false)
                                    onMoreRequested: root.toggle("moreModules", moduleEntry.modelData.operation, false)
                                    onChangesRequested: changes => root.changesRequested(moduleEntry.modelData.operation, modelData, changes)
                                    onEnableRequested: on => root.enableRequested(moduleEntry.modelData.operation, modelData, on)
                                    onResetRequested: root.resetRequested(moduleEntry.modelData.operation, modelData, moduleEntry.modelData)
                                    onInteractionChanged: active => root.interactionChanged(active)
                                    onControlSelected: id => root.controlSelected(id)
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
