import QtQuick
import QtCore

// darktable's colour pickers and module buttons (native: engine/module_tools.h), as data and
// state for the generated rows and the photo viewport. Non-visual; the sidebar composition
// instantiates it, hands it to ModuleCatalog as `tools` and runs `runRequested` on the backend.
//
// - rowTools: layout rows (operation → row field) that become a picker or a button, and
//   GUI-only rows a tool reads (`local`, e.g. a target lightness).
// - sliderTools: darktable's picker attached to a slider (its "quad" button), by row field.
// - histograms: graph rows that draw the module input histogram behind their handles.
// Picker kinds follow darktable: area (drag a box; a new picker starts with 2–98 % of the
// image), point (click) and pointarea (a click picks a point, a drag an area). A picker stays
// active until it is toggled again, cancelled, another one starts or a parameter of its module
// is edited (dt_iop_gui_changed resets it, except darktable's "keep-active" pickers); while it
// is active every box drawn on the photo applies it again. Each picker remembers its last box.
QtObject {
    id: root
    // JSON array of {operation, tool} the engine implements (backend.moduleTools()).
    property string available: "[]"
    readonly property var supported: {
        const out = {}
        try { for (const t of JSON.parse(available || "[]")) out[t.operation + "/" + t.tool] = true } catch (e) {}
        return out
    }

    // gui: GUI-only values sent along (row fields, "@tab" for the shown channel). hint:
    // darktable's tooltip. icon: a glyph darktable draws instead of (or before) the label.
    readonly property var colorbalancePatches: ["@luma_lift", "@luma_gamma", "@luma_gain", "@luma_lift_flag",
        "@luma_gamma_flag", "@luma_gain_flag", "@color_lift_0", "@color_lift_1", "@color_lift_2",
        "@color_gamma_0", "@color_gamma_1", "@color_gamma_2", "@color_gain_0", "@color_gain_1",
        "@color_gain_2", "@color_lift_flag", "@color_gamma_flag", "@color_gain_flag"]
    readonly property var rowTools: ({
        temperature: {
            "preset:from_image_area": { tool: "from_image_area", kind: "area", hint: "set white balance to detected from area" }
        },
        exposure: {
            "@area_mode": { local: true },
            "@lightness": { local: true, conf: "darkroom/modules/exposure/lightness", tool: "spot", kind: "area", gui: ["@area_mode", "@lightness"],
                            label: "exposure", report: "exposure",
                            hint: "set the exposure adjustment using the selected area" }
        },
        rgblevels: {
            "@black": { tool: "black", kind: "point", gui: ["@tab"], hint: "pick black point from image" },
            "@gray": { tool: "gray", kind: "point", gui: ["@tab"], hint: "pick medium gray point from image" },
            "@white": { tool: "white", kind: "point", gui: ["@tab"], hint: "pick white point from image" },
            "@auto": { tool: "auto", kind: "button", gui: ["@tab"], hint: "apply auto levels" },
            "@auto_region": { tool: "auto_region", kind: "area", gui: ["@tab"],
                              hint: "apply auto levels based on a region defined by the user\nclick and drag to draw the area\nright-click to cancel" }
        },
        levels: {
            "@auto": { tool: "auto", kind: "button", hint: "apply auto levels" }
        },
        flip: {
            "orientation:rotate_90_degrees_ccw": { tool: "rotate_ccw", kind: "button", icon: "↺" },
            "orientation:rotate_90_degrees_cw": { tool: "rotate_cw", kind: "button", icon: "↻" },
            "orientation:flip_horizontally": { tool: "flip_horizontally", kind: "button", icon: "⇋" },
            "orientation:flip_vertically": { tool: "flip_vertically", kind: "button", icon: "⇵" }
        },
        filmicrgb: {
            "@auto_tune_levels": { tool: "auto_tune_levels", kind: "area",
                                   hint: "try to optimize the settings with some statistical assumptions.\nthis will fit the luminance range inside the histogram bounds.\nworks better for landscapes and evenly-lit images\nbut fails for high-keys, low-keys and high-ISO images.\nthis is not an artificial intelligence, but a simple guess.\nensure you understand its assumptions before using it." }
        },
        filmic: {
            "@auto_tune_levels": { tool: "auto_tune_levels", kind: "area", hint: "try to optimize the settings with some guessing.\nthis will fit the luminance range inside the histogram bounds.\nworks better for landscapes and evenly-lit images\nbut fails for high-keys and low-keys." }
        },
        agx: {
            "@auto_tune_levels": { tool: "auto_tune_levels", kind: "area",
                                   hint: "set black and white relative exposure using the selected area" },
            "@read_exposure": { tool: "read_exposure", kind: "button",
                                hint: "read exposure from metadata and exposure module" },
            "@reset_primaries": { tool: "reset_primaries", kind: "button", hint: "reset primaries to a predefined configuration",
                                  menu: [{ label: "blender-like", gui: { preset: 0 } }, { label: "smooth", gui: { preset: 1 } },
                                         { label: "unmodified", gui: { preset: 2 } }] },
            "@set_from_above": { tool: "set_from_above", kind: "button",
                                 hint: "set parameters to completely reverse primaries modifications,\nbut allow subsequent editing" }
        },
        profile_gamma: {
            "@auto_tune_levels": { tool: "auto_tune_levels", kind: "area", hint: "make an optimization with some guessing" }
        },
        negadoctor: {
            "@film_material": { tool: "film_material", kind: "area", hint: "pick color of film material from image" },
            "@shadows": { tool: "shadows", kind: "area", hint: "pick shadows color from image" },
            "@illuminant": { tool: "illuminant", kind: "area", hint: "pick illuminant color from image" }
        },
        basicadj: {
            "@auto": { tool: "auto", kind: "button", hint: "apply auto exposure based on the entire image" },
            "@select_region": { tool: "select_region", kind: "area",
                                hint: "apply auto exposure based on a region defined by the user\nclick and drag to draw the area\nright-click to cancel" }
        },
        colorbalance: {
            "@auto_luma": { tool: "auto_luma", kind: "area", oneShot: true, gui: colorbalancePatches,
                            hint: "fit the whole histogram and center the average luma" },
            "@auto_color": { tool: "auto_color", kind: "area", oneShot: true, gui: colorbalancePatches,
                             hint: "optimize the RGB curves to remove color casts" }
        },
        tonecurve: {
            "@pick_color": { tool: "pick_color", kind: "pointarea", marker: true,
                             hint: "pick GUI color from image\nctrl+click or right-click to select an area" }
        },
        rgbcurve: {
            "@show_color": { tool: "show_color", kind: "pointarea", marker: true, keepActive: true,
                             hint: "pick GUI color from image\nctrl+click or right-click to select an area" },
            "@create_curve": { tool: "create_curve", kind: "area", marker: true, gui: ["@tab"],
                               hint: "create a curve based on an area from the image\ndrag to create a flat curve\nctrl+drag to create a positive curve\nshift+drag to create a negative curve" }
        },
        colorzones: {
            "@colorpicker": { tool: "show_color", kind: "pointarea", marker: true, keepActive: true,
                              hint: "pick GUI color from image\nctrl+click or right-click to select an area" },
            "@colorpicker_set_values": { tool: "create_curve", kind: "area", marker: true, gui: ["@tab"],
                                         hint: "create a curve based on an area from the image\ndrag to create a flat curve\nctrl+drag to create a positive curve\nshift+drag to create a negative curve" }
        },
        channelmixerrgb: {
            "@picker": { tool: "picker", kind: "area", gui: ["@spot_mode", "@use_mixing", "@lightness_spot", "@hue_spot", "@chroma_spot"],
                         report: "lch", hint: "set white balance to detected from area" },
            "@spot_mode": { local: true },
            "@use_mixing": { local: true, conf: "darkroom/modules/channelmixerrgb/use_mixing" },
            "@lightness_spot": { local: true, conf: "darkroom/modules/channelmixerrgb/lightness" },
            "@hue_spot": { local: true, conf: "darkroom/modules/channelmixerrgb/hue" },
            "@chroma_spot": { local: true, conf: "darkroom/modules/channelmixerrgb/chroma" }
        },
        // lens.cc:4386/4398: the cameras and lenses lensfun finds for the EXIF names, as a menu
        // (engine list "find_camera"/"find_lens" of catalogModel.requestChoices).
        lens: {
            "@find_camera": { choices: "find_camera", kind: "button", icon: "▾" },
            "@find_lens": { choices: "find_lens", kind: "button", icon: "▾" }
        },
        colorharmonizer: {
            "@auto_detect": { tool: "auto_detect", kind: "button", icon: "camera",
                              hint: "analyze the image's hue distribution and automatically suggest the harmony rule\nand anchor hue that best match its existing color palette." },
            // area E: the RYB vectorscope (VectorscopeView, tools_vectorscope.c) above darktable's
            // sync row (colorharmonizer.c:1513-1540).
            "@sync_to_vectorscope": { local: true, before: "vectorscope", conf: "plugins/darkroom/colorharmonizer/sync_to_vectorscope" },
            "@set_from_vectorscope": { tool: "set_from_vectorscope", kind: "button", icon: "↻", disabledBy: "@sync_to_vectorscope",
                                       hint: "import the harmony rule and anchor hue currently displayed in the vectorscope." }
        },
        // ---- area E (tools_effects.c) ----
        // colorchecker.c:1559: the picker beside "patch" selects the nearest source patch.
        colorchecker: {
            "@patch": { local: true, tool: "patch", kind: "pointarea", label: "patch", hint: "pick the patch nearest to the picked color" }
        },
        // colormapping.c:1003-1013; the cluster swatches darktable draws above the buttons
        // (cluster_preview_draw) come first ("before").
        colormapping: {
            source: { tool: "acquire_source", kind: "button", before: "clusters", hint: "analyze this image as a source image" },
            target: { tool: "acquire_target", kind: "button", hint: "analyze this image as a target image" }
        }
    })
    readonly property var sliderTools: ({
        filmicrgb: {
            grey_point_source: { tool: "grey_point_source", kind: "area" },
            black_point_source: { tool: "black_point_source", kind: "area" },
            white_point_source: { tool: "white_point_source", kind: "area" }
        },
        filmic: {
            grey_point_source: { tool: "grey_point_source", kind: "area" },
            black_point_source: { tool: "black_point_source", kind: "area" },
            white_point_source: { tool: "white_point_source", kind: "area" }
        },
        agx: {
            range_black_relative_ev: { tool: "range_black_relative_ev", kind: "area" },
            range_white_relative_ev: { tool: "range_white_relative_ev", kind: "area" },
            curve_pivot_x: { tool: "curve_pivot_x", kind: "area" },
            curve_pivot_y_linear_output: { tool: "curve_pivot_y_linear_output", kind: "area" }
        },
        profile_gamma: {
            grey_point: { tool: "grey_point", kind: "area" },
            shadows_range: { tool: "shadows_range", kind: "area" },
            dynamic_range: { tool: "dynamic_range", kind: "area" }
        },
        negadoctor: {
            D_max: { tool: "D_max", kind: "area" },
            offset: { tool: "offset", kind: "area" },
            black: { tool: "black", kind: "area" },
            exposure: { tool: "exposure", kind: "area" }
        },
        basicadj: { middle_grey: { tool: "middle_grey", kind: "area" } },
        colorbalance: {
            grey: { tool: "grey", kind: "area" },
            lift_0: { tool: "lift_factor", kind: "area", gui: colorbalancePatches },
            gamma_0: { tool: "gamma_factor", kind: "area", gui: colorbalancePatches },
            gain_0: { tool: "gain_factor", kind: "area", gui: colorbalancePatches },
            "@lift_hue": { tool: "hue_lift", kind: "area", gui: colorbalancePatches },
            "@gamma_hue": { tool: "hue_gamma", kind: "area", gui: colorbalancePatches },
            "@gain_hue": { tool: "hue_gain", kind: "area", gui: colorbalancePatches }
        },
        colorharmonizer: {
            anchor_hue: { tool: "anchor_hue", kind: "pointarea" },
            custom_hue_0: { tool: "custom_hue_0", kind: "pointarea" },
            custom_hue_1: { tool: "custom_hue_1", kind: "pointarea" },
            custom_hue_2: { tool: "custom_hue_2", kind: "pointarea" },
            custom_hue_3: { tool: "custom_hue_3", kind: "pointarea" }
        },
        // ---- area E (tools_effects.c): pickers on sliders and beside colour swatches ----
        colorize: { hue: { tool: "hue", kind: "point" } },
        splittoning: { shadow_hue: { tool: "shadow_hue", kind: "point" }, highlight_hue: { tool: "highlight_hue", kind: "point" } },
        graduatednd: { hue: { tool: "hue", kind: "point" } },
        monochrome: { highlights: { tool: "highlights", kind: "area" } },
        borders: { color: { tool: "color", kind: "point", hint: "pick border color from image" },
                   frame_color: { tool: "frame_color", kind: "point", hint: "pick frame line color from image" } },
        watermark: { color: { tool: "color", kind: "point", hint: "pick color from image" } },
        invert: { color: { tool: "color", kind: "area", hint: "pick color of film material from image" } },
        relight: { center: { tool: "center", kind: "pointarea", band: true, hint: "toggle tool for picking median lightness in image" } },
        colorequal: { hue_shift: { tool: "hue_shift", kind: "pointarea", band: true },
                      white_level: { tool: "white_level", kind: "area" } },
        retouch: { fill_color: { tool: "fill_color", kind: "point", hint: "pick fill color from image" } },
        // toneequal.c:3326, 3338: magic-wand buttons on the two mask compensation sliders.
        toneequal: { exposure_boost: { tool: "exposure_boost", kind: "button", icon: "wand", hint: "auto-adjust the average exposure" },
                     contrast_boost: { tool: "contrast_boost", kind: "button", icon: "wand", hint: "auto-adjust the contrast" } }
    })
    readonly property var histograms: ({ rgblevels: "levels", levels: "levels" })

    property var active: null          // { operation, instance, tool, kind, gui, oneShot, keepActive }
    property var boxes: ({})           // "operation/instance/tool" → last box
    property var results: ({})         // "operation/instance/tool" → last result
    property var histogramData: ({})   // "operation/instance" → { channels, max }
    property string message: ""
    // ---- area E ----
    // GUI-only values darktable keeps in its configuration (dt_conf keys such as
    // darkroom/modules/exposure/lightness) persist here under the same key names.
    property Settings confStore: Settings { category: "darktable-conf" }
    function confValue(key, fallback) {
        if (!key) return fallback
        const v = confStore.value(key, undefined)
        return v === undefined || v === null || v === "" ? fallback : Number(v)
    }
    function storeConf(key, value) { if (key) confStore.setValue(key, value) }
    function confOf(operation, field) { const s = (rowTools[operation] || {})[field]; return s && s.conf ? s.conf : "" }
    // The vectorscope's harmony guide and plot (tools_vectorscope.c), from the latest result.
    property var harmonyGuide: ({ type: 0, rotation: 0, width: 0 })
    property var vectorscope: null
    function setHarmonyGuide(type, rotation, width) {
        harmonyGuide = { type: type, rotation: rotation, width: width }
        send("gamma", 0, "harmony_guide", null, { type: type, rotation: rotation, width: width })
    }
    function requestVectorscope() { send("gamma", 0, "vectorscope", null, null) }
    signal runRequested(string operation, int instance, var request)

    function key(operation, instance, tool) { return operation + "/" + instance + "/" + tool }
    function implemented(operation, spec) { return !!spec && !!spec.tool && !!supported[operation + "/" + spec.tool] }
    function rowTool(operation, field) {
        const s = (rowTools[operation] || {})[field]
        return s && (s.local && !s.tool || s.choices || implemented(operation, s)) ? s : null
    }
    function sliderTool(operation, field) {
        const s = (sliderTools[operation] || {})[field]
        return implemented(operation, s) ? s : null
    }
    function histogramField(operation) {
        return implemented(operation, { tool: "histogram" }) ? (histograms[operation] || "") : ""
    }
    function isActive(operation, instance, tool) {
        return !!active && active.operation === operation && active.instance === instance && active.tool === tool
    }
    function defaultBox(kind) { return kind === "area" ? [0.02, 0.02, 0.98, 0.98] : [0.5, 0.5, 0.5, 0.5] }
    function activeBox() {
        if (!active) return null
        return boxes[key(active.operation, active.instance, active.tool)] || defaultBox(active.kind)
    }
    // The last result of a tool with a marker for a curve of this module (show color, create curve).
    function marker(operation, instance) {
        if (!active || active.operation !== operation || active.instance !== instance) return null
        const r = results[key(operation, instance, active.tool)]
        return r && r.marker ? r.marker : null
    }
    // The picked band (min, mean, max on darktable's axis) of a marking picker while it is
    // active: relight's center slider, color equalizer's hue (tools_effects.c report_band).
    function band(operation, instance, tool) {
        if (!isActive(operation, instance, tool)) return null
        const r = results[key(operation, instance, tool)]
        return r && r.band ? r.band : null
    }
    function send(operation, instance, tool, box, gui) {
        const request = { tool: tool }
        if (box) request.box = box
        if (gui && Object.keys(gui).length) request.gui = gui
        runRequested(operation, instance, request)
    }
    // A picker button: switch it on (and apply it with its last box) or off. A button runs once;
    // `extraGui` carries a menu choice.
    function toggle(operation, instance, spec, gui, extraGui) {
        const g = Object.assign({}, gui || {}, extraGui || {})
        if (spec.kind === "button") { send(operation, instance, spec.tool, null, g); return }
        if (isActive(operation, instance, spec.tool)) { cancel(); return }
        active = { operation: operation, instance: instance, tool: spec.tool, kind: spec.kind, gui: g,
                   oneShot: !!spec.oneShot, keepActive: !!spec.keepActive }
        send(operation, instance, spec.tool, activeBox(), g)
    }
    function cancel() { active = null }
    // The viewport drew a new box (normalised to the displayed image); modifiers are the
    // darktable variants of create curve (ctrl positive, shift negative).
    function setBox(box, modifiers) {
        if (!active) return
        const next = Object.assign({}, boxes)
        next[key(active.operation, active.instance, active.tool)] = box
        boxes = next
        const gui = Object.assign({}, active.gui)
        gui["@picker_modifier"] = (modifiers & Qt.ControlModifier) ? 1 : (modifiers & Qt.ShiftModifier) ? -1 : 0
        send(active.operation, active.instance, active.tool, box, gui)
    }
    // GUI-only values of the active picker's module changed (e.g. the target lightness):
    // darktable applies the picker again.
    function updateGui(operation, instance, gui) {
        if (!active || active.operation !== operation || active.instance !== instance) return
        active = Object.assign({}, active, { gui: gui })
        send(operation, instance, active.tool, activeBox(), gui)
    }
    // A parameter of a module was edited from its controls: darktable switches the module's
    // picker off unless it is a keep-active one (develop/imageop.c dt_iop_gui_changed).
    function parameterEdited(operation, instance) {
        if (active && active.operation === operation && active.instance === instance && !active.keepActive) cancel()
    }
    function requestHistogram(operation, instance) { send(operation, instance, "histogram", null, null) }
    // backend.moduleToolResult
    function accept(operation, instance, tool, resultJson, error) {
        let result = {}
        try { result = JSON.parse(resultJson || "{}") } catch (e) {}
        result.error = error
        if (tool === "histogram") {
            if (result.histogram) {
                const next = Object.assign({}, histogramData)
                next[operation + "/" + instance] = result.histogram
                histogramData = next
            }
            return
        }
        if (result.guide) harmonyGuide = result.guide
        if (tool === "vectorscope") {
            if (result.vectorscope) vectorscope = result.vectorscope
            return
        }
        if (tool === "harmony_guide") return
        const next = Object.assign({}, results)
        next[key(operation, instance, tool)] = result
        results = next
        message = error === 6 ? "the picked area holds no pixels" : ""
        // Optimisers that darktable switches off once they have run.
        if (isActive(operation, instance, tool) && active.oneShot) cancel()
    }
}
