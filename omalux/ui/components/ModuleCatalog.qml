import QtQuick

// The generated module layout (omalux/design/layout.json) and the engine's module catalog,
// parsed once per change. `states` maps "operation/instance" to the module's current state;
// a module whose catalog entry did not change keeps its previous object, so bindings of
// untouched modules are not re-evaluated while another module is being dragged.
QtObject {
    id: root
    property string catalog: ""
    property string layoutText: "{}"

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

    function parse(text) {
        let parsed = []
        try { parsed = JSON.parse(text || "[]") } catch (e) { return }
        const next = {}, cache = {}, inst = {}
        let paths = false
        for (const m of parsed) {
            const key = m.operation + "/" + m.instance
            const signature = JSON.stringify(m)
            const previous = root._cache[key]
            if (previous && previous.signature === signature) {
                next[key] = previous.state
                cache[key] = previous
            } else {
                const values = {}, params = {}
                for (const p of m.parameters) {
                    if (p.path !== undefined) paths = true
                    const k = p.path !== undefined ? p.path : p.name
                    values[k] = p.value
                    params[k] = p
                    if (p.field && values[p.field] === undefined && p.value !== undefined) values[p.field] = p.value
                    if (p.field && !params[p.field]) params[p.field] = p
                }
                const state = { operation: m.operation, instance: m.instance, label: m.label, enabled: !!m.enabled,
                                hidden: !!m.hidden, values: values, params: params }
                next[key] = state
                cache[key] = { signature: signature, state: state }
            }
            if (m.parameters.some(p => p.path !== undefined)) paths = true
            if (!inst[m.operation]) inst[m.operation] = []
            inst[m.operation].push(m.instance)
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
        const next = Object.assign({}, root.states)
        next[key] = { operation: operation, instance: instance, label: m.label, enabled: !!m.enabled,
                      hidden: !!m.hidden, values: values, params: params }
        root.states = next
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
    // A params path the engine can write: plain members always, indexed paths once supported.
    function writable(path) {
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
