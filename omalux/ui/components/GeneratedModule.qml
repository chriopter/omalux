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
    // A compact card (the Advanced list of a pane): the module icon and name only until it is
    // opened, like the Advanced cards of the Filters pane.
    property bool compact: false
    // Unfolded beneath the summary rows of a pane, which stay as its main rows
    // (ModuleGroupsPanel): no card heading, a strip with the module's name, reset and instances
    // instead, and every parameter as an indented sub-row. `linkedPaths` are the
    // parameters those main rows drive.
    property bool attached: false
    property var linkedPaths: []
    readonly property bool cardOnly: compact && !expanded && term === ""
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
    // The heading keeps its place when the card opens or closes (a second click hits it again).
    topPadding: attached || compact ? 0 : 12
    bottomPadding: attached ? 0 : cardOnly ? 4 : expanded ? 10 : 8

    OpenScroll { target: root; open: root.expanded; active: root.term === "" && !root.attached }
    ModuleHeader {
        id: moduleHeader
        visible: !root.attached
        theme: root.theme
        operation: root.module.operation
        name: root.module.name
        instanceLabel: root.instanceLabel
        purpose: root.module.purpose || ""
        deprecated: !!root.module.deprecated
        moduleState: root.moduleState
        moduleEnabled: root.moduleEnabled
        ready: root.editable && !!root.moduleState
        expanded: root.expanded && root.term === ""
        hasDetails: rows.hasDetails && root.term === ""
        showInstances: root.term === ""
        instanceCount: root.instance === 0 ? (root.catalogModel.instances[root.module.operation] || []).length : 1
        blockHeight: root.height - moduleHeader.y
        instancesSuffix: root.module.operation + (root.instance ? "-" + root.instance : "")
        navId: "module:" + root.module.operation + "/" + root.instance
        navGroup: root.module.operation + "/" + root.instance
        onToggleRequested: root.requestEnabled(!root.moduleEnabled)
        onExpansionRequested: root.expansionRequested()
        onResetRequested: root.resetRequested()
        onInstanceRequested: (action, name) => root.requestInstance(action, name)
    }
    ModuleBody {
        id: body
        spacing: 4
        animate: root.term === ""
        ModuleStrip {
            id: strip
            visible: root.attached
            width: parent.width
            theme: root.theme
            operation: root.module.operation
            name: root.module.name
            instanceLabel: root.instanceLabel
            moduleState: root.moduleState
            ready: root.editable && !!root.moduleState
            showInstances: root.term === ""
            instanceCount: (root.catalogModel.instances[root.module.operation] || []).length
            navGroup: root.module.operation + "/" + root.instance
            onResetRequested: root.resetRequested()
            onInstanceRequested: (action, name) => root.requestInstance(action, name)
        }
        Item {
            width: parent.width
            implicitHeight: rows.implicitHeight
                GeneratedRows {
                    id: rows
                    x: root.attached ? 16 : 0
                width: parent.width - x
                sub: root.attached
                linkedPaths: root.linkedPaths
                indent: x
                    theme: root.theme
                    module: root.module
                    moduleState: root.moduleState
                    catalogModel: root.catalogModel
                    instance: root.instance
                    moduleEnabled: root.moduleEnabled
                    overrides: root.overrides
                    overridePrefix: root.module.operation + "/" + root.instance + "/"
                    editable: root.editable && !!root.moduleState
                    expanded: root.expanded
                    collapsedRows: !root.compact
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
    }
}
