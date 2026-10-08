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
    topPadding: headerVisible && section.primary.length !== 1 ? 12 : 0
    bottomPadding: headerVisible ? 8 : 0

    function findControl(id) {
        for (let i = 0; i < rows.count; ++i) {
            const item = rows.itemAt(i)
            if (item.control.id === id && item.visible) return item
        }
        return null
    }
    readonly property bool headerVisible: expanded || section.primary.length !== 1
    function resetModule() { for (const c of section.controls) controlReset(c.id) }
    Item {
        id: moduleHeader
        visible: root.headerVisible
        x: -14
        width: parent.width + 14
        implicitHeight: Math.max(30, headerContent.implicitHeight + 10)
        // The heading is a keyboard stop of its own: Enter or ←/→ open and close the module.
        NavTarget {
            id: headerNav
            navId: "module:" + root.section.key
            label: root.section.name
            kind: "module"
            group: root.section.key
            enabled: root.editable
            groupActions: true
            adjustLabel: root.hasDetails ? "COLLAPSE/EXPAND" : ""
            activateLabel: root.hasDetails ? (root.expanded ? "COLLAPSE" : "EXPAND") : ""
            onActivate: if (root.hasDetails) root.expansionRequested()
            onAdjust: steps => { if (root.hasDetails && (steps > 0) !== root.expanded) root.expansionRequested() }
            onToggleGroup: root.controlEdited(root.enableControl, root.moduleEnabled ? 0 : 1)
            onResetGroup: root.resetModule()
        }
        // The background spans the module without participating in Column layout.
        Rectangle {
            z: -1
            width: parent.width
            height: (instancesBox.visible ? instancesBox.y : root.height) - moduleHeader.y
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
                objectName: "module-toggle-" + root.section.module
                Layout.fillWidth: true
                padding: 0
                enabled: root.editable
                onClicked: { headerNav.claim(); root.controlEdited(root.enableControl, root.moduleEnabled ? 0 : 1) }
                Accessible.name: "Enable " + root.section.name
                Accessible.checkable: true; Accessible.checked: root.moduleEnabled
                contentItem: RowLayout {
                    spacing: 7
                    Rectangle { opacity: root.moduleEnabled ? 1 : 0; width: 7; height: 7; radius: 3.5; color: root.theme.ink }
                    ModuleIcon {
                        moduleKey: root.section.module
                        opacity: root.moduleEnabled ? 1 : .6
                    }
                    Text {
                        Layout.fillWidth: true
                        text: root.section.name + (!root.section.shortcut && root.moduleState && root.moduleState.instanceLabel ? " • " + root.moduleState.instanceLabel : "")
                        color: heading.hovered || heading.activeFocus || headerNav.current ? root.theme.accent : (root.moduleEnabled ? root.theme.ink : root.theme.muted)
                        font: root.theme.moduleHeadingFont; wrapMode: Text.WordWrap
                    }
                }
                background: Rectangle { color: "transparent"; border.color: heading.activeFocus || headerNav.current ? root.theme.accent : "transparent" }
            }
            InstanceButton {
                id: instanceButton
                objectName: "module-instances-" + root.section.module
                visible: !root.section.shortcut && !!root.moduleState && root.term === ""
                theme: root.theme
                moduleState: root.moduleState
                title: root.section.name
                editable: root.editable
                navGroup: root.section.key
                onInstanceRequested: (action, name) => root.parameterChangesRequested({ "@instance": action, "@name": name })
            }
        }
        DisclosureButton {
            objectName: "module-details-" + root.section.module
            visible: root.hasDetails
            anchors.right: parent.right
            anchors.verticalCenter: headerContent.verticalCenter
            theme: root.theme
            expanded: root.expanded
            onClicked: root.expansionRequested()
            Accessible.name: (root.expanded ? "Hide details for " : "Details for ") + root.section.name
        }
    }
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
            darkVignette: modelData.id === "vignette" && !root.expanded
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
        x: 12; width: parent.width - 12
        sourceComponent: DenoiseCurve {
            opacity: root.moduleEnabled ? 1 : .45
            theme: root.theme; values: root.values; editable: root.editable
            group: root.section.key
            onEdited: (id, value) => root.controlEdited(id, value)
        }
    }
    Item {
        visible: root.expanded && root.hasExtra && root.term === ""
        width: parent.width - 28
        implicitHeight: 22
        ToolButton {
            id: moreButton
            objectName: "module-more-" + root.section.module
            padding: 0
            hoverEnabled: true
            onClicked: root.moreRequested()
            Accessible.name: (root.moreOpen ? "Fewer settings for " : "More settings for ") + root.section.name
            contentItem: Text {
                text: root.moreOpen ? "less" : "more"
                color: moreButton.hovered || moreButton.visualFocus ? root.theme.accent : root.theme.muted
                font: root.theme.textFont
            }
            background: Item {}
        }
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
        width: parent.width - 28
        theme: root.theme
        button: instanceButton
        navGroup: root.section.key
        active: root.editable
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
