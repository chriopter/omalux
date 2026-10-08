import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

// The rows of one darktable module as described by omalux/design/layout.json, drawn with the
// same controls as the curated block. Primary rows show while the module is collapsed, detail
// rows when it is expanded and advanced rows behind "more". Rows that need something Omalux
// cannot do yet (drawn shapes) collapse into one muted notice. darktable's pickers and module
// buttons come from catalogModel.tools (ModuleTools). Values darktable shows
// through a conversion ("@" paths) and runtime lists are read from the engine's catalog entry
// ("derived", "labels") and its choice lists (catalogModel.requestChoices).
// The component never writes a parameter: edits leave through changesRequested as
// { params path: raw value }.
Column {
    id: root
    required property var theme
    required property var module          // layout module
    required property var moduleState     // ModuleCatalog state, may be undefined
    required property var catalogModel    // ModuleCatalog: path resolution and capabilities
    property int instance: 0
    property var overrides: ({})
    property string overridePrefix: ""
    property bool editable: true
    property bool expanded: false
    property bool moreOpen: false
    // Curated blocks: every row is secondary and shows only while moreOpen.
    property bool extraMode: false
    property string term: ""
    property bool nameMatched: false
    property string activeControl: ""
    signal changesRequested(var changes)
    signal interactionChanged(bool active)
    signal controlSelected(string id)
    signal moreRequested()
    // Keyboard group of every row (see NavTarget): Shift+R and E reach the module heading.
    property string navGroup: module.operation + "/" + instance
    function navId(r) { return module.operation + "/" + root.instance + "/" + (r.path || r.field) }

    // GeneratedModule passes the state just clicked, before the engine reports it.
    property bool moduleEnabled: !!moduleState && moduleState.enabled
    // ---- module tools (pickers, buttons, histograms; ModuleTools.qml) --------------------
    readonly property var tools: catalogModel && catalogModel.tools ? catalogModel.tools : null
    function toolOf(r) { return root.tools ? root.tools.rowTool(module.operation, r.field) : null }
    // The GUI-only values a tool sends along (the target lightness, the levels channel).
    function toolGui(spec) {
        const out = {}
        for (const name of (spec.gui || []))
            out[name] = name === "@tab" ? root.tabValue : name === "@channel" ? root.lastChannel : (root.gui[name] || 0)
        return out
    }
    function runTool(spec, choice) {
        // area E: a button that writes fixed values (lens "use latest algorithm", lens.cc:2463)
        if (spec.set) { root.changesRequested(Object.assign({}, spec.set)); return }
        if (spec.choices) { root.findList = spec.choices; root.catalogModel.requestChoices(module.operation, root.instance, spec.choices, ""); return }
        const extra = spec.menu && choice >= 0 ? spec.menu[choice].gui : null
        root.tools.toggle(module.operation, root.instance, spec, toolGui(spec), extra)
    }
    function toolEntries(specs) {
        const t = root.tools
        return specs.map(s => ({ label: s.label, kind: s.kind, icon: s.icon || "", hint: s.hint || "", menu: s.menu || null,
                                 active: s.kind === "display" ? !!t && t.displayActive(module.operation, root.instance, s)
                                                              : !!t && !!t.active && t.isActive(module.operation, root.instance, s.tool),
                                 enabled: !s.disabledBy || !root.gui[s.disabledBy] }))
    }
    // What darktable prints beside a picker: exposure's input lightness (exposure.c:887),
    // color calibration's input LCh (channelmixerrgb.c:4189).
    function toolReport(specs) {
        const s = specs.find(x => x.report)
        const r = s && root.tools ? root.tools.results[root.tools.key(module.operation, root.instance, s.tool)] : null
        if (s && s.report === "message") return r && r.message ? r.message.replace(/\n/g, " ") : ""   // area E
        const g = r && r.gui ? r.gui : null
        if (!g || g.input_lightness === undefined) return ""
        if (s.report === "lch")
            return "L: " + Number(g.input_lightness).toFixed(1) + " %  h: " + Number(g.input_hue).toFixed(1)
                   + " °  c: " + Number(g.input_chroma).toFixed(1)
        return "L : " + Number(g.input_lightness).toFixed(1) + " %"
    }
    // area E: a multi-line report of the row's tools (the colour checker's quality report).
    function toolDetails(specs) {
        if (!root.tools) return ""
        for (const s of specs) {
            const r = root.tools.results[root.tools.key(module.operation, root.instance, s.tool)]
            if (s.tool === "checker" && r && r.report) return r.report.replace(/ *\t/g, "  ")
        }
        return ""
    }
    // A "find" button's list (lens find camera / find lens) while its menu is wanted.
    property string findList: ""
    // A picker writes GUI-only values back (exposure "measure" fills the target lightness).
    property var seenResults: ({})
    Connections {
        target: root.tools
        ignoreUnknownSignals: true
        function onResultsChanged() {
            const prefix = root.module.operation + "/" + root.instance + "/"
            const results = root.tools.results
            let g = null
            for (const k in results) {
                if (!k.startsWith(prefix) || root.seenResults[k] === results[k]) continue
                const next = Object.assign({}, root.seenResults); next[k] = results[k]; root.seenResults = next
                const out = results[k].gui || {}
                for (const name in out) if (name.startsWith("@")) {
                    g = g || Object.assign({}, root.gui); g[name] = out[name]
                    root.tools.storeConf(root.tools.confOf(root.module.operation, name), out[name])
                }
            }
            if (g) root.gui = g
        }
    }
    // --------------------------------------------------------------------------------------
    property int tabIndex: 0
    property var gui: ({})
    readonly property int tabValue: module.tabs && module.tabs.length ? tabIndex : (gui["@tab"] || 0)
    // The last colour page shown (color equalizer keeps it while "options" is open,
    // colorequal.c _channel_tabs_switch_callback 2596).
    property int lastChannel: 0
    onTabValueChanged: { if (tabValue < 3) lastChannel = tabValue; refreshDisplay() }
    onLastChannelChanged: refreshDisplay()
    // A module's own mask preview follows the page shown and the checkerboard settings;
    // collapsing the module (darktable: losing focus) switches it off.
    function refreshDisplay() {
        const t = root.tools
        if (!t || !t.moduleDisplay || t.moduleDisplay.operation !== module.operation || t.moduleDisplay.instance !== root.instance) return
        t.updateDisplay(module.operation, root.instance, toolGui(t.moduleDisplay.spec))
    }
    onExpandedChanged: if (!expanded && root.tools) root.tools.moduleCollapsed(module.operation, root.instance)

    spacing: 4

    // ---- static preparation (once per module) ------------------------------------------
    readonly property var localNames: {
        const names = { "@tab": true }
        for (const r of module.rows) {
            const p = r.path || ""
            const re = /\[(@[A-Za-z0-9_]+)\]/g
            let m
            while ((m = re.exec(p)) !== null) names[m[1]] = true
            const conds = r.visible_when ? (r.visible_when.all || [r.visible_when]) : []
            for (const c of conds) if (c.field && c.field.startsWith("@")) names[c.field] = true
        }
        return names
    }
    readonly property var fieldPaths: {
        const out = {}
        for (const r of module.rows) if (r.path) out[r.field] = r.path
        return out
    }
    function sliderControl(r, id) {
        const f = r.factor || 1, o = r.offset || 0
        const a = r.min * f + o, b = r.max * f + o
        const sa = (r.soft_min !== null ? r.soft_min : r.min) * f + o
        const sb = (r.soft_max !== null ? r.soft_max : r.max) * f + o
        const digits = r.digits !== undefined && r.digits !== null ? r.digits : 2
        return { id: id, label: r.label || r.field, unit: r.unit || "", colors: r.colors || "",
                 decimals: digits, step: digits > 0 ? Math.pow(10, -digits) : 1,
                 minimum: Math.min(a, b), maximum: Math.max(a, b),
                 softMinimum: Math.min(sa, sb), softMaximum: Math.max(sa, sb),
                 section: module.name + (r.section ? " · " + r.section : "") }
    }
    function noticeKind(r) {
        if (r.widget === "picker") return "picker"
        if (r.widget === "drawn") return "drawn"
        if (r.widget === "file") return "file"
        if (r.widget === "graph") return "graph"
        return "unavailable"
    }
    function noticeText(kind, labels) {
        const l = labels.filter(x => x).join(", ")
        switch (kind) {
        case "picker": return (l === "picker" ? "image picker" : l + ": image picker") + " not available yet"
        case "drawn": return l + ": drawn on the image, not available yet"
        case "file": return l + ": file choice not available yet"
        case "graph": return "darktable's " + (l || "graph") + " is not drawn here yet"
        default: return l + ": not available yet"
        }
    }
    readonly property var items: {
        const out = []
        const id = r => module.operation + "/" + root.instance + "/" + (r.path || r.field)
        const base = r => ({ row: r, tier: r.tier === "primary" && ["slider", "combobox", "toggle", "text"].indexOf(r.widget) < 0 ? "detail" : r.tier,
                             tab: r.tab, section: r.section, cond: r.visible_when, labels: [r.label] })
        for (const r of module.rows) {
            const path = r.path
            const local = path && path.startsWith("@") && path.indexOf("[") < 0 && localNames[path]
            let it
            // Module tools: GUI-only rows they read, and the pickers and buttons themselves.
            const ts = root.toolOf(r)
            // area E: a display darktable draws above the row (color mapping clusters, vectorscope).
            if (ts && ts.before) out.push(Object.assign(base(r), { kind: ts.before, tier: r.tier === "primary" ? "detail" : r.tier }))
            if (ts && ts.local) {
                const lr = Object.assign({}, r, { path: r.field })
                out.push(Object.assign(base(lr), r.widget === "slider"
                         ? { kind: "localSlider", control: sliderControl(lr, id(lr)) }
                         : r.widget === "color" ? { kind: "localColor", fallback: ts.fallback || [0, 0, 0] } : { kind: "local" }))
                if (!ts.tool) continue
            }
            if (ts && (ts.tool || ts.choices || ts.set)) {
                const spec = Object.assign({ label: r.label }, ts)
                const prev = out.length ? out[out.length - 1] : null
                const item = Object.assign(base(r), { kind: "tools", specs: [spec] })
                if (ts.when) item.cond = { all: [r.visible_when, ts.when].filter(x => x) }
                if (item.tier === "primary") item.tier = "detail"
                if (prev && prev.kind === "tools" && prev.tier === item.tier && prev.tab === item.tab && prev.section === item.section
                        && JSON.stringify(prev.cond) === JSON.stringify(item.cond) && prev.specs.length < 4) {
                    prev.specs = prev.specs.concat([spec]); prev.labels.push(r.label)
                } else out.push(item)
                continue
            }
            // ---- values and lists (area "Werte & Listen") ----
            const derivedPath = path && path.startsWith("@") && !local
            if (r.custom && r.custom.kind === "choice")
                it = Object.assign(base(r), { kind: "choiceList", tier: r.tier === "primary" ? "detail" : r.tier })
            else if (r.custom && r.custom.kind === "patches")
                it = Object.assign(base(r), { kind: "patches" })
            else if (r.widget === "text" && r.custom && r.custom.editable && path)
                it = Object.assign(base(r), { kind: "textEdit" })
            else if (r.widget === "slider" && derivedPath)
                it = Object.assign(base(r), { kind: "slider", derived: true, control: sliderControl(r, id(r)) })
            // area E: retouch's wavelet decompose bar (WaveletBar); its preview levels only shape
            // darktable's darkroom preview of a single scale, which the photo here does not show.
            else if (module.operation === "retouch" && r.field === "wavelet_decompose")
                it = Object.assign(base(r), { kind: "waveletbar" })
            else if (module.operation === "retouch" && r.field === "preview_levels")
                it = Object.assign(base(r), { kind: "notice", text: "preview single scale: darktable shows one wavelet scale in its darkroom preview, which is not drawn here" })
            // area E: zone system's bar edits both its rows (the number of zones by the wheel).
            else if (r.widget === "drawn" && module.operation === "zonesystem" && r.field === "size") continue
            else if (r.widget === "drawn" && module.operation === "zonesystem" && r.field === "zone")
                it = Object.assign(base(r), { kind: "zonebar", tier: r.tier === "primary" ? "detail" : r.tier })
            else if (r.widget === "drawn" && r.field === "grid" && ["monochrome", "colorcorrection"].indexOf(module.operation) >= 0)
                it = Object.assign(base(r), { kind: "colorgrid", tier: r.tier === "primary" ? "detail" : r.tier })   // area E
            else if (r.widget === "combobox" && derivedPath && r.values) // area E: a list position (clipping @aspect, @flip)
                it = Object.assign(base(r), { kind: "choice", derived: true })
            else if (r.widget === "color" && derivedPath)
                it = Object.assign(base(r), { kind: "color", derived: true, paths: [path + "[0]", path + "[1]", path + "[2]"] })
            // ---- end values and lists ----
            else if (r.widget === "notice") it = Object.assign(base(r), { kind: "notice", text: r.label })
            // A tool drawn on the photo (CanvasToolRow); selecting it shows the tool there.
            else if (r.widget === "canvas") it = Object.assign(base(r), { kind: "canvas", tier: r.tier })
            else if (local && (r.widget === "combobox" || r.widget === "toggle" || r.widget === "text"))
                it = Object.assign(base(r), { kind: "local" })
            else if (r.widget === "curve" && r.custom && !r.custom.nodes_field && r.custom.fields.length)
                it = Object.assign(base(r), { kind: "bands", tier: r.tier })
            else if (r.widget === "curve" && r.custom && r.custom.nodes_field && !r.custom.count_field)
                it = Object.assign(base(r), { kind: "bars", tier: r.tier })
            else if (r.widget === "curve" && r.custom && r.custom.nodes_field)
                it = Object.assign(base(r), { kind: "curve", tier: r.tier })
            else if (r.widget === "graph" && r.custom && r.custom.fields.length) {
                // Levels: darktable's three handles over the histogram, here as three sliders.
                const channels = r.custom.channels || []
                const names = ["black", "gray", "white"]
                const deflt = Array.isArray(r.default) ? r.default : [0, 0.5, 1]
                // darktable draws the module input histogram behind the three handles.
                if (root.tools && root.tools.histogramField(module.operation) === r.field)
                    out.push(Object.assign(base(r), { kind: "histogram", tier: r.tier, levelsPath: r.custom.fields[0], multi: channels.length > 1 }))
                if (channels.length > 1)
                    out.push(Object.assign(base(r), { kind: "channels", channels: channels, tier: r.tier,
                                                      cond: { all: [r.visible_when, { field: "autoscale", in: [1] }].filter(x => x) } }))
                for (let i = 0; i < 3; ++i) {
                    const sr = Object.assign({}, r, { field: r.field + "_" + names[i], label: names[i], widget: "slider",
                                                      path: r.custom.fields[0] + (channels.length > 1 ? "[@tab]" : "") + "[" + i + "]",
                                                      min: 0, max: 1, soft_min: 0, soft_max: 1, digits: 3, factor: 1, offset: 0,
                                                      default: deflt[i] })
                    out.push(Object.assign(base(sr), { kind: "slider", control: sliderControl(sr, id(sr)) }))
                }
                continue
            }
            else if (r.widget === "color" && r.custom && r.custom.fields.length && path && !path.startsWith("@")) {
                const f = r.custom.fields
                it = Object.assign(base(r), { kind: "color", paths: f.length === 3 ? f : [f[0] + "[0]", f[0] + "[1]", f[0] + "[2]"] })
            }
            else if (r.widget === "text") it = Object.assign(base(r), { kind: "text" })
            else if (!path || path.startsWith("@") || r.widget === "picker" || r.widget === "button"
                     || r.widget === "drawn" || r.widget === "file" || r.widget === "graph" || r.widget === "color")
                it = Object.assign(base(r), { kind: "notice", notice: noticeKind(r) })
            else if (r.widget === "slider") it = Object.assign(base(r), { kind: "slider", control: sliderControl(r, id(r)) })
            else if (r.widget === "combobox") it = Object.assign(base(r), { kind: "choice" })
            else if (r.widget === "toggle") it = Object.assign(base(r), { kind: "switch" })
            else it = Object.assign(base(r), { kind: "notice", notice: "unavailable" })
            // Neighbouring notices of the same kind and place read as one line.
            const prev = out.length ? out[out.length - 1] : null
            if (it.kind === "notice" && it.notice && prev && prev.kind === "notice" && prev.notice === it.notice
                    && prev.tier === it.tier && prev.tab === it.tab && prev.section === it.section
                    && JSON.stringify(prev.cond) === JSON.stringify(it.cond)) {
                prev.labels.push(r.label)
                prev.text = noticeText(prev.notice, prev.labels)
                continue
            }
            if (it.kind === "notice" && it.notice) it.text = noticeText(it.notice, it.labels)
            // What cannot be used never takes a place among the rows shown while collapsed.
            if (it.kind === "notice" && it.tier === "primary") it.tier = "detail"
            out.push(it)
        }
        // A notebook page whose rows are all "advanced" (tone equalizer "advanced", "masking")
        // shows them when chosen: darktable's page has no "more", and an empty page reads as a
        // tab that did nothing.
        const pageTabs = module.tabs || []
        for (const t of pageTabs)
            if (!out.some(it => it.tab === t && it.tier !== "advanced"))
                for (const it of out) if (it.tab === t) it.tier = "detail"
        // Section captions where darktable has them. A colour swatch named like its caption
        // shows no label of its own (negadoctor "color of the film base", negadoctor.c:852); a
        // caption that only repeats the label of a value row below it is left out (white
        // balance "settings" is an action section in darktable, not a caption, temperature.c:2130).
        for (const it of out)
            if (it.kind === "color" && it.section && it.row.label === it.section) it.underCaption = true
        const repeats = it => out.some(o => o.kind !== "color" && o.section === it.section && o.tab === it.tab
                                            && o.row && o.row.label === it.section)
        const withSections = []
        let section = null, tab = null
        for (const it of out) {
            if (it.section && (it.section !== section || it.tab !== tab) && !repeats(it))
                withSections.push({ kind: "section", text: it.section, tab: it.tab, tier: it.tier, cond: null, labels: [] })
            section = it.section; tab = it.tab
            withSections.push(it)
        }
        return withSections
    }
    readonly property bool hasAdvanced: items.some(it => it.tier === "advanced")
    readonly property bool hasDetails: items.some(it => it.tier !== "primary" && it.kind !== "section")
    Component.onCompleted: {
        const g = {}
        for (const r of module.rows)
            if (r.path && localNames[r.path]) g[r.path] = r.default !== null && r.default !== undefined ? Number(r.default) || 0 : 0
        for (const r of module.rows) {
            const ts = root.toolOf(r)
            if (ts && ts.local) g[r.field] = r.default !== null && r.default !== undefined ? Number(r.default) || 0 : 0
            // area E: darktable's dt_conf value of this GUI-only row, as stored last time.
            if (ts && ts.conf) g[r.field] = root.tools.confValues(ts, g[r.field])
        }
        gui = g
    }

    // ---- values ------------------------------------------------------------------------
    function subst(path) {
        if (!path) return path
        return path.replace(/\[(@[A-Za-z0-9_]+)\]/g, (m, name) => "[" + (name === "@tab" ? root.tabValue : (root.gui[name] || 0)) + "]")
    }
    function raw(path) {
        const p = subst(path)
        const o = root.overrides[root.overridePrefix + p]
        if (o !== undefined) return o
        return root.catalogModel.resolve(root.moduleState, p)
    }
    function readable(r) { return raw(r.path) !== undefined }
    function canWrite(path) { return root.catalogModel.writable(subst(path), root.moduleState) }
    function valueOrDefault(r) {
        const v = raw(r.path)
        return v !== undefined && typeof v !== "object" ? Number(v) : (r.default !== null && typeof r.default !== "object" ? Number(r.default) : 0)
    }
    function fieldValue(field) {
        if (field.startsWith("@")) return root.gui[field]
        if (root.fieldPaths[field]) return raw(root.fieldPaths[field])
        return root.catalogModel.resolve(root.moduleState, field)
    }
    function condition(c) {
        if (!c) return true
        if (c.all) return c.all.every(x => condition(x))
        const v = fieldValue(c.field)
        if (v === undefined || v === null || typeof v === "object") return true
        return c.in.indexOf(Math.round(Number(v))) >= 0
    }
    // A displayed conversion whose range depends on the image (white balance finetune: the
    // camera preset's tuning range, temperature.c:1257-1264) carries "_min"/"_max" values.
    function derivedControl(it) {
        const lo = root.raw(it.row.path + "_min"), hi = root.raw(it.row.path + "_max")
        if (typeof lo !== "number" || typeof hi !== "number") return it.control
        return Object.assign({}, it.control, { minimum: lo, maximum: hi, softMinimum: lo, softMaximum: hi })
    }
    function edit(path, value) {
        const changes = {}
        changes[subst(path)] = value
        root.changesRequested(changes)
    }
    function editRaw(r, value) {
        root.edit(r.path, Math.max(r.min !== null ? r.min : -Infinity, Math.min(r.max !== null ? r.max : Infinity, value)))
    }
    function setGui(name, value) {
        const g = Object.assign({}, root.gui); g[name] = value; root.gui = g
        if (root.tools) root.tools.storeConf(root.tools.confOf(module.operation, name), value)
        if (name.startsWith("@checker_")) refreshDisplay()
    }
    // A GUI-only value a module tool reads changed: an active picker of this module applies again,
    // in "correction" mode only (exposure.c and channelmixerrgb.c _spot_settings_changed_callback;
    // in "measure" mode the target is just recorded).
    function guiEdited() {
        if (!root.tools || !root.tools.active || root.tools.active.operation !== module.operation) return
        // area E: chart, optimisation and patch scale only redraw the chart on the photo
        // (channelmixerrgb.c _checker_changed_callback 2850, _safety_changed_callback 2875);
        // the profile is computed again with "recompute".
        if (root.tools.active.tool === "checker") { root.tools.chartSettings(toolGui({ gui: ["@checker", "@optimize", "@safety"] })); return }
        if (root.gui["@area_mode"] === 1 || root.gui["@spot_mode"] === 1) return
        const spec = { gui: Object.keys(root.tools.active.gui || {}) }
        root.tools.updateGui(module.operation, root.instance, toolGui(spec))
    }
    function matches(it) {
        const t = root.term
        return !t || root.nameMatched || it.labels.some(l => (l || "").toLowerCase().indexOf(t) >= 0)
                  || (it.text || "").toLowerCase().indexOf(t) >= 0
    }
    // Which items are on screen, recomputed when values, tab, expansion or search change.
    readonly property var visibility: {
        const searching = root.term !== ""
        const tabs = module.tabs || []
        const shown = items.map(it => {
            if (it.kind === "section") return false
            let ok
            if (searching) ok = root.matches(it)
            else if (root.extraMode) ok = root.moreOpen && (!tabs.length || !it.tab || it.tab === tabs[root.tabIndex])
            else {
                const tier = it.tier === "primary" || (it.tier === "detail" && root.expanded)
                             || (it.tier === "advanced" && root.expanded && root.moreOpen)
                const tab = !tabs.length || !it.tab || it.tab === tabs[root.tabIndex] || (!root.expanded && it.tier === "primary")
                ok = tier && tab
            }
            // A displayed conversion the engine does not report here (e.g. white balance finetune
            // without a camera preset with tuning) is not shown, as darktable hides the slider.
            if (ok && it.derived && (it.kind === "slider" || it.kind === "choice") && !root.readable(it.row)) ok = false
            // The fourth white balance coefficient exists only on 4-colour sensors (temperature.c:2010).
            if (ok && module.operation === "temperature" && it.row.field === "various" && root.raw("@four_channels") !== 1) ok = false
            return ok && root.condition(it.cond)
        })
        for (let i = 0; i < items.length; ++i) {
            if (items[i].kind !== "section" || !(root.expanded || root.extraMode || searching)) continue
            for (let j = i + 1; j < items.length && items[j].kind !== "section"; ++j)
                if (shown[j] && items[j].section === items[i].text) { shown[i] = true; break }
        }
        return shown
    }
    readonly property bool showsUnreadable: !root.catalogModel.pathsSupported
        && items.some((it, i) => visibility[i] && (it.kind === "slider" || it.kind === "choice" || it.kind === "switch") && !readable(it.row))

    ModuleTabs {
        visible: root.module.tabs.length > 0 && (root.extraMode ? root.moreOpen : root.expanded) && root.term === ""
        width: root.width - 28
        theme: root.theme
        tabs: root.module.tabs
        currentIndex: root.tabIndex
        editable: true
        onTabSelected: index => root.tabIndex = index
        navTarget.navId: root.navGroup + "/@tabs"
        navTarget.group: root.navGroup
    }
    Repeater {
        model: root.items
        delegate: Loader {
            id: entry
            required property var modelData
            required property int index
            readonly property bool shown: !!root.visibility[index]
            visible: shown
            // A row is built once the change that shows it has settled: the search term and the
            // expansion reach the rows along separate bindings, and their in-between states
            // (expanded but not yet searching, or the reverse) would otherwise build and drop
            // every row of every module. The first build happens at once.
            active: false
            Component.onCompleted: active = shown
            onShownChanged: if (shown) Qt.callLater(entry.settle); else active = false
            function settle() { active = shown }
            property var it: modelData
            sourceComponent: ({ slider: sliderRow, choice: choiceRow, "switch": switchRow, local: localRow, text: textRow,
                                notice: noticeRow, section: sectionRow, curve: curveRow, bars: barsRow, bands: bandsRow,
                                color: colorRow, channels: channelsRow, choiceList: choiceListRow,
                                textEdit: textEditRow, patches: patchesRow, tools: toolsRow, localSlider: localSliderRow, localColor: localColorRow,
                                histogram: histogramRow, canvas: canvasRow, clusters: clustersRow, vectorscope: vectorscopeRow, colorgrid: colorGridRow, zonebar: zoneBarRow, waveletbar: waveletBarRow })[modelData.kind] || noticeRow
        }
    }
    ModuleNotice {
        visible: root.showsUnreadable
        width: root.width - 28
        theme: root.theme
        text: "greyed-out values need a newer engine build to be read and edited"
    }
    Item {
        visible: !root.extraMode && root.expanded && root.hasAdvanced && root.term === ""
        width: root.width - 28
        implicitHeight: 22
        ToolButton {
            id: moreButton
            objectName: "module-more-" + root.module.operation
            anchors.left: parent.left
            padding: 0
            hoverEnabled: true
            onClicked: { moreNav.claim(); root.moreRequested() }
            Accessible.name: (root.moreOpen ? "Fewer settings for " : "More settings for ") + root.module.name
            NavTarget { id: moreNav; navId: root.navGroup + "/@more"; label: root.moreOpen ? "less" : "more"; group: root.navGroup; onActivate: root.moreRequested() }
            contentItem: Text {
                text: root.moreOpen ? "less" : "more"
                color: moreButton.hovered || moreButton.visualFocus || moreNav.current ? root.theme.accent : root.theme.muted
                font: root.theme.textFont
            }
            background: Item {}
        }
    }

    // ---- row components ----------------------------------------------------------------
    // Choice and switch rows read like slider rows: label in the control font, value column
    // aligned with the slider values, the disclosure column kept free on the right.
    Component {
        id: sliderRow
        ControlSlider {
            readonly property var r: it.row
            readonly property bool known: root.readable(r)
            width: root.width
            theme: root.theme
            control: it.derived ? root.derivedControl(it) : it.control
            value: root.valueOrDefault(r) * (r.factor || 1) + (r.offset || 0)
            editable: root.editable && known && root.canWrite(r.path)
            compact: true
            moduleToggleAvailable: false
            qualifyLabel: false
            // Collapsed, section captions are hidden: say which section a primary row is in.
            // Only where two collapsed rows would otherwise read the same (split-toning hue).
            displayLabel: r.section && !root.expanded && !root.extraMode && root.term === ""
                          && root.module.rows.some(o => o.field !== r.field && o.tier === "primary" && o.label === r.label)
                          ? r.section + " · " + r.label : ""
            opacity: !known ? .45 : root.moduleEnabled ? 1 : .7
            selected: root.activeControl === it.control.id
            onSelectedRequested: root.controlSelected(it.control.id)
            onInteractionChanged: active => root.interactionChanged(active)
            onEdited: v => root.editRaw(r, (v - (r.offset || 0)) / (r.factor || 1))
            onResetRequested: root.edit(r.path, r.default)
            navTarget.group: root.navGroup
            // darktable's picker on the slider (its "quad" button), in the free right column.
            Loader {
                readonly property var spec: root.tools ? root.tools.sliderTool(root.module.operation, r.field) : null
                active: !!spec
                anchors.right: parent.right
                anchors.verticalCenter: parent.verticalCenter
                width: 24; height: 24
                sourceComponent: ModuleToolButtons {
                    width: 24 + 28
                    theme: root.theme
                    entries: root.toolEntries([Object.assign({}, spec, { label: "" })])
                    editable: root.editable
                    navPrefix: root.navId(r) + "/@picker"
                    navGroup: root.navGroup
                    onTriggered: (index, choice) => root.runTool(spec, choice)
                }
            }
            // area E: what a marking picker found (relight center, color equalizer hue).
            PickerBand {
                readonly property var spec: root.tools ? root.tools.sliderTool(root.module.operation, r.field) : null
                theme: root.theme
                band: spec && spec.band ? root.tools.band(root.module.operation, root.instance, spec.tool) : null
                x: 0; y: parent.height - 7
                width: parent.width - 28; height: 5
            }
        }
    }
    Component {
        id: choiceRow
        RowWrapper {
            readonly property bool known: root.readable(it.row)
            width: root.width
            theme: root.theme
            label: it.row.label
            resetEnabled: root.editable && known && root.canWrite(it.row.path)
            onResetRequested: root.edit(it.row.path, it.row.default)
            opacity: !known ? .45 : root.moduleEnabled ? 1 : .7
            ControlChoice {
                width: parent.width
                theme: root.theme
                label: it.row.label
                labelFont: root.theme.settingsFont
                labelColor: root.theme.ink
                options: (it.row.values || []).filter(o => o.label)
                value: root.valueOrDefault(it.row)
                editable: root.editable && root.readable(it.row) && root.canWrite(it.row.path)
                onEdited: v => root.edit(it.row.path, v)
                onResetRequested: root.edit(it.row.path, it.row.default)
                navTarget.navId: root.navId(it.row)
                navTarget.group: root.navGroup
            }
        }
    }
    Component {
        id: switchRow
        RowWrapper {
            readonly property bool known: root.readable(it.row)
            width: root.width
            theme: root.theme
            label: it.row.label
            resetEnabled: root.editable && known && root.canWrite(it.row.path)
            onResetRequested: root.edit(it.row.path, it.row.default)
            opacity: !known ? .45 : root.moduleEnabled ? 1 : .7
            ControlSwitch {
                width: parent.width
                theme: root.theme
                label: it.row.label
                labelFont: root.theme.settingsFont
                labelColor: root.theme.ink
                value: root.valueOrDefault(it.row)
                editable: root.editable && root.readable(it.row) && root.canWrite(it.row.path)
                onEdited: v => root.edit(it.row.path, v)
                onResetRequested: root.edit(it.row.path, it.row.default)
                navTarget.navId: root.navId(it.row)
                navTarget.group: root.navGroup
            }
        }
    }
    // GUI-only state that picks what other rows show (destination channel, colour patch).
    Component {
        id: localRow
        RowWrapper {
            width: root.width
            theme: root.theme
            label: it.row.label
            resetEnabled: false
            Loader {
                width: parent.width
                readonly property var r: it.row
                readonly property var choices: {
                    if (r.values) return r.values.filter(o => o.label)
                    const count = r.custom && r.custom.fields.length
                                  ? Number(root.catalogModel.resolve(root.moduleState, r.custom.fields[0])) || 0 : 0
                    const list = []
                    for (let i = 0; i < count; ++i)
                        list.push({ value: i, label: r.path === "@patch" ? "patch #" + i : String(i + 1) })
                    return list
                }
                sourceComponent: r.widget === "toggle" ? localSwitch : localChoice
            }
        }
    }
    Component {
        id: localChoice
        ControlChoice {
            theme: root.theme
            label: r.label
            labelFont: root.theme.settingsFont
            labelColor: root.theme.ink
            options: choices
            value: root.gui[r.path] || 0
            editable: true
            onEdited: v => { root.setGui(r.path, v); root.guiEdited() }
            navTarget.navId: root.navId(r)
            navTarget.group: root.navGroup
            navTarget.resettable: false
        }
    }
    Component {
        id: localSwitch
        ControlSwitch {
            theme: root.theme
            label: r.label
            labelFont: root.theme.settingsFont
            labelColor: root.theme.ink
            value: root.gui[r.path] || 0
            editable: true
            onEdited: v => { root.setGui(r.path, v); root.guiEdited() }
            navTarget.navId: root.navId(r)
            navTarget.group: root.navGroup
            navTarget.resettable: false
        }
    }
    Component {
        id: textRow
        RowLayout {
            readonly property var r: it.row
            readonly property var p: r.path && root.moduleState ? root.moduleState.params[root.subst(r.path)] : undefined
            readonly property var v: r.path ? root.raw(r.path) : undefined
            width: root.width
            spacing: 8
            Text {
                Layout.fillWidth: true
                text: r.label
                color: root.theme.ink; font: root.theme.settingsFont; elide: Text.ElideRight
            }
            Text {
                Layout.rightMargin: 28
                Layout.maximumWidth: root.width * .55
                elide: Text.ElideLeft
                text: {
                    if (p && p.values && v !== undefined) {
                        const o = p.values.find(x => x.value === Math.round(v))
                        if (o && o.label) return o.label
                    }
                    if (typeof v === "number") return Number(v).toFixed(Math.abs(v) >= 100 ? 0 : 1)
                    if (typeof r.default === "string") return r.default
                    return "set by darktable"
                }
                color: root.theme.muted; font: root.theme.settingsFont
            }
        }
    }
    Component {
        id: canvasRow
        CanvasToolRow {
            width: root.width - 28
            theme: root.theme
            label: it.row.label
            hint: it.row.custom ? it.row.custom.hint || "" : ""
            active: root.activeControl.indexOf(root.navGroup + "/") === 0
            editable: root.editable
            onActivated: root.controlSelected(root.navId(it.row))
            navTarget.navId: root.navId(it.row)
            navTarget.group: root.navGroup
        }
    }
    Component {
        id: noticeRow
        ModuleNotice { theme: root.theme; text: it.text || ""; width: root.width - 28 }
    }
    Component {
        id: sectionRow
        Text {
            width: root.width - 28
            text: it.text
            topPadding: 6
            color: root.theme.muted
            font: root.theme.textFont
        }
    }
    Component {
        id: channelsRow
        ChannelChooser {
            width: root.width - 28
            theme: root.theme
            options: it.channels.map((c, i) => ({ label: c, value: i, color: ({ R: "#e05555", G: "#5ac06a", B: "#5a8ad0" })[c] }))
            current: root.tabValue
            onChosen: v => root.setGui("@tab", v)
            navTarget.navId: root.navGroup + "/@channel"
            navTarget.group: root.navGroup
        }
    }
    Component {
        id: curveRow
        Column {
            id: curveBox
            readonly property var r: it.row
            readonly property var c: r.custom
            readonly property bool multi: c.channels.length > 1
            readonly property bool channelsActive: {
                if (!multi) return false
                if (root.module.operation === "rgbcurve") return Math.round(root.valueOrDefault({ path: "curve_autoscale", default: 0 })) === 1
                if (root.module.operation === "tonecurve") return Math.round(root.valueOrDefault({ path: "tonecurve_autoscale_ab", default: 3 })) === 0
                return true
            }
            readonly property int channel: channelsActive ? (root.gui["@tab"] || 0) : 0
            readonly property string nodesPath: c.nodes_field + (multi ? "[" + channel + "]" : "")
            readonly property string countPath: c.count_field + (multi ? "[" + channel + "]" : "")
            readonly property string typePath: c.fields.length > 2 ? c.fields[2] + (multi ? "[" + channel + "]" : "") : ""
            readonly property var stored: root.raw(nodesPath)
            readonly property int count: Number(root.raw(countPath))
            readonly property bool known: Array.isArray(stored) && count > 0
            readonly property var nodes: known ? stored.slice(0, count).map(n => ({ x: Number(n.x), y: Number(n.y) }))
                                               : [{ x: 0, y: 0 }, { x: 1, y: 1 }]
            readonly property string interpolation: {
                const t = typePath ? root.raw(typePath) : undefined
                if (t === 0) return "cubic"
                if (t === 1) return "catmull"
                if (t === 2) return "monotone"
                return c.interpolation || "monotone"
            }
            spacing: 6
            width: root.width - 28
            ChannelChooser {
                visible: curveBox.channelsActive
                theme: root.theme
                options: curveBox.c.channels.map((ch, i) => ({ label: ch, value: i, color: ({ R: "#e05555", G: "#5ac06a", B: "#5a8ad0" })[ch] }))
                current: curveBox.channel
                onChosen: v => root.setGui("@tab", v)
                navTarget.navId: root.navGroup + "/@curve-channel"
                navTarget.group: root.navGroup
            }
            CurveEditor {
                objectName: "curve-" + root.module.operation
                width: parent.width
                theme: root.theme
                nodes: curveBox.nodes
                interpolation: curveBox.interpolation
                splineVersion: ["tonecurve", "basecurve"].indexOf(root.module.operation) >= 0 ? 1 : 2
                // area E: "scale for graph" (basecurve.c:2124, tonecurve.c:1325, to_log): both axes
                // logarithmic; tone curve only for its L curve.
                readonly property real logBase: root.module.operation === "basecurve"
                    || (root.module.operation === "tonecurve" && curveBox.channel === 0) ? (root.gui["@scale_for_graph"] || 0) : 0
                xLog: logBase
                yLog: logBase
                periodic: !!curveBox.c.periodic
                background: root.module.operation === "colorzones"
                            ? (Math.round(root.valueOrDefault({ path: "channel", default: 2 })) === 2 ? "gradient-hue" : "gradient-luma") : "none"
                referenceLine: root.module.operation === "colorzones" ? "center" : "diagonal"
                readOnly: !curveBox.known || !root.canWrite(curveBox.nodesPath)
                editable: root.editable
                curveLabel: curveBox.r.label
                maxNodes: 20
                aspectRatio: .8
                opacity: root.moduleEnabled ? 1 : .7
                onInteractionChanged: active => root.interactionChanged(active)
                navTarget.navId: root.navId(curveBox.r)
                navTarget.group: root.navGroup
                onNodesEdited: list => {
                    const changes = {}
                    for (let i = 0; i < list.length; ++i) {
                        changes[curveBox.nodesPath + "[" + i + "].x"] = list[i].x
                        changes[curveBox.nodesPath + "[" + i + "].y"] = list[i].y
                    }
                    if (curveBox.c.count_field) changes[curveBox.countPath] = list.length
                    root.changesRequested(changes)
                }
                onResetRequested: {
                    const changes = {}
                    changes[curveBox.nodesPath + "[0].x"] = 0; changes[curveBox.nodesPath + "[0].y"] = 0
                    changes[curveBox.nodesPath + "[1].x"] = 1; changes[curveBox.nodesPath + "[1].y"] = 1
                    changes[curveBox.countPath] = 2
                    root.changesRequested(changes)
                }
                // The active picker's band (min…max) and mean on the graph, with darktable's
                // "input → output" label where the module prints one.
                Canvas {
                    id: pickerMarker
                    readonly property var marker: root.tools ? root.tools.marker(root.module.operation, root.instance) : null
                    readonly property int ch: Math.max(0, Math.min(2, curveBox.channel))
                    visible: !!marker
                    x: 0; y: 22
                    width: parent.width; height: parent.height - 22
                    onMarkerChanged: requestPaint()
                    onWidthChanged: requestPaint()
                    onPaint: {
                        const c = getContext("2d")
                        c.clearRect(0, 0, width, height)
                        const m = marker
                        if (!m) return
                        const curve = parent
                        const a = curve.toPx(Math.max(0, Math.min(1, m.min[ch]))), b = curve.toPx(Math.max(0, Math.min(1, m.max[ch])))
                        c.fillStyle = Qt.rgba(0.7, 0.5, 0.5, 0.33)
                        c.fillRect(Math.min(a, b), 0, Math.max(1, Math.abs(b - a)), height)
                        c.strokeStyle = Qt.rgba(0.9, 0.7, 0.7, 0.8)
                        c.lineWidth = 1
                        const x = Math.round(curve.toPx(Math.max(0, Math.min(1, m.mean[ch])))) + .5
                        c.beginPath(); c.moveTo(x, 0); c.lineTo(x, height); c.stroke()
                    }
                    Text {
                        visible: !!pickerMarker.marker && !!pickerMarker.marker.text
                        x: 8; y: 6
                        text: pickerMarker.marker && pickerMarker.marker.text ? pickerMarker.marker.text[pickerMarker.ch] : ""
                        color: root.theme.ink
                        font: root.theme.textFont
                    }
                }
            }
            ModuleNotice {
                visible: !curveBox.known
                width: parent.width
                theme: root.theme
                text: "curve points need a newer engine build; shown as a straight line"
            }
        }
    }
    // Values over fixed positions stored as x and y arrays (wavelet bands, low light).
    Component {
        id: barsRow
        Column {
            id: barsBox
            readonly property var r: it.row
            readonly property var c: r.custom
            readonly property bool multi: c.channels.length > 1
            readonly property int channel: multi ? (root.gui["@tab"] || 0) : 0
            readonly property string yPath: c.nodes_field + (multi ? "[" + channel + "]" : "")
            readonly property string xPath: (c.fields.find(f => f.indexOf("x") === 0 || f.indexOf("_x") > 0) || "") + (multi ? "[" + channel + "]" : "")
            readonly property var xs: root.raw(xPath)
            readonly property var ys: root.raw(yPath)
            readonly property bool known: Array.isArray(xs) && Array.isArray(ys) && xs.length === ys.length && xs.length > 0
            spacing: 6
            width: root.width - 28
            ChannelChooser {
                visible: barsBox.multi
                theme: root.theme
                options: barsBox.c.channels.map((ch, i) => ({ label: ch, value: i }))
                current: barsBox.channel
                onChosen: v => root.setGui("@tab", v)
                navTarget.navId: root.navGroup + "/@bars-channel"
                navTarget.group: root.navGroup
            }
            GraphView {
                visible: barsBox.known
                width: parent.width
                theme: root.theme
                title: barsBox.r.label
                xs: barsBox.known ? barsBox.xs.map(Number) : []
                ys: barsBox.known ? barsBox.ys.map(Number) : []
                yMin: 0; yMax: 1; yZero: .5
                editable: root.editable && barsBox.known && root.canWrite(barsBox.yPath)
                axisLabels: root.module.operation === "lowlight" ? ["dark", "bright"] : ["coarse", "fine"]
                opacity: root.moduleEnabled ? 1 : .7
                onInteractionChanged: active => root.interactionChanged(active)
                onValueEdited: (i, v) => root.edit(barsBox.yPath + "[" + i + "]", v)
                navTarget.navId: root.navId(barsBox.r)
                navTarget.group: root.navGroup
            }
            ModuleNotice {
                visible: !barsBox.known
                width: parent.width
                theme: root.theme
                text: barsBox.r.label + ": this graph needs a newer engine build"
            }
        }
    }
    // Band sliders shown together as one graph (tone equalizer): the same scalar parameters.
    Component {
        id: bandsRow
        GraphView {
            id: bands
            readonly property var r: it.row
            readonly property var bandRows: root.module.rows.filter(x => r.custom.fields.indexOf(x.field) >= 0 && /EV$/.test(x.label || ""))
            width: root.width - 28
            theme: root.theme
            // darktable's advanced page draws the graph without a caption; the header keeps the readout.
            title: ""
            xs: bandRows.map((x, i) => i - bandRows.length + 1)
            ys: bandRows.map(x => root.valueOrDefault(x))
            yMin: -2; yMax: 2; yZero: 0
            step: .01
            labels: bandRows.map((x, i) => i % 2 === 0 ? String(i - bandRows.length + 1) : "")
            formatValue: v => (v >= 0 ? "+" : "") + Number(v).toFixed(2) + " EV"
            editable: root.editable && bandRows.every(x => root.canWrite(x.path))
            opacity: root.moduleEnabled ? 1 : .7
            onInteractionChanged: active => root.interactionChanged(active)
            onValueEdited: (i, v) => root.editRaw(bandRows[i], v)
            navTarget.navId: root.navId(r)
            navTarget.group: root.navGroup
            onResetRequested: {
                const changes = {}
                for (const x of bandRows) changes[x.path] = x.default
                root.changesRequested(changes)
            }
        }
    }
    Component {
        id: colorRow
        ColorSwatch {
            readonly property var r: it.row
            readonly property var rgb3: it.paths.map(p => root.raw(p))
            readonly property bool known: rgb3.every(v => v !== undefined && typeof v !== "object")
            width: root.width - 28
            theme: root.theme
            label: r.label
            labelShown: !it.underCaption
            color: known ? rgb3.map(Number) : [0.5, 0.5, 0.5]
            editable: root.editable && known && it.paths.every(p => root.canWrite(p))
            opacity: !known ? .45 : root.moduleEnabled ? 1 : .7
            onInteractionChanged: active => root.interactionChanged(active)
            navTarget.navId: root.navId(r)
            navTarget.group: root.navGroup
            onColorEdited: rgb => {
                const changes = {}
                for (let i = 0; i < 3; ++i) changes[root.subst(it.paths[i])] = rgb[i]
                root.changesRequested(changes)
            }
            onResetRequested: {
                const d = Array.isArray(r.default) ? r.default : null
                if (!d) return
                const changes = {}
                for (let i = 0; i < 3; ++i) changes[root.subst(it.paths[i])] = d[i]
                root.changesRequested(changes)
            }
            // area E: darktable's picker beside a colour button (framing, watermark, invert,
            // retouch fill), in the free right column.
            Loader {
                readonly property var spec: root.tools ? root.tools.sliderTool(root.module.operation, r.field) : null
                active: !!spec
                x: parent.width + 2
                anchors.verticalCenter: parent.verticalCenter
                width: 24; height: 24
                sourceComponent: ModuleToolButtons {
                    width: 24 + 28
                    theme: root.theme
                    entries: root.toolEntries([Object.assign({}, spec, { label: "" })])
                    editable: root.editable
                    navPrefix: root.navId(r) + "/@picker"
                    navGroup: root.navGroup
                    onTriggered: (index, choice) => root.runTool(spec, choice)
                }
            }
        }
    }
    // ---- values and lists (area "Werte & Listen") ------------------------------------------
    // A runtime list or file choice (module_choices.c): the current text comes from the
    // catalog's "labels", the list from catalogModel.requestChoices, a pick is the item's
    // "set" object as one setParameters batch.
    Component {
        id: choiceListRow
        RowWrapper {
            id: wrapper
            readonly property var r: it.row
            readonly property var c: r.custom
            readonly property string key: root.catalogModel.choiceKey(root.module.operation, root.instance, c.list || r.field)
            readonly property var result: root.catalogModel.choiceResults[key] || null
            readonly property var labels: root.moduleState && root.moduleState.labels ? root.moduleState.labels : ({})
            // darktable's LUT file and name widgets carry no label, only a folder icon and the
            // tooltips "the file path ..." and "select the LUT" (lut3d.c:1747-1776): name them.
            readonly property string shownLabel: r.label || ({ "lut3d/filepath": "LUT file", "lut3d/lutname": "LUT name" })[root.module.operation + "/" + r.field] || ""
            width: root.width
            theme: root.theme
            label: shownLabel
            resetEnabled: false
            opacity: !root.moduleState ? .45 : root.moduleEnabled ? 1 : .7
            ChoiceRow {
                objectName: "choice-" + root.module.operation + "-" + wrapper.r.field
                width: parent.width
                theme: root.theme
                label: wrapper.shownLabel
                valueText: wrapper.labels[wrapper.r.field] !== undefined ? wrapper.labels[wrapper.r.field] : ""
                items: wrapper.result ? wrapper.result.items : []
                current: wrapper.result ? wrapper.result.current : -1
                more: wrapper.result ? wrapper.result.more : 0
                error: wrapper.result ? wrapper.result.error : ""
                loading: !!wrapper.result && !!wrapper.result.loading
                listed: !!wrapper.c.list
                placeholder: wrapper.c.search ? "search" : "filter"
                fileFilters: wrapper.c.browse ? wrapper.c.filters : []
                fileTitle: "Choose " + (wrapper.r.label || "a file")
                editable: root.editable && !!root.moduleState
                onRequested: query => root.catalogModel.requestChoices(root.module.operation, root.instance,
                                                                       wrapper.c.list || wrapper.r.field, query)
                onChosen: index => {
                    const item = wrapper.result && wrapper.result.items[index]
                    if (item && item.set) root.changesRequested(Object.assign({}, item.set))
                }
                onFileChosen: path => {
                    const changes = {}
                    changes[wrapper.c.browse] = path
                    root.changesRequested(changes)
                }
                navTarget.navId: root.navId(wrapper.r)
                navTarget.group: root.navGroup
                navTarget.resettable: false
            }
        }
    }
    // Free text darktable takes from an entry (watermark text and font).
    Component {
        id: textEditRow
        RowWrapper {
            id: textWrapper
            readonly property var r: it.row
            readonly property var stored: root.raw(r.path)
            width: root.width
            theme: root.theme
            label: r.label
            resetEnabled: root.editable && typeof r.default === "string"
            onResetRequested: { const ch = {}; ch[r.path] = r.default; root.changesRequested(ch) }
            implicitHeight: 26
            RowLayout {
                width: parent.width
                height: 26
                spacing: 8
                Text {
                    Layout.fillWidth: true
                    Layout.leftMargin: 8
                    text: textWrapper.r.label
                    color: textNav.current ? root.theme.accent : root.theme.ink
                    font: root.theme.settingsFont
                    elide: Text.ElideRight
                }
                TextField {
                    id: field
                    objectName: "text-" + root.module.operation + "-" + textWrapper.r.field
                    Layout.preferredWidth: parent.width * .6
                    Layout.rightMargin: 8
                    text: typeof textWrapper.stored === "string" ? textWrapper.stored : ""
                    enabled: root.editable && typeof textWrapper.stored === "string"
                    font: root.theme.textFont
                    color: root.theme.ink
                    selectByMouse: true
                    background: Rectangle { color: root.theme.well; radius: 4; border.color: field.activeFocus ? root.theme.accent : root.theme.line }
                    onEditingFinished: {
                        if (text === textWrapper.stored) return
                        const ch = {}; ch[textWrapper.r.path] = text; root.changesRequested(ch)
                    }
                    onPressed: textNav.claim()
                    NavTarget {
                        id: textNav
                        navId: root.navId(textWrapper.r)
                        label: textWrapper.r.label
                        kind: "search"
                        group: root.navGroup
                        input: field
                        activateLabel: "EDIT"
                    }
                }
            }
        }
    }
    // The colour checker patches of "color look up table".
    Component {
        id: patchesRow
        PatchGrid {
            id: grid
            objectName: "patches-" + root.module.operation
            readonly property int patchCount: Math.max(0, Number(root.raw("num_patches")) || 0)
            readonly property var sources: ["source_L", "source_a", "source_b"].map(f => root.raw(f))
            readonly property var targets: ["target_L", "target_a", "target_b"].map(f => root.raw(f))
            width: root.width - 28
            theme: root.theme
            count: patchCount
            current: Math.min(root.gui["@patch"] || 0, Math.max(0, patchCount - 1))
            colors: { const out = []; for (let i = 0; i < patchCount; ++i) out.push([0, 1, 2].map(c => Number(root.raw("@source_rgb[" + i + "][" + c + "]")) || 0)); return out }
            changed: { const out = []; for (let i = 0; i < patchCount; ++i) out.push(Number(root.raw("@changed[" + i + "]")) > 0); return out }
            lightness: Array.isArray(sources[0]) ? sources[0].map(Number) : []
            editable: root.editable && Array.isArray(sources[0]) && root.canWrite("target_L[0]")
            opacity: root.moduleEnabled ? 1 : .7
            onPatchSelected: index => root.setGui("@patch", index)
            onPatchReset: index => {
                const ch = {}
                const names = ["L", "a", "b"]
                for (let k = 0; k < 3; ++k) ch["target_" + names[k] + "[" + index + "]"] = Number(grid.sources[k][index])
                root.changesRequested(ch)
            }
            // darktable moves the following patches up and drops the last one.
            onPatchRemoved: index => {
                const ch = {}
                const fields = ["source_L", "source_a", "source_b", "target_L", "target_a", "target_b"]
                const all = fields.map(f => root.raw(f))
                if (!all.every(Array.isArray)) return
                for (let f = 0; f < fields.length; ++f)
                    for (let i = index; i < grid.patchCount - 1; ++i) ch[fields[f] + "[" + i + "]"] = Number(all[f][i + 1])
                ch["num_patches"] = grid.patchCount - 1
                if ((root.gui["@patch"] || 0) >= grid.patchCount - 1) root.setGui("@patch", Math.max(0, grid.patchCount - 2))
                root.changesRequested(ch)
            }
            navTarget.navId: root.navGroup + "/@patches"
            navTarget.group: root.navGroup
        }
    }
    // ---- module tool rows ------------------------------------------------------------------
    Component {
        id: toolsRow
        ModuleToolButtons {
            width: root.width
            theme: root.theme
            entries: root.toolEntries(it.specs)
            editable: root.editable
            navPrefix: root.navId(it.row)
            navGroup: root.navGroup
            // A value darktable shows beside the picker, e.g. exposure's input lightness.
            report: root.toolReport(it.specs)
            details: root.toolDetails(it.specs)
            opacity: root.moduleEnabled ? 1 : .7
            onTriggered: (index, choice) => root.runTool(it.specs[index], choice)
            // The answer of a find button opens as a menu, as darktable pops one up.
            readonly property var found: {
                if (!root.findList || !it.specs.some(s => s.choices === root.findList)) return null
                const r = root.catalogModel.choiceResults[root.catalogModel.choiceKey(root.module.operation, root.instance, root.findList)]
                return r && !r.loading ? r : null
            }
            onFoundChanged: if (found) { root.findList = ""; findMenu.items = found.items; findMenu.popup(0, height) }
            Menu {
                id: findMenu
                objectName: "module-find-menu-" + root.module.operation
                property var items: []
                Repeater {
                    model: findMenu.items
                    MenuItem {
                        required property var modelData
                        text: modelData.label + (modelData.detail ? "  ·  " + modelData.detail : "")
                        onTriggered: if (modelData.set) root.changesRequested(Object.assign({}, modelData.set))
                    }
                }
                MenuItem { visible: findMenu.items.length === 0; enabled: false; text: "no match" }
            }
        }
    }
    // A GUI-only slider a tool reads (exposure's target lightness).
    Component {
        id: localSliderRow
        ControlSlider {
            readonly property var r: it.row
            width: root.width
            theme: root.theme
            control: it.control
            value: root.gui[r.field] !== undefined ? root.gui[r.field] : (r.default || 0)
            editable: true
            compact: true
            moduleToggleAvailable: false
            qualifyLabel: false
            selected: root.activeControl === it.control.id
            onSelectedRequested: root.controlSelected(it.control.id)
            onEdited: v => { root.setGui(r.field, Math.max(r.min, Math.min(r.max, v))); root.guiEdited() }
            onResetRequested: { root.setGui(r.field, r.default); root.guiEdited() }
            navTarget.group: root.navGroup
        }
    }
    // A GUI-only colour (color balance rgb's checkerboard colours, kept in darktable's dt_conf keys).
    Component {
        id: localColorRow
        ColorSwatch {
            readonly property var r: it.row
            width: root.width - 28
            theme: root.theme
            label: r.label
            color: Array.isArray(root.gui[r.field]) ? root.gui[r.field] : it.fallback
            editable: true
            onInteractionChanged: active => root.interactionChanged(active)
            navTarget.navId: root.navId(r)
            navTarget.group: root.navGroup
            onColorEdited: rgb => root.setGui(r.field, rgb)
            onResetRequested: root.setGui(r.field, it.fallback)
        }
    }
    Component {
        id: histogramRow
        HistogramView {
            id: hist
            readonly property string key: root.module.operation + "/" + root.instance
            width: root.width - 28
            theme: root.theme
            histogram: root.tools ? root.tools.histogramData[key] || null : null
            channels: it.multi ? (Math.round(root.valueOrDefault({ path: "autoscale", default: 0 })) === 1 ? [root.tabValue] : [0, 1, 2]) : [0]
            markers: {
                const l = root.raw(it.levelsPath + (it.multi ? "[" + root.tabValue + "]" : ""))
                return Array.isArray(l) ? l.map(Number) : []
            }
            opacity: root.moduleEnabled ? 1 : .7
            // Refresh after edits anywhere in the pipe, at most every 400 ms.
            Timer { id: refresh; interval: 400; onTriggered: if (root.tools) root.tools.requestHistogram(root.module.operation, root.instance) }
            Connections { target: root.catalogModel; function onStatesChanged() { refresh.restart() } }
            Component.onCompleted: refresh.restart()
        }
    }
    // area E: color harmonizer's vectorscope with darktable's two-way sync
    // (colorharmonizer.c _push_to_vectorscope 814, _on_vectorscope_harmony_changed 879).
    Component {
        id: vectorscopeRow
        VectorscopeView {
            id: scope
            objectName: "vectorscope-" + root.module.operation
            readonly property bool sync: !!root.gui["@sync_to_vectorscope"]
            readonly property int rule: Math.round(Number(root.raw("rule")) || 0)
            readonly property int nodes: Math.max(2, Math.min(4, Math.round(Number(root.raw("num_custom_nodes")) || 2)))
            readonly property var customHues: { const out = []; for (let i = 0; i < nodes; ++i) out.push(Number(root.raw("@custom_hue[" + i + "]")) || 0); return out }
            // What the module would push: its rule and anchor, or a custom guide.
            readonly property var wanted: rule === 9 ? { type: 0, rotation: -1 }
                                                     : { type: rule + 1, rotation: Math.round(Number(root.raw("@anchor_hue")) || 0) % 360 }
            width: root.width - 28
            theme: root.theme
            editable: root.editable
            png: root.tools && root.tools.vectorscope ? root.tools.vectorscope.png || "" : ""
            guide: root.tools.harmonyGuide
            customAngles: sync && rule === 9 && root.moduleEnabled ? customHues.map(h => h / 360) : []
            function push() {
                const g = root.tools.harmonyGuide
                if (!sync || !root.moduleEnabled) return
                if (g.type !== wanted.type || (wanted.type > 0 && g.rotation !== wanted.rotation))
                    root.tools.setHarmonyGuide(wanted.type, wanted.rotation < 0 ? g.rotation : wanted.rotation, g.width)
            }
            onWantedChanged: push()
            onSyncChanged: push()
            onGuideEdited: (type, rotation, w) => {
                root.tools.setHarmonyGuide(type, rotation, w)
                if (sync && root.moduleEnabled && type > 0)
                    root.changesRequested({ "rule": type - 1, "@anchor_hue": rotation })
            }
            onCustomRotated: turns => {
                if (!sync || !root.moduleEnabled) return
                const ch = {}
                for (let i = 0; i < customHues.length; ++i) ch["@custom_hue[" + i + "]"] = ((customHues[i] + turns * 360) % 360 + 360) % 360
                root.changesRequested(ch)
            }
            Timer { id: scopeRefresh; interval: 600; onTriggered: if (root.tools) root.tools.requestVectorscope() }
            Connections { target: root.catalogModel; function onStatesChanged() { scopeRefresh.restart() } }
            Component.onCompleted: { scopeRefresh.restart(); push() }
        }
    }
    // area E: retouch's wavelet decompose bar (WaveletBar).
    Component {
        id: waveletBarRow
        WaveletBar {
            readonly property var forms: { const f = root.raw("rt_forms"); return Array.isArray(f) ? f : [] }
            readonly property var used: forms.map((f, i) => ({ i: i, id: Number(f.formid), scale: Number(f.scale) })).filter(f => f.id > 0)
            width: root.width - 28
            theme: root.theme
            numScales: Math.round(Number(root.raw("num_scales")) || 0)
            currScale: Math.round(Number(root.raw("curr_scale")) || 0)
            mergeFrom: Math.round(Number(root.raw("merge_from_scale")) || 0)
            formScales: used.map(f => f.scale)
            formSlots: used.map(f => f.i)
            editable: root.editable && root.canWrite("num_scales")
            opacity: root.moduleEnabled ? 1 : .7
            onEdited: changes => root.changesRequested(changes)
            navTarget.navId: root.navGroup + "/@wavelets"
            navTarget.group: root.navGroup
        }
    }
    // area E: zone system's zone bar (ZoneBar).
    Component {
        id: zoneBarRow
        Column {
            width: root.width - 28
            spacing: 2
            Text { text: "zones: " + Math.round(Number(root.raw("size")) || 10) + "  (scroll to change)"; color: root.theme.muted; font: root.theme.textFont }
            ZoneBar {
                objectName: "zonebar-" + root.module.operation
                width: parent.width
                theme: root.theme
                size: Math.round(Number(root.raw("size")) || 10)
                zones: { const z = root.raw("zone"); return Array.isArray(z) ? z.map(Number) : [] }
                editable: root.editable && root.canWrite("size") && root.canWrite("zone[0]")
                opacity: root.moduleEnabled ? 1 : .7
                onInteractionChanged: active => root.interactionChanged(active)
                onEdited: changes => root.changesRequested(changes)
                navTarget.navId: root.navGroup + "/@zones"
                navTarget.group: root.navGroup
            }
        }
    }
    // area E: the a/b panels of monochrome and color correction (ColorGrid).
    Component {
        id: colorGridRow
        ColorGrid {
            readonly property var names: root.module.operation === "monochrome" ? ["a", "b", "size"] : ["loa", "lob", "hia", "hib", "saturation"]
            objectName: "colorgrid-" + root.module.operation
            width: Math.min(root.width - 28, 260)
            theme: root.theme
            mode: root.module.operation === "monochrome" ? "monochrome" : "correction"
            values: { const out = {}; for (const n of names) out[n] = root.raw(n); return out }
            editable: root.editable && names.every(n => root.canWrite(n))
            opacity: root.moduleEnabled ? 1 : .7
            onInteractionChanged: active => root.interactionChanged(active)
            onEdited: changes => root.changesRequested(changes)
            onResetRequested: list => {
                const ch = {}
                for (const n of list) { const d = root.module.rows.find(x => x.field === n); ch[n] = d && d.default !== null ? d.default : (n === "size" ? 2 : n === "saturation" ? 1 : 0) }
                root.changesRequested(ch)
            }
            navTarget.navId: root.navGroup + "/@grid"
            navTarget.group: root.navGroup
        }
    }
    // area E: color mapping's source and target clusters (colormapping.c:1000-1001).
    Component {
        id: clustersRow
        Column {
            width: root.width - 28
            spacing: 4
            opacity: root.moduleEnabled ? 1 : .7
            Repeater {
                model: [{ title: "source clusters:", mean: "source_mean", sigma: "source_var" },
                        { title: "target clusters:", mean: "target_mean", sigma: "target_var" }]
                Column {
                    required property var modelData
                    width: parent.width
                    spacing: 2
                    Text { text: modelData.title; color: root.theme.muted; font: root.theme.textFont }
                    ClusterPreview {
                        objectName: "clusters-" + modelData.mean
                        width: parent.width
                        theme: root.theme
                        count: Math.max(0, Math.min(5, Number(root.raw("n")) || 0))
                        means: root.raw(modelData.mean) || []
                        sigmas: root.raw(modelData.sigma) || []
                    }
                }
            }
        }
    }
}
