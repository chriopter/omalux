import QtQuick

// The generated module layout (omalux/design/layout.json) and the engine's module catalog,
// parsed once per change. `states` maps "operation/instance" to the module's current state;
// a module whose catalog entry did not change keeps its previous object, so bindings of
// untouched modules are not re-evaluated while another module is being dragged.
QtObject {
    id: root
    property string catalog: ""
    property string layoutText: "{}"
    // The per-module blend section (omalux/design/layout-blending.json, same row format).
    property string blendLayoutText: "{}"
    readonly property var blendRows: {
        try { return (JSON.parse(root.blendLayoutText || "{}").rows || []) } catch (e) { return [] }
    }
    // ModuleTools (pickers, module buttons, histograms) for the generated rows; may be null.
    property QtObject tools: null

    // Which sidebar tab shows which module. Everything not listed follows its darktable group.
    readonly property var tabOfGroup: ({ base: "tone", tone: "tone", color: "color", correct: "detail",
                                         technical: "detail", effect: "effects" })
    readonly property var tabOfModule: ({
        // Looks that act like a style sit under the style cards.
        colorchecker: "styles", colormapping: "styles", splittoning: "styles", colortransfer: "styles",
        // Geometry next to crop.
        ashift: "geometry", flip: "geometry", lens: "geometry", clipping: "geometry",
        // Deprecated modules, in the tab of the group they used to belong to.
        basicadj: "tone", levels: "tone", filmic: "tone", globaltonemap: "tone", tonemap: "tone",
        relight: "tone", zonesystem: "tone", invert: "color", channelmixer: "color", vibrance: "color",
        spots: "detail", equalizer: "detail", clahe: "detail"
    })
    readonly property var groupLabels: ({ base: "base", tone: "tone", color: "color", correct: "correct",
                                          effect: "effect", technical: "technical", deprecated: "deprecated" })

    readonly property var layout: {
        try { return JSON.parse(root.layoutText || "{}") } catch (e) { return ({ groups: [] }) }
    }
    // operation → layout module, for curated blocks that append their remaining rows.
    readonly property var modulesByOperation: {
        const out = {}
        for (const g of (layout.groups || [])) for (const m of g.modules) out[m.operation] = Object.assign({ group: g.id }, m)
        return out
    }

    property var states: ({})
    // operation → [instances], in pipeline order.
    property var instances: ({})
    // true once the engine reports parameter paths and array values (see controls.md).
    property bool pathsSupported: false
    property var _cache: ({})

    onCatalogChanged: parse(catalog)
    Component.onCompleted: parse(catalog)

    // The engine's catalog holds one module per line (module_catalog.cpp): a module whose line
    // did not change keeps its state without being parsed again. Other texts (tests) are
    // parsed whole and compared module by module.
    function entries(text) {
        if (text.length > 2 && text[0] === "[" && text.indexOf("\n") >= 0) {
            const lines = text.slice(1, -1).split(",\n")
            if (lines.every(l => l[0] === "{" && l[l.length - 1] === "}"))
                return lines.map(l => ({ signature: l, module: null }))
        }
        let parsed = null
        try { parsed = JSON.parse(text || "[]") } catch (e) { return null }
        return parsed.map(m => ({ signature: JSON.stringify(m), module: m }))
    }
    function parse(text) {
        const list = root.entries(text || "[]")
        if (!list) return
        const next = {}, cache = {}, inst = {}
        let paths = false
        for (const entry of list) {
            let cached = root._cache[entry.signature]
            if (!cached) {
                let m = entry.module
                if (!m) { try { m = JSON.parse(entry.signature) } catch (e) { continue } }
                const values = {}, params = {}
                for (const p of m.parameters) {
                    const k = p.path !== undefined ? p.path : p.name
                    values[k] = p.value
                    params[k] = p
                    if (p.field && values[p.field] === undefined && p.value !== undefined) values[p.field] = p.value
                    if (p.field && !params[p.field]) params[p.field] = p
                }
                root.addDerived(m, values)
                const state = { operation: m.operation, instance: m.instance, label: m.label, enabled: !!m.enabled,
                                hidden: !!m.hidden, values: values, params: params,
                                derived: m.derived || ({}), labels: m.labels || ({}) }
                Object.assign(state, root.extras(m))
                cached = { state: state, paths: m.parameters.some(p => p.path !== undefined) }
            }
            const state = cached.state
            const key = state.operation + "/" + state.instance
            next[key] = state
            cache[entry.signature] = cached
            if (cached.paths) paths = true
            if (!inst[state.operation]) inst[state.operation] = []
            inst[state.operation].push(state.instance)
        }
        root._cache = cache
        root.pathsSupported = paths
        if (JSON.stringify(inst) !== JSON.stringify(root.instances)) root.instances = inst
        root.states = next
    }
    // A pushed update for one module (backend signal moduleUpdated, when the engine has it).
    function updateModule(operation, instance, moduleJson) {
        let m
        try { m = typeof moduleJson === "string" ? JSON.parse(moduleJson) : moduleJson } catch (e) { return }
        const key = operation + "/" + instance
        const values = {}, params = {}
        for (const p of (m.parameters || [])) {
            const k = p.path !== undefined ? p.path : p.name
            values[k] = p.value; params[k] = p
            if (p.field && values[p.field] === undefined && p.value !== undefined) values[p.field] = p.value
            if (p.field && !params[p.field]) params[p.field] = p
        }
        root.addDerived(m, values)
        const next = Object.assign({}, root.states)
        next[key] = { operation: operation, instance: instance, label: m.label, enabled: !!m.enabled,
                      hidden: !!m.hidden, values: values, params: params,
                      derived: m.derived || ({}), labels: m.labels || ({}) }
        Object.assign(next[key], extras(m))
        root.states = next
    }

    // darktable's displayed conversions ("@" paths such as "@lift_hue" or "@target_C[0][3]",
    // module_values.c) resolve like parameters.
    function addDerived(m, values) {
        const derived = m.derived || {}
        for (const k in derived) values[k] = derived[k]
    }

    // Runtime lists of module rows (profiles, lenses, LUT files ...), answered by the engine.
    // `choiceResults` maps "operation/instance/list" to { query, items, current, more, error }.
    signal choicesRequested(string operation, int instance, string list, string query)
    property var choiceResults: ({})
    function choiceKey(operation, instance, list) { return operation + "/" + instance + "/" + list }
    function requestChoices(operation, instance, list, query) {
        const key = choiceKey(operation, instance, list)
        const next = Object.assign({}, root.choiceResults)
        next[key] = Object.assign({}, next[key] || { items: [], current: -1, more: 0, error: "" }, { loading: true })
        root.choiceResults = next
        root.choicesRequested(operation, instance, list, query || "")
    }
    function receiveChoices(operation, instance, list, query, text) {
        let parsed = null
        try { parsed = JSON.parse(text || "null") } catch (e) {}
        const next = Object.assign({}, root.choiceResults)
        next[choiceKey(operation, instance, list)] = parsed
            ? { query: query, items: parsed.items || [], current: parsed.current === undefined ? -1 : parsed.current,
                more: parsed.more || 0, error: parsed.error || "", loading: false }
            : { query: query, items: [], current: -1, more: 0, error: "the list is not available", loading: false }
        root.choiceResults = next
    }

    // Pipeline position, blend section and multi-instance state of a catalog entry.
    function extras(m) {
        return { position: m.position, blend: m.blend || null, multiName: m.multi_name || "",
                 instanceLabel: m.instance_label || "", handEdited: !!m.multi_name_hand_edited,
                 canNew: !!m.can_new, canDelete: !!m.can_delete,
                 canMoveUp: !!m.can_move_up, canMoveDown: !!m.can_move_down }
    }
    // A further instance of a curated module shows every row (build_layout.py instance_rows).
    function moduleForInstance(m, instance) {
        if (!m || instance === 0 || !m.instance_rows) return m
        return Object.assign({}, m, { rows: m.instance_rows, primary: m.instance_primary || [],
                                      tabs: m.instance_tabs || [], curated: false })
    }
    // The raster masks earlier modules offer to this one (blend_gui.c _raster_combo_populate):
    // [{ operation, instance, id, label }] in pipeline order.
    function rasterSources(state) {
        const out = []
        if (!state) return out
        for (const key in root.states) {
            const s = root.states[key]
            if (!s.blend || s.position === undefined || s.position >= state.position) continue
            for (const r of (s.blend.raster_masks || []))
                out.push({ operation: s.operation, instance: s.instance, id: r.id, label: r.label, position: s.position })
        }
        out.sort((a, b) => a.position - b.position || a.id - b.id)
        return out
    }

    // "name", "name[i]", "name[i][j].member" → tokens.
    function tokens(path) {
        const out = []
        const re = /([A-Za-z_][A-Za-z0-9_]*)|\[(\d+)\]|\.([A-Za-z_][A-Za-z0-9_]*)/g
        let match
        while ((match = re.exec(path)) !== null)
            out.push(match[1] !== undefined ? match[1] : match[2] !== undefined ? Number(match[2]) : match[3])
        return out
    }
    // Current raw value at a params path, or undefined when the engine does not report it.
    function resolve(state, path) {
        if (!state || path === null || path === undefined) return undefined
        const values = state.values
        if (values[path] !== undefined) return values[path]
        const t = tokens(path)
        if (!t.length) return undefined
        let v = values[t[0]]
        for (let i = 1; i < t.length && v !== undefined && v !== null; ++i) v = v[t[i]]
        return v === null ? undefined : v
    }
    // A params path the engine can write: plain members always, indexed paths once supported,
    // displayed conversions ("@" paths) when the engine reports them for this module (state).
    function writable(path, state) {
        if (path && path.startsWith("@") && path.indexOf("[@") < 0)
            return !!state && !!state.derived && state.derived[path] !== undefined
        if (!path || path.startsWith("@") || path.indexOf("@") >= 0) return false
        return root.pathsSupported || /^[A-Za-z_][A-Za-z0-9_]*$/.test(path)
    }

    // Groups of one sidebar tab: [{ id, label, quiet, modules: [layout module + key] }].
    function groupsForTab(tab) {
        const out = []
        const deprecated = []
        for (const g of (layout.groups || [])) {
            const modules = []
            for (const m of g.modules) {
                const target = tabOfModule[m.operation] || tabOfGroup[g.id] || ""
                if (target !== tab) continue
                if (m.curated && tab !== "geometry") continue
                if (!m.rows.length) continue
                if (g.id === "deprecated") deprecated.push(m); else modules.push(m)
            }
            if (modules.length) out.push({ id: g.id, label: groupLabels[g.id] || g.id, quiet: g.id === "technical", modules: modules })
        }
        if (deprecated.length) out.push({ id: "deprecated", label: "deprecated", quiet: true, deprecated: true, modules: deprecated })
        return out
    }
    function labelOf(m) { return m.name || m.operation }
    // A search term matches a module by its darktable name or operation, or one of its rows.
    function moduleMatches(m, term) {
        if (!term) return true
        if (labelOf(m).toLowerCase().indexOf(term) >= 0 || m.operation.toLowerCase().indexOf(term) >= 0) return true
        return m.rows.some(r => (r.label || "").toLowerCase().indexOf(term) >= 0)
    }
    function moduleNameMatches(m, term) {
        return !term || labelOf(m).toLowerCase().indexOf(term) >= 0 || m.operation.toLowerCase().indexOf(term) >= 0
    }
    function rowMatches(r, term) { return !term || (r.label || "").toLowerCase().indexOf(term) >= 0 }
    // Deprecated modules are listed only while the current image uses them.
    function moduleShown(m) {
        if (m.show !== "if-used") return true
        const list = instances[m.operation] || []
        return list.some(i => states[m.operation + "/" + i] && states[m.operation + "/" + i].enabled)
    }
    function matchCount(tab, term) {
        let n = 0
        for (const g of groupsForTab(tab)) for (const m of g.modules) if (moduleShown(m) && moduleMatches(m, term)) ++n
        return n
    }
}
