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
    // id => the image's own default of a registered control (what its reset sets), or null.
    property var controlDefault: null
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
    // A module with one main row keeps that row when it is unfolded: same place, name, value and
    // track; only its chevron turns. The block grows downward from it: a strip with the module's
    // name, reset and instances, then every parameter as a sub-row. Modules without a main row
    // (the Advanced cards) have the card heading (ModuleHeader).
    readonly property bool hasMain: section.primary.length === 1
    readonly property var mainControl: hasMain ? section.controls.find(c => c.id === section.primary[0]) || null : null
    // The parameters that a main row of the pane drives: their sub-rows are marked as linked.
    property var linkedIds: section.primary
    // A shortcut row turns its chevron with the block of its module.
    property bool detailsOpen: expanded
    topPadding: hasMain ? 0 : 12
    bottomPadding: expanded ? 10 : hasMain ? 0 : 8

    function findControl(id) {
        if (mainControl && mainControl.id === id) return mainRow
        for (let i = 0; i < rows.count; ++i) {
            const item = rows.itemAt(i)
            if (item.control.id === id && item.visible) return item
        }
        return null
    }
    function resetModule() { for (const c of section.controls) controlReset(c.id) }
    OpenScroll { target: root; open: root.expanded; active: root.term === "" }
    Item {
        id: mainHolder
        visible: root.hasMain
        width: parent.width
        implicitHeight: mainRow.implicitHeight
        // The block around the kept row and what unfolds beneath it.
        Rectangle {
            objectName: "module-block-" + root.section.key
            visible: root.expanded
            z: -1
            x: -14; y: -7
            width: parent.width + 14
            height: (instancesBox.visible ? instancesBox.y : root.height) - mainHolder.y + 7
            color: root.theme.surface
            radius: 6
            border.width: 1
            border.color: root.theme.line
        }
        ControlSlider {
            id: mainRow
            objectName: "filter-control-" + (root.mainControl ? root.mainControl.id : "")
            anchors.fill: parent
            theme: root.theme
            control: root.mainControl || ({ id: "", label: "", unit: "", decimals: 0, step: 1, softMinimum: 0, softMaximum: 1, section: "", module: "" })
            value: root.mainControl ? root.values[root.mainControl.id] : 0
            opacity: moduleEnabled ? 1 : .7
            compact: true
            moduleToggleAvailable: true
            moduleIconKey: root.section.module
            qualifyLabel: true
            displayLabel: control.id === "vibrance" ? "vibrance" : root.section.shortTitle ? root.section.name : ""
            moduleName: control.section
            moduleEnabled: root.moduleEnabled
            onModuleToggleRequested: root.controlEdited(root.enableControl, moduleEnabled ? 0 : 1)
            editable: root.editable; selected: root.activeControl === control.id
            detailsAvailable: root.hasDetails
            detailsExpanded: root.detailsOpen
            onDetailsRequested: root.expansionRequested()
            onSelectedRequested: root.controlSelected(control.id)
            onInteractionChanged: active => root.interactionChanged(active)
            onEdited: value => root.controlEdited(control.id, value)
            onResetRequested: root.controlReset(control.id)
            defaultValue: hot && root.controlDefault && root.mainControl ? root.controlDefault(control.id) : undefined
            navTarget.group: root.section.key
            onActivated: if (root.hasDetails) root.expansionRequested()
            onModuleResetRequested: root.resetModule()
        }
    }
    ModuleHeader {
        id: moduleHeader
        visible: !root.hasMain
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
        instanceCount: root.catalogModel && !root.section.shortcut ? (root.catalogModel.instances[root.section.module] || []).length : 1
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
        // A folded module with a main row is that row alone.
        visible: root.expanded || !root.hasMain
        spacing: 4
        animate: root.term === ""
        ModuleStrip {
            id: strip
            visible: root.hasMain && root.expanded
            width: parent.width
            theme: root.theme
            operation: root.section.module
            name: root.section.name
            instanceLabel: root.moduleState && root.moduleState.instanceLabel ? root.moduleState.instanceLabel : ""
            moduleState: root.moduleState
            ready: root.editable
            showInstances: root.term === ""
            instanceCount: root.catalogModel && !root.section.shortcut ? (root.catalogModel.instances[root.section.module] || []).length : 1
            navGroup: root.section.key
            onResetRequested: root.resetModule()
            onInstanceRequested: (action, name) => root.parameterChangesRequested({ "@instance": action, "@name": name })
        }
        Item {
            id: subRows
            visible: root.expanded
            width: parent.width
            implicitHeight: subColumn.implicitHeight
            // The guide line: the rows beside it are parts of the main row above.
            Rectangle {
                visible: root.hasMain
                x: 3; y: 0
                width: 1; height: parent.height - 6
                color: root.theme.line
            }
            Column {
                id: subColumn
                x: root.hasMain ? 16 : 0
                width: parent.width - x
                spacing: 4
                Repeater {
                    id: rows
                    model: root.section.controls
                    delegate: ControlSlider {
                        required property var modelData
                        objectName: "filter-sub-" + modelData.id
                        width: parent.width
                        opacity: moduleEnabled ? 1 : .7
                        compact: true
                        sub: root.hasMain
                        linked: root.hasMain && root.linkedIds.includes(modelData.id)
                        moduleToggleAvailable: false
                        qualifyLabel: false
                        displayLabel: modelData.id === "vibrance" ? "vibrance"
                            : modelData.section.includes(" · ")
                              ? modelData.section.split(" · ").slice(1).join(" · ") + " · " + modelData.label : ""
                        moduleName: modelData.section
                        moduleEnabled: root.values[modelData.module + "_enabled"] > .5
                        onModuleToggleRequested: root.controlEdited(modelData.module + "_enabled", moduleEnabled ? 0 : 1)
                        theme: root.theme; control: modelData; value: root.values[modelData.id]
                        editable: root.editable; selected: root.activeControl === modelData.id
                        onSelectedRequested: root.controlSelected(modelData.id)
                        onInteractionChanged: active => root.interactionChanged(active)
                        onEdited: value => root.controlEdited(modelData.id, value)
                        onResetRequested: root.controlReset(modelData.id)
                        defaultValue: hot && root.controlDefault ? root.controlDefault(modelData.id) : undefined
                        navTarget.group: root.section.key
                        onActivated: if (root.hasDetails) root.expansionRequested()
                        onModuleResetRequested: root.resetModule()
                        onRevealRequested: if (!root.expanded && root.hasDetails) root.expansionRequested()
                    }
                }
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
