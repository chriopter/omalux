import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

// The rows of one darktable module as described by omalux/design/layout.json, drawn with the
// same controls as the curated block. Primary rows show while the module is collapsed, detail
// rows when it is expanded and advanced rows behind "more". Rows that need something Omalux
// cannot do yet (pickers, drawn shapes, converted values) collapse into one muted notice.
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

    readonly property bool moduleEnabled: !!moduleState && moduleState.enabled
    property int tabIndex: 0
    property var gui: ({})
    readonly property int tabValue: module.tabs && module.tabs.length ? tabIndex : (gui["@tab"] || 0)

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
            if (r.widget === "notice") it = Object.assign(base(r), { kind: "notice", text: r.label })
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
        // Section captions where darktable has them.
        const withSections = []
        let section = null, tab = null
        for (const it of out) {
            if (it.section && (it.section !== section || it.tab !== tab))
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
    function canWrite(path) { return root.catalogModel.writable(subst(path)) }
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
            active: shown
            property var it: modelData
            sourceComponent: ({ slider: sliderRow, choice: choiceRow, "switch": switchRow, local: localRow, text: textRow,
                                notice: noticeRow, section: sectionRow, curve: curveRow, bars: barsRow, bands: bandsRow,
                                color: colorRow, channels: channelsRow })[modelData.kind] || noticeRow
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
            control: it.control
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
                    for (let i = 0; i < count; ++i) list.push({ value: i, label: String(i + 1) })
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
            onEdited: v => root.setGui(r.path, v)
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
            onEdited: v => root.setGui(r.path, v)
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
            title: "simple"
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
        }
    }
}
