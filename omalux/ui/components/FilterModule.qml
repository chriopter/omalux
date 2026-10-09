import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

Column {
    id: root
    required property var theme
    required property var section
    required property var values
    required property bool editable
    required property string activeControl
    required property bool expanded
    // Parameters of the same darktable module that the registry does not cover, from the
    // generated layout; shown under a quiet "more" inside the expanded block.
    property var extraModule: null
    property var moduleState: undefined
    property var catalogModel: null
    property var overrides: ({})
    property bool moreOpen: false
    property string term: ""
    signal moreRequested()
    signal parameterChangesRequested(var changes)
    // Edits and actions of further instances of this module (darktable's multi-instance).
    signal instanceChangesRequested(int instance, var changes)
    readonly property var extraInstances: !!catalogModel && !section.shortcut
                                          ? (catalogModel.instances[section.module] || []).filter(i => i > 0) : []
    property var instanceExpanded: ({})
    property var instanceMore: ({})
    function toggleInstance(property, instance) {
        const next = Object.assign({}, root[property]); next[instance] = !next[instance]; root[property] = next
    }
    readonly property bool hasExtra: !!extraModule && !!catalogModel && extraModule.rows.length > 0 && !section.shortcut
    signal interactionChanged(bool active)
    signal expansionRequested()
    signal controlSelected(string id)
    signal controlEdited(string id, real value)
    signal controlReset(string id)
    signal halationRequested()
    readonly property string enableControl: section.controls[0].module + "_enabled"
    readonly property bool moduleEnabled: values[enableControl] > .5
    // Shortcut rows open their parent module block, so they disclose like any other row.
    readonly property bool hasDetails: !!section.shortcut || section.controls.some(c => !section.primary.includes(c.id))
                                       || section.name === "denoise (profiled)" || section.name === "diffuse or sharpen" || hasExtra
    spacing: 4
    // An open single-row module keeps its name where the collapsed row had it: the heading
    // moves up into the gap between the rows, and the rows above stay where they were.
    topPadding: !headerVisible ? 0 : section.primary.length !== 1 ? 12 : -4
    bottomPadding: headerVisible ? (expanded ? 10 : 8) : 0

    function findControl(id) {
        for (let i = 0; i < rows.count; ++i) {
            const item = rows.itemAt(i)
            if (item.control.id === id && item.visible) return item
        }
        return null
    }
    readonly property bool headerVisible: expanded || section.primary.length !== 1
    function resetModule() { for (const c of section.controls) controlReset(c.id) }
    OpenScroll { target: root; open: root.expanded; active: root.term === "" }
    ModuleHeader {
        id: moduleHeader
        visible: root.headerVisible
        theme: root.theme
        operation: root.section.module
        name: root.section.name
        instanceLabel: !root.section.shortcut && root.moduleState && root.moduleState.instanceLabel ? root.moduleState.instanceLabel : ""
        moduleState: root.moduleState
        moduleEnabled: root.moduleEnabled
        ready: root.editable
        expanded: root.expanded
        hasDetails: root.hasDetails
        showInstances: !root.section.shortcut && root.term === ""
        blockHeight: (instancesBox.visible ? instancesBox.y : root.height) - moduleHeader.y
        navId: "module:" + root.section.key
        navGroup: root.section.key
        onToggleRequested: root.controlEdited(root.enableControl, root.moduleEnabled ? 0 : 1)
        onExpansionRequested: root.expansionRequested()
        onResetRequested: root.resetModule()
        onInstanceRequested: (action, name) => root.parameterChangesRequested({ "@instance": action, "@name": name })
    }
    ModuleBody {
        id: body
        spacing: 4
        animate: root.term === ""
        Repeater {
            id: rows
            model: root.section.controls
            delegate: ControlSlider {
                required property var modelData
                objectName: "filter-control-" + modelData.id + (root.section.module === "colisa" && !root.section.shortcut && modelData.id !== "contrast" ? "-module" : "")
                readonly property bool secondary: !root.section.primary.includes(modelData.id)
                x: 0
                width: parent.width - x
                opacity: moduleEnabled ? 1 : .7
                visible: root.expanded || root.section.primary.includes(modelData.id)
                compact: true
                moduleToggleAvailable: !root.headerVisible
                moduleIconKey: !root.headerVisible ? root.section.module : ""
                qualifyLabel: !root.headerVisible
                displayLabel: modelData.id === "vibrance" ? "vibrance"
                    : !root.headerVisible && root.section.shortTitle ? root.section.name
                    : root.headerVisible && modelData.section.includes(" · ")
                      ? modelData.section.split(" · ").slice(1).join(" · ") + " · " + modelData.label : ""
                moduleName: modelData.section
                moduleEnabled: root.values[modelData.module + "_enabled"] > .5
                onModuleToggleRequested: root.controlEdited(modelData.module + "_enabled", moduleEnabled ? 0 : 1)
                theme: root.theme; control: modelData; value: root.values[modelData.id]
                editable: root.editable; selected: root.activeControl === modelData.id
                detailsAvailable: !root.headerVisible && root.hasDetails && root.section.primary[0] === modelData.id
                detailsExpanded: root.expanded
                onDetailsRequested: root.expansionRequested()
                onSelectedRequested: root.controlSelected(modelData.id)
                onInteractionChanged: active => root.interactionChanged(active)
                onEdited: value => root.controlEdited(modelData.id, value)
                onResetRequested: root.controlReset(modelData.id)
                navTarget.group: root.section.key
                onActivated: if (root.hasDetails) root.expansionRequested()
                onModuleResetRequested: root.resetModule()
                onRevealRequested: if (!root.expanded && root.hasDetails) root.expansionRequested()
            }
        }
        Button {
            id: halationButton
            visible: root.expanded && root.section.name === "diffuse or sharpen"
            text: "Halation recipe (experimental)"
            enabled: root.editable
            onClicked: root.halationRequested()
            highlighted: halationNav.current
            NavTarget { id: halationNav; navId: "halation"; label: "halation recipe"; group: root.section.key; enabled: halationButton.enabled; onActivate: root.halationRequested() }
        }
        // Built only where it is shown (denoise, expanded): it repaints on every value change.
        Loader {
            active: root.expanded && root.section.name === "denoise (profiled)"
            visible: active
            // Aligned with the slider rows above it, clear of the disclosure column on the right.
            width: parent.width - 28
            sourceComponent: DenoiseCurve {
                opacity: root.moduleEnabled ? 1 : .7
                theme: root.theme; values: root.values; editable: root.editable
                group: root.section.key
                onEdited: (id, value) => root.controlEdited(id, value)
            }
        }
        SectionRow {
            objectName: "module-more-" + root.section.module
            visible: root.expanded && root.hasExtra && root.term === ""
            width: parent.width
            theme: root.theme
            label: "more"
            open: root.moreOpen
            navTarget.navId: root.section.key + "/@more"
            navTarget.group: root.section.key
            onRequested: root.moreRequested()
            Accessible.name: (root.moreOpen ? "Fewer settings for " : "More settings for ") + root.section.name
        }
        Loader {
            width: parent.width
            active: root.hasExtra && root.expanded && (root.moreOpen || root.term !== "")
            visible: active
            sourceComponent: GeneratedRows {
                theme: root.theme
                module: root.extraModule
                moduleState: root.moduleState
                catalogModel: root.catalogModel
                overrides: root.overrides
                overridePrefix: root.extraModule.operation + "/0/"
                editable: root.editable && !!root.moduleState
                expanded: true
                moreOpen: true
                extraMode: true
                navGroup: root.section.key
                term: root.term
                activeControl: root.activeControl
                onChangesRequested: changes => root.parameterChangesRequested(changes)
                onInteractionChanged: active => root.interactionChanged(active)
                onControlSelected: id => root.controlSelected(id)
            }
        }
        // The blend section of the curated module's first instance (blend_gui.c).
        Loader {
            id: blendLoader
            width: parent.width
            // Built once the expansion and the search term have both settled (see GeneratedRows).
            readonly property bool wanted: root.expanded && !root.section.shortcut && root.term === "" && !!root.moduleState && !!root.moduleState.blend
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
                overridePrefix: root.section.module + "/0/"
                editable: root.editable
                moduleEnabled: root.moduleEnabled
                navGroup: root.section.key
                onChangesRequested: changes => root.parameterChangesRequested(changes)
                onInteractionChanged: active => root.interactionChanged(active)
                onDrawnShapeRequested: shape => root.parameterChangesRequested({ "@drawn": shape })
            }
        }
        InstanceFooter {
            visible: root.expanded && !root.section.shortcut && root.term === "" && !!root.moduleState
            width: parent.width
            theme: root.theme
            button: moduleHeader.instanceButton
            count: root.catalogModel ? (root.catalogModel.instances[root.section.module] || []).length : 0
            navGroup: root.section.key
            active: root.editable
        }
    }
    // Further instances are not edited through the curated controls: each is a generated
    // module with every row of its darktable module (ModuleCatalog.moduleForInstance).
    Column {
        id: instancesBox
        visible: root.extraInstances.length > 0 && !!root.extraModule && root.term === ""
        width: parent.width
        topPadding: 8
        spacing: 8
        Repeater {
            model: instancesBox.visible ? root.extraInstances : []
            delegate: GeneratedModule {
                required property int modelData
                objectName: "generated-module-" + root.section.module + "-" + modelData
                width: instancesBox.width
                theme: root.theme
                module: root.catalogModel.moduleForInstance(root.extraModule, modelData)
                instance: modelData
                moduleState: root.catalogModel.states[root.section.module + "/" + modelData]
                catalogModel: root.catalogModel
                overrides: root.overrides
                editable: root.editable
                expanded: !!root.instanceExpanded[modelData]
                moreOpen: !!root.instanceMore[modelData]
                activeControl: root.activeControl
                onExpansionRequested: root.toggleInstance("instanceExpanded", modelData)
                onMoreRequested: root.toggleInstance("instanceMore", modelData)
                onChangesRequested: changes => root.instanceChangesRequested(modelData, changes)
                onEnableRequested: on => root.instanceChangesRequested(modelData, { "@enabled": on ? 1 : 0 })
                onResetRequested: root.instanceChangesRequested(modelData, { "@reset": 1 })
                onInteractionChanged: active => root.interactionChanged(active)
                onControlSelected: id => root.controlSelected(id)
            }
        }
    }
}
