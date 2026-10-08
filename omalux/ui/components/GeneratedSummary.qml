import QtQuick

// The summary rows of one generated module in a pane (ModuleCatalog.summaryFor), drawn like the
// rows of the Filters pane: module icon, short label, value, a dot while the module is on (a
// click on the label switches it) and the chevron that opens the whole module in their place.
// Edits leave as params changes of the first instance, as in GeneratedRows.
Column {
    id: root
    required property var theme
    required property var block              // { module, rows: [{ field, label, colors, row }] }
    required property var moduleState
    required property var catalogModel
    property var overrides: ({})
    property bool editable: true
    property string activeControl: ""
    signal expansionRequested()
    signal changesRequested(var changes)
    signal enableRequested(bool enabled)
    signal resetRequested()
    signal interactionChanged(bool active)
    signal controlSelected(string id)
    // The gap between the rows of the Filters pane.
    spacing: 8

    readonly property string operation: block.module.operation
    // The click shows at once; the engine's answer follows (see GeneratedModule).
    readonly property bool engineEnabled: !!moduleState && moduleState.enabled
    property var requestedEnabled: undefined
    readonly property bool moduleEnabled: requestedEnabled !== undefined ? requestedEnabled : engineEnabled
    onEngineEnabledChanged: requestedEnabled = undefined
    function requestEnabled(on) { requestedEnabled = on; enableRequested(on) }

    function raw(path) {
        const o = root.overrides[root.operation + "/0/" + path]
        if (o !== undefined) return o
        return root.catalogModel.resolve(root.moduleState, path)
    }
    // GeneratedRows.sliderControl, with the pane's label and track colours.
    function control(e) {
        const r = e.row, f = r.factor || 1, o = r.offset || 0
        const a = r.min * f + o, b = r.max * f + o
        const sa = (r.soft_min !== null ? r.soft_min : r.min) * f + o
        const sb = (r.soft_max !== null ? r.soft_max : r.max) * f + o
        const digits = r.digits !== undefined && r.digits !== null ? r.digits : 2
        return { id: root.operation + "/0/" + r.path, label: e.label, unit: r.unit || "", colors: e.colors || "",
                 decimals: digits, step: digits > 0 ? Math.pow(10, -digits) : 1,
                 minimum: Math.min(a, b), maximum: Math.max(a, b),
                 softMinimum: Math.min(sa, sb), softMaximum: Math.max(sa, sb),
                 section: root.block.module.name }
    }
    function edit(r, value) {
        const changes = {}
        changes[r.path] = Math.max(r.min !== null ? r.min : -Infinity, Math.min(r.max !== null ? r.max : Infinity, value))
        root.changesRequested(changes)
    }

    Repeater {
        model: root.block.rows
        delegate: ControlSlider {
            required property var modelData
            readonly property var r: modelData.row
            readonly property var v: root.raw(r.path)
            readonly property bool known: v !== undefined && typeof v !== "object"
            objectName: "summary-row-" + root.operation + "-" + modelData.field
            width: root.width
            theme: root.theme
            control: root.control(modelData)
            value: (known ? Number(v) : Number(r.default)) * (r.factor || 1) + (r.offset || 0)
            editable: root.editable && known && root.catalogModel.writable(r.path, root.moduleState)
            opacity: !known ? .45 : root.moduleEnabled ? 1 : .7
            compact: true
            moduleToggleAvailable: true
            moduleIconKey: root.operation
            displayLabel: modelData.label
            qualifyLabel: false
            moduleName: root.block.module.name
            moduleEnabled: root.moduleEnabled
            onModuleToggleRequested: root.requestEnabled(!root.moduleEnabled)
            selected: root.activeControl === control.id
            detailsAvailable: true
            detailsExpanded: false
            onDetailsRequested: root.expansionRequested()
            onActivated: root.expansionRequested()
            onSelectedRequested: root.controlSelected(control.id)
            onInteractionChanged: active => root.interactionChanged(active)
            onEdited: value => root.edit(r, (value - (r.offset || 0)) / (r.factor || 1))
            onResetRequested: root.changesRequested({ [r.path]: r.default })
            onModuleResetRequested: root.resetRequested()
            navTarget.group: root.operation + "/0"
        }
    }
}
