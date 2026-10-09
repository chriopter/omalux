import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import "Scroll.js" as Scroll

// darktable's per-module blend section (develop/blend_gui.c) for one module instance:
// mask mode, blend mode and order, fulcrum, opacity, the drawn, raster and parametric masks
// and the mask refinement. Labels, units and ranges come from
// omalux/design/layout-blending.json (catalogModel.blendRows); what is offered and shown follows
// the module's blend state in the catalog (moduleState.blend, see blending.c). The component
// never writes anything itself: edits leave through changesRequested as { "blend.…": value }.
Column {
    id: root
    required property var theme
    required property var moduleState
    required property var catalogModel
    property var overrides: ({})
    property string overridePrefix: ""
    property bool editable: true
    property bool moduleEnabled: true
    property string navGroup: ""
    signal changesRequested(var changes)
    signal interactionChanged(bool active)
    // Drawn shapes are made on the image (masks_api.h); the composition decides who draws them.
    signal drawnShapeRequested(int shape)

    readonly property var blend: moduleState ? moduleState.blend : null
    visible: !!blend
    spacing: 4

    // ---- values --------------------------------------------------------------------------
    function row(field) { return root.catalogModel.blendRows.find(r => r.field === field) || ({ label: field }) }
    function raw(member) {
        const o = root.overrides[root.overridePrefix + "blend." + member]
        if (o !== undefined) return o
        if (!root.blend) return undefined
        const m = /^([a-z_]+)\[(\d+)\]$/.exec(member)
        const v = m ? (root.blend[m[1]] || [])[Number(m[2])] : root.blend[member]
        return typeof v === "boolean" ? (v ? 1 : 0) : v
    }
    function num(member, fallback) { const v = raw(member); return v === undefined || v === null ? fallback : Number(v) }
    function set(member, value) { const c = {}; c["blend." + member] = value; root.changesRequested(c) }
    function control(field, member) {
        const r = row(field), f = r.factor || 1, o = r.offset || 0
        const a = r.min * f + o, b = r.max * f + o
        const sa = (r.soft_min !== null && r.soft_min !== undefined ? r.soft_min : r.min) * f + o
        const sb = (r.soft_max !== null && r.soft_max !== undefined ? r.soft_max : r.max) * f + o
        const digits = r.digits !== undefined && r.digits !== null ? r.digits : 2
        return { id: root.navGroup + "/blend." + (member || field), label: r.label, unit: r.unit || "", colors: "",
                 decimals: digits, step: digits > 0 ? Math.pow(10, -digits) : 1,
                 minimum: Math.min(a, b), maximum: Math.max(a, b), softMinimum: Math.min(sa, sb), softMaximum: Math.max(sa, sb),
                 factor: f, offset: o, section: "blending", rawDefault: r.default }
    }

    readonly property int maskMode: num("mask_mode", 0)
    readonly property bool maskEnabled: (maskMode & 1) !== 0
    readonly property bool drawn: maskEnabled && (maskMode & 2) !== 0
    readonly property bool parametric: maskEnabled && (maskMode & 4) !== 0 && !!blend && blend.parametric
    readonly property bool raster: maskEnabled && (maskMode & 8) !== 0
    readonly property int csp: blend ? blend.csp : 0
    readonly property int blendMode: num("blend_mode", 24)
    // _blendif_blend_parameter_enabled
    readonly property bool fulcrum: csp === 4 && [6, 4, 7, 37, 38, 39, 33, 34, 35].indexOf(blendMode) >= 0
    readonly property bool rawSpace: csp === 1

    // ---- parametric mask -----------------------------------------------------------------
    property int tab: 0
    // area E: the alternative marker scale per channel tab and slider (blend_gui.c altmode[tab][in_out]).
    property var altModes: ({})
    function setAltMode(inOut, active) { const m = Object.assign({}, altModes); m[tab + "/" + inOut] = active; altModes = m }
    property bool outputsRequested: false
    readonly property bool outputsShown: outputsRequested || (!!blend && blend.outputs_used)
    readonly property var channels: (blend && blend.channels) || []
    readonly property var channel: channels.length ? channels[Math.min(tab, channels.length - 1)] : null
    onCspChanged: tab = 0
    function negative(ch) {
        const o = root.overrides[root.overridePrefix + "blend.polarity[" + ch + "]"]
        if (o !== undefined) return o > 0.5
        return !!root.blend && ((root.blend.blendif >>> (ch + 16)) & 1) === 1
    }
    function range(ch) { return [0, 1, 2, 3].map(k => num("blendif_parameters[" + (4 * ch + k) + "]", k < 2 ? 0 : 1)) }
    function boostOf(ch) { return root.blend && root.blend.boost_factors ? Number(root.blend.boost_factors[ch]) : 0 }
    // darktable's colour stops behind each channel (blend_gui.c _gradient_*).
    function hex(r, g, b) { return Qt.rgba(r, g, b, 1).toString() }
    readonly property var gradients: {
        const ramp = (r, g, b) => [[0, hex(0, 0, 0)], [.125, hex(r / 8, g / 8, b / 8)], [.25, hex(r / 4, g / 4, b / 4)],
                                   [.5, hex(r / 2, g / 2, b / 2)], [1, hex(r, g, b)]]
        return {
            gray: ramp(.5, .5, .5), red: ramp(.75, 0, 0), green: ramp(0, .75, 0), blue: ramp(0, 0, .75),
            a: [[0, hex(.0112790, .75, .5609999)], [.25, hex(.2888855, .75, .6318934)], [.375, hex(.4872486, .75, .6825501)],
                [.5, hex(.75, .7499399, .7496052)], [.625, hex(.75, .5054633, .5676756)], [.75, hex(.75, .3423850, .4463195)],
                [1, hex(.75, .1399815, .2956989)]],
            b: [[0, hex(.0162050, .1968228, .75)], [.25, hex(.2027354, .3168822, .75)], [.375, hex(.3645722, .4210476, .75)],
                [.5, hex(.6167146, .5833379, .75)], [.625, hex(.75, .6172369, .5412091)], [.75, hex(.75, .5590797, .3071980)],
                [1, hex(.75, .4963975, .0549797)]],
            chroma: [[0, hex(.5, .5, .5)], [.125, hex(.5, .4375, .5)], [.25, hex(.5, .375, .5)], [.5, hex(.5, .25, .5)], [1, hex(.5, 0, .5)]],
            lchHue: [[0, hex(.75, .2200405, .4480174)], [.104, hex(.75, .2475123, .2488547)], [.2, hex(.75, .3921083, .2017670)],
                     [.295, hex(.75, .7440329, .3011876)], [.377, hex(.3813996, .75, .3799668)], [.503, hex(.0747526, .75, .7489037)],
                     [.65, hex(.0282981, .3736209, .75)], [.803, hex(.2583821, .2591069, .75)], [.928, hex(.75, .2788102, .7492077)],
                     [1, hex(.75, .2200405, .4480174)]],
            hslHue: [[0, hex(.75, .25, .25)], [.167, hex(.75, .75, .25)], [.333, hex(.25, .75, .25)], [.5, hex(.25, .75, .75)],
                     [.667, hex(.25, .25, .75)], [.833, hex(.75, .25, .75)], [1, hex(.75, .25, .25)]],
            jzHue: [[0, hex(.75, .1946971, .3697612)], [.082, hex(.75, .2278141, .2291548)], [.15, hex(.75, .3132381, .1653960)],
                    [.275, hex(.7483232, .75, .1939316)], [.378, hex(.2642865, .75, .2642768)], [.57, hex(.0233180, .7493543, .75)],
                    [.65, hex(.1119025, .5116763, .75)], [.762, hex(.3331225, .3337235, .75)], [.883, hex(.74647, .2754816, .75)],
                    [1, hex(.75, .1946971, .3697612)]]
        }
    }
    function stopsOf(c) {
        if (!c) return []
        const lab = { L: "gray", a: "a", b: "b", C: "chroma", h: "lchHue" }
        const rgb = { g: "gray", R: "red", G: "green", B: "blue", H: "hslHue", S: "chroma", L: "gray", Jz: "gray", Cz: "chroma", hz: "jzHue" }
        return root.gradients[(root.csp === 2 ? lab : rgb)[c.label]] || []
    }
    function rangeEdited(ch, values) {
        const c = {}
        for (let k = 0; k < 4; ++k) c["blend.blendif_parameters[" + (4 * ch + k) + "]"] = values[k]
        root.changesRequested(c)
    }
    // Raster masks offered by earlier modules (_raster_combo_populate).
    readonly property var rasterSources: root.catalogModel.rasterSources(root.moduleState)
    readonly property int rasterIndex: {
        if (!blend || !blend.raster_mask_source) return -1
        return rasterSources.findIndex(s => s.operation === blend.raster_mask_source && s.instance === blend.raster_mask_instance
                                            && s.id === blend.raster_mask_id)
    }

    // ---- rows ----------------------------------------------------------------------------
    component Caption: Text {
        width: root.width - 28
        topPadding: 6
        color: root.theme.muted
        font: root.theme.textFont
    }
    component ChoiceRow: RowWrapper {
        id: choiceRow
        property string field
        property string member: field
        property var options: []
        property real value: 0
        property real resetValue: 0
        signal chosen(real value)
        width: root.width
        theme: root.theme
        label: root.row(field).label
        resetEnabled: root.editable
        onResetRequested: chosen(resetValue)
        opacity: root.moduleEnabled ? 1 : .7
        ControlChoice {
            width: parent.width
            theme: root.theme
            label: choiceRow.label
            labelFont: root.theme.settingsFont
            labelColor: root.theme.ink
            options: choiceRow.options
            value: choiceRow.value
            editable: root.editable && choiceRow.options.length > 0
            onEdited: v => choiceRow.chosen(v)
            onResetRequested: choiceRow.chosen(choiceRow.resetValue)
            navTarget.navId: root.navGroup + "/blend." + choiceRow.member
            navTarget.group: root.navGroup
        }
    }
    component SwitchRow: RowWrapper {
        id: switchRow
        property string field
        property string member
        property string text: root.row(field).label
        width: root.width
        theme: root.theme
        label: text
        resetEnabled: root.editable
        onResetRequested: root.set(member, 0)
        opacity: root.moduleEnabled ? 1 : .7
        ControlSwitch {
            width: parent.width
            theme: root.theme
            label: switchRow.text
            labelFont: root.theme.settingsFont
            labelColor: root.theme.ink
            value: root.num(switchRow.member, 0)
            editable: root.editable
            onEdited: v => root.set(switchRow.member, v)
            onResetRequested: root.set(switchRow.member, 0)
            navTarget.navId: root.navGroup + "/blend." + switchRow.member
            navTarget.group: root.navGroup
        }
    }
    component SliderRow: ControlSlider {
        id: sliderRow
        property string field
        property string member: field
        readonly property var c: control
        width: root.width
        theme: root.theme
        control: root.control(field, member)
        value: root.num(member, c.rawDefault) * c.factor + c.offset
        editable: root.editable
        compact: true
        moduleToggleAvailable: false
        qualifyLabel: false
        opacity: root.moduleEnabled ? 1 : .7
        onInteractionChanged: active => root.interactionChanged(active)
        onEdited: v => root.set(member, Math.max(root.row(field).min, Math.min(root.row(field).max, (v - c.offset) / c.factor)))
        onResetRequested: root.set(member, c.rawDefault)
        navTarget.group: root.navGroup
        defaultValue: c.rawDefault !== undefined && c.rawDefault !== null ? c.rawDefault * c.factor + c.offset : undefined
    }
    // A small action of the section ("add circle", "reset blend mask settings"): an outlined
    // chip, so it reads as a button among the captions.
    component TextButton: ToolButton {
        id: textButton
        property string label
        property alias nav: buttonNav
        signal triggered()
        leftPadding: 7; rightPadding: 7; topPadding: 3; bottomPadding: 3
        hoverEnabled: true
        enabled: root.editable
        onClicked: { buttonNav.claim(); triggered() }
        Accessible.name: label
        NavTarget { id: buttonNav; navId: root.navGroup + "/blend/" + textButton.label; label: textButton.label; group: root.navGroup
                    enabled: textButton.enabled; onActivate: textButton.triggered() }
        contentItem: Text {
            text: textButton.label
            color: textButton.visualFocus || buttonNav.current ? root.theme.accent
                 : textButton.hovered ? root.theme.ink : root.theme.muted
            font: root.theme.textFont
        }
        background: Rectangle {
            radius: 4
            color: textButton.pressed ? root.theme.active : textButton.hovered ? root.theme.hover : "transparent"
            border.width: 1
            border.color: textButton.visualFocus || buttonNav.current ? root.theme.accent : root.theme.line
        }
    }

    // area E: darktable's "display mask and/or color channel" and "temporarily switch off blend
    // mask" toggles beside the mask modes (blend_gui.c:3508-3528), shown for a real mask only
    // (3121); view state, not history (engine blend_display.c).
    readonly property var displayState: root.tools && root.moduleState ? root.tools.blendDisplayOf(root.moduleState.operation, root.moduleState.instance) : null
    // The parametric mask's pickers (blend_gui.c:2578 "show color", :2588 "set range"),
    // run through ModuleTools (engine tools_blend.c) for the shown channel.
    readonly property var tools: root.catalogModel && root.catalogModel.tools ? root.catalogModel.tools : null
    readonly property var pickerSpecs: [
        { tool: "blend_show", kind: "pointarea", keepActive: true, label: "show color",
          hint: "pick GUI color from image\nctrl+click or right-click to select an area" },
        { tool: "blend_set_range", kind: "area", label: "set range",
          hint: "set the range based on an area from the image\ndrag to use the input image\nctrl+drag to use the output image" }]
    function pickerGui() {
        return { tab: Math.min(root.tab, Math.max(0, root.channels.length - 1)), channel_in: root.channel ? root.channel.in : 0,
                 channel_out: root.channel ? root.channel.out : 4, outputs_shown: root.outputsShown ? 1 : 0 }
    }
    // The last sample of the active blend picker of this module, per slider.
    readonly property var pickerMarkers: {
        const t = root.tools, s = root.moduleState
        if (!t || !s || !t.active || t.active.operation !== s.operation || t.active.instance !== s.instance
                || t.active.tool.indexOf("blend_") !== 0) return null
        const r = t.results[t.key(s.operation, s.instance, t.active.tool)]
        return r && r.blendMarker ? r.blendMarker : null
    }
    // Another channel tab: an active picker samples that channel instead.
    onTabChanged: if (root.tools && root.moduleState && root.tools.active && root.tools.active.tool.indexOf("blend_") === 0)
                      root.tools.updateGui(root.moduleState.operation, root.moduleState.instance, root.pickerGui())
    // dt_iop_gui_update_blending: refinement for drawn or parametric masks, or a raster mask;
    // a module blending in raw data keeps only the blur.
    readonly property bool refine: (root.maskEnabled && (root.drawn || root.parametric)) || root.raster
    // The section is one collapsible row of the module: closed it names the mask mode in use,
    // so "off" costs one line. It starts open when the module blends through a mask, and opens
    // when a mask is switched on from elsewhere.
    property bool open: false
    Component.onCompleted: open = maskEnabled
    onMaskEnabledChanged: if (maskEnabled) open = true
    readonly property string maskModeLabel: {
        const o = (root.row("mask_mode").values || []).find(v => v.value === root.maskMode)
        return o ? o.label : ""
    }
    SectionRow {
        objectName: "blend-section-" + root.navGroup
        width: root.width
        theme: root.theme
        label: "blending"
        summary: root.maskModeLabel
        open: root.open
        navTarget.navId: root.navGroup + "/@blending"
        navTarget.group: root.navGroup
        onRequested: root.open = !root.open
    }
    Column {
        id: content
        visible: root.open
        width: root.width
        spacing: 4
        ChoiceRow {
            id: maskModeRow
            objectName: "blend-mask-mode-" + root.navGroup
            field: "mask_mode"
            // dt_iop_gui_init_blending: drawn and raster need mask support, parametric a Lab or RGB module.
            options: (root.row("mask_mode").values || []).filter(o => o.value === 0 || o.value === 1
                         || (o.value === 3 && root.blend && root.blend.masks)
                         || (o.value === 5 && root.blend && root.blend.parametric)
                         || (o.value === 7 && root.blend && root.blend.masks && root.blend.parametric)
                         || (o.value === 9 && root.blend && root.blend.masks))
            value: root.maskMode
            onChosen: v => { root.set("mask_mode", v); revealRows.target = maskModeRow; revealRows.restart() }
        }
        // A new mask mode adds rows below the choice: scroll them into view, keeping the choice
        // itself on screen, so the choice visibly did something.
        // Rows are built over a few frames (later under load): follow the section's height for a
        // moment instead of guessing when it is complete.
        Timer {
            id: revealRows
            property Item target: null
            property real until: 0
            interval: 120
            onTriggered: { until = Date.now() + 1500; if (target) Scroll.reveal(root, content.y + target.y, root.height) }
        }
        onHeightChanged: if (revealRows.target && Date.now() < revealRows.until) Scroll.reveal(root, content.y + revealRows.target.y, root.height)
        ModuleToolButtons {
            objectName: "blend-display-" + root.navGroup
            visible: (root.maskMode & ~1) !== 0 && !!root.tools && root.tools.supported["*/blend_display"] === true
            width: root.width
            theme: root.theme
            editable: root.editable
            navPrefix: root.navGroup + "/blend/display"
            navGroup: root.navGroup
            entries: [{ label: "display mask", kind: "button", active: !!root.displayState && root.displayState.mask,
                        hint: "display mask and/or color channel.\nctrl+click to display mask,\nshift+click to display channel.\nhover over parametric mask slider to select channel for display" },
                      { label: "switch off mask", kind: "button", active: !!root.displayState && root.displayState.suppress,
                        hint: "temporarily switch off blend mask.\nonly for module in focus" }]
            onTriggered: (index, choice) => {
                const s = root.displayState || { mask: false, suppress: false }
                root.tools.setBlendDisplay(root.moduleState.operation, root.moduleState.instance,
                                           index === 0 ? !s.mask : s.mask, index === 1 ? !s.suppress : s.suppress)
            }
        }
        // The blending options menu: colour space of the mask and blend (_blendif_options_callback).
        ChoiceRow {
            visible: root.maskEnabled && !!root.blend && root.blend.parametric && [2, 3, 4].indexOf(root.blend.default_cst) >= 0
            field: "blend_cst"
            label: "blend colorspace"
            options: (root.blend && root.blend.default_cst === 2 ? [{ value: 2, label: "Lab" }] : [])
                     .concat([{ value: 3, label: "RGB (display)" }, { value: 4, label: "RGB (scene)" }])
            value: root.num("blend_cst", 0)
            resetValue: 0
            onChosen: v => root.set("blend_cst", v)
        }

        Caption { visible: root.maskEnabled; text: "blend mask" }
        ChoiceRow {
            visible: root.maskEnabled
            field: "blend_mode"
            options: ((root.blend && root.blend.blend_modes) || []).map(m => ({ value: m.value, label: m.label }))
            value: root.blendMode
            resetValue: 24
            onChosen: v => root.set("blend_mode", v)
        }
        SwitchRow { visible: root.maskEnabled; field: "blend_reverse"; member: "reverse" }
        SliderRow { visible: root.maskEnabled && root.fulcrum; field: "blend_parameter" }
        SliderRow { objectName: "blend-opacity-" + root.navGroup; visible: root.maskEnabled; field: "opacity" }

        Caption { visible: root.drawn; text: "drawn mask" }
        RowLayout {
            visible: root.drawn
            width: root.width - 28
            spacing: 8
            Text { Layout.fillWidth: true; text: root.row("mask_id").label; color: root.theme.ink; font: root.theme.settingsFont }
            Text {
                readonly property int shapes: root.blend ? root.blend.drawn_shapes : 0
                text: shapes > 0 ? shapes + (shapes === 1 ? " shape used" : " shapes used") : "no mask used"
                color: root.theme.muted; font: root.theme.settingsFont
            }
        }
        SwitchRow { visible: root.drawn; field: "drawn_polarity"; member: "drawn_polarity" }
        Flow {
            visible: root.drawn && !!root.blend && root.blend.drawn_available
            width: root.width - 28
            spacing: 6
            Repeater {
                // blend_gui.c dt_iop_gui_init_masks: the shape buttons (DT_MASKS_* types).
                model: [{ type: 16, label: "add gradient" }, { type: 2, label: "add path" }, { type: 32, label: "add ellipse" },
                        { type: 1, label: "add circle" }, { type: 64, label: "add brush" }]
                TextButton {
                    required property var modelData
                    label: modelData.label
                    onTriggered: root.drawnShapeRequested(modelData.type)
                }
            }
        }
        // area E: darktable's mask manager for this module's shapes (MaskManagerView, mask_manager.c).
        MaskManagerView {
            id: maskManager
            objectName: "mask-manager-" + root.navGroup
            visible: root.drawn && !!root.tools && root.tools.supported["*/masks"] === true
            width: root.width - 28
            theme: root.theme
            editable: root.editable
            navGroup: root.navGroup
            readonly property string key: root.moduleState ? root.tools.key(root.moduleState.operation, root.moduleState.instance, "masks") : ""
            report: root.tools && key && root.tools.results[key] ? root.tools.results[key].masks || null : null
            readonly property int shapes: root.blend ? root.blend.drawn_shapes : 0
            function refresh() { if (visible && root.moduleState) root.tools.send(root.moduleState.operation, root.moduleState.instance, "masks", null, { action: "list" }) }
            onShapesChanged: refresh()
            onVisibleChanged: refresh()
            // A shape selected, moved or resized on the photo: the properties follow.
            readonly property string shapesRevision: visible && root.tools ? root.tools.shapesRevision : ""
            onShapesRevisionChanged: if (visible) followShapes.restart()
            Timer { id: followShapes; interval: 200; onTriggered: maskManager.refresh() }
            onRequested: (action, args) => root.tools.send(root.moduleState.operation, root.moduleState.instance, "masks", null,
                                                           Object.assign({ action: action }, args))
        }
        ModuleNotice {
            visible: root.drawn && !!root.blend && !root.blend.drawn_available
            width: root.width - 28
            theme: root.theme
            text: root.row("@notice").label
        }

        Caption { visible: root.raster; text: "raster mask" }
        ChoiceRow {
            visible: root.raster
            field: "raster_mask"
            options: [{ value: -1, label: "no mask used" }].concat(root.rasterSources.map((s, i) => ({ value: i, label: s.label })))
            value: root.rasterIndex
            resetValue: -1
            onChosen: v => {
                if (v < 0) { root.set("raster_mask", -1); return }
                const s = root.rasterSources[v]
                root.set("raster_mask@" + s.operation + "/" + s.instance, s.id)
            }
        }
        SwitchRow { visible: root.raster; field: "raster_mask_invert"; member: "raster_mask_invert" }

        Caption { visible: root.parametric; text: "parametric mask" }
        ChannelChooser {
            visible: root.parametric
            width: root.width - 28
            theme: root.theme
            options: root.channels.map((c, i) => ({ label: c.label, value: i }))
            current: Math.min(root.tab, Math.max(0, root.channels.length - 1))
            editable: root.editable
            onChosen: v => root.tab = v
            navTarget.navId: root.navGroup + "/blend/channel"
            navTarget.group: root.navGroup
        }
        BlendifRange {
            visible: root.parametric && root.outputsShown && !!root.channel
            width: root.width - 28
            theme: root.theme
            label: root.row("blendif_output").label
            tooltip: "adjustment based on unblended output of this module"
            readonly property int ch: root.channel ? root.channel.out : 4
            pickerMarker: root.pickerMarkers ? root.pickerMarkers.output || null : null
            values: root.range(ch)
            negative: root.negative(ch)
            stops: root.stopsOf(root.channel)
            scale: root.channel ? root.channel.scale : "default"
            altScale: ({ ab: "zoom", hue: "" })[scale] ?? "log"
            altActive: !!root.altModes[root.tab + "/" + (label === root.row("blendif_output").label ? 1 : 0)]
            onAlternativeRequested: active => root.setAltMode(label === root.row("blendif_output").label ? 1 : 0, active)
            boost: Math.pow(2, root.boostOf(ch))
            increment: root.channel ? root.channel.increment : .01
            editable: root.editable
            opacity: root.moduleEnabled ? 1 : .7
            onValuesEdited: values => root.rangeEdited(ch, values)
            onPolarityToggled: neg => root.set("polarity[" + ch + "]", neg ? 1 : 0)
            onResetRequested: root.set("reset_channel[" + ch + "]", 1)
            onInteractionChanged: active => root.interactionChanged(active)
            navTarget.navId: root.navGroup + "/blend/output"
            navTarget.group: root.navGroup
        }
        BlendifRange {
            objectName: "blendif-input-" + root.navGroup
            visible: root.parametric && !!root.channel
            width: root.width - 28
            theme: root.theme
            label: root.row("blendif_input").label
            tooltip: "adjustment based on input received by this module"
            readonly property int ch: root.channel ? root.channel.in : 0
            pickerMarker: root.pickerMarkers ? root.pickerMarkers.input || null : null
            values: root.range(ch)
            negative: root.negative(ch)
            stops: root.stopsOf(root.channel)
            scale: root.channel ? root.channel.scale : "default"
            altScale: ({ ab: "zoom", hue: "" })[scale] ?? "log"
            altActive: !!root.altModes[root.tab + "/" + (label === root.row("blendif_output").label ? 1 : 0)]
            onAlternativeRequested: active => root.setAltMode(label === root.row("blendif_output").label ? 1 : 0, active)
            boost: Math.pow(2, root.boostOf(ch))
            increment: root.channel ? root.channel.increment : .01
            editable: root.editable
            opacity: root.moduleEnabled ? 1 : .7
            onValuesEdited: values => root.rangeEdited(ch, values)
            onPolarityToggled: neg => root.set("polarity[" + ch + "]", neg ? 1 : 0)
            onResetRequested: root.set("reset_channel[" + ch + "]", 1)
            onInteractionChanged: active => root.interactionChanged(active)
            navTarget.navId: root.navGroup + "/blend/input"
            navTarget.group: root.navGroup
        }
        // _blendop_blendif_boost_factor_callback: shown value = stored factor − the channel's offset.
        SliderRow {
            id: boostRow
            visible: root.parametric && !!root.channel
            field: "boost_factor"
            member: "boost_factor[" + (root.channel ? root.channel.in : 0) + "]"
            value: root.channel && root.channel.boost ? root.boostOf(root.channel.in) - root.channel.boost_offset : 0
            editable: root.editable && !!root.channel && root.channel.boost
        }
        ChoiceRow {
            visible: root.parametric
            field: "mask_combine"
            options: (root.row("mask_combine").values || [])
            value: root.num("mask_combine", 0)
            onChosen: v => root.changesRequested({ "blend.mask_combine": v, "blend.output_channels_shown": root.outputsShown ? 1 : 0 })
        }
        // "show output channels" / "reset and hide output channels" of the blending options menu.
        RowWrapper {
            visible: root.parametric
            width: root.width
            theme: root.theme
            label: "show output channels"
            resetEnabled: false
            ControlSwitch {
                width: parent.width
                theme: root.theme
                label: "show output channels"
                labelFont: root.theme.settingsFont
                labelColor: root.theme.ink
                value: root.outputsShown ? 1 : 0
                editable: root.editable
                onEdited: v => {
                    if (v > 0.5) { root.outputsRequested = true; return }
                    root.outputsRequested = false
                    if (root.blend && root.blend.outputs_used) root.set("clean_output_channels", 1)
                }
                navTarget.navId: root.navGroup + "/blend/outputs"
                navTarget.group: root.navGroup
                navTarget.resettable: false
            }
        }
        Flow {
            visible: root.parametric
            width: root.width - 28
            spacing: 6
            TextButton { label: "reset blend mask settings"; onTriggered: root.set("reset_parametric", 1) }
            TextButton { label: "invert all channel's polarities"; onTriggered: root.set("invert_all", 1) }
        }
        ModuleToolButtons {
            visible: root.parametric && !!root.tools && root.tools.supported["*/blend_set_range"] === true
            width: root.width
            theme: root.theme
            editable: root.editable
            navPrefix: root.navGroup + "/blend/pickers"
            navGroup: root.navGroup
            entries: root.pickerSpecs.map(p => ({ label: p.label, kind: p.kind, hint: p.hint,
                                                  active: !!root.tools && !!root.moduleState
                                                          && root.tools.isActive(root.moduleState.operation, root.moduleState.instance, p.tool) }))
            onTriggered: (index, choice) => root.tools.toggle(root.moduleState.operation, root.moduleState.instance,
                                                              root.pickerSpecs[index], root.pickerGui())
        }

        Caption { visible: root.refine; text: "mask refinement" }
        SliderRow { visible: root.refine && !root.rawSpace && !!root.blend && root.blend.raw; field: "details" }
        ChoiceRow {
            visible: root.refine && !root.rawSpace
            field: "feathering_guide"
            options: root.row("feathering_guide").values || []
            value: root.num("feathering_guide", 5)
            resetValue: 5
            onChosen: v => root.set("feathering_guide", v)
        }
        SliderRow { visible: root.refine && !root.rawSpace; field: "feathering_radius" }
        SliderRow { visible: root.refine; field: "blur_radius" }
        SliderRow { visible: root.refine && !root.rawSpace; field: "brightness" }
        SliderRow { visible: root.refine && !root.rawSpace; field: "contrast" }
    }
}
