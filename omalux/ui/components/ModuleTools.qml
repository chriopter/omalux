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
    // What the engine draws on the photo for the shown module (backend.canvasOverlay): it changes
    // when a shape is selected or edited there, which the mask manager's properties follow.
    property string shapesRevision: ""
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
    readonly property var checkerGui: ["@checker_size", "@checker_color_1", "@checker_color_2"]
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
        // Mask previews darktable draws from inside the module (module_display.c): a toggle
        // with its "showmask" button; one preview at a time.
        toneequal: {
            "@display_exposure_mask": { tool: "display_mask", kind: "display", icon: "showmask" }
        },
        filmicrgb: {
            "@display_highlight_reconstruction_mask": { tool: "display_mask", kind: "display", icon: "showmask" },
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
                             hint: "pick GUI color from image\nctrl+click or right-click to select an area" },
            "@scale_for_graph": { local: true }   // area E: CurveEditor xLog/yLog
        },
        rgbcurve: {
            "@show_color": { tool: "show_color", kind: "pointarea", marker: true, keepActive: true,
                             hint: "pick GUI color from image\nctrl+click or right-click to select an area" },
            "@create_curve": { tool: "create_curve", kind: "area", marker: true, gui: ["@tab"],
                               hint: "create a curve based on an area from the image\ndrag to create a flat curve\nctrl+drag to create a positive curve\nshift+drag to create a negative curve" }
        },
        colorzones: {
            // the selection of the curve shown (g->channel, colorzones.c:439)
            "@display_mask": { tool: "display_mask", kind: "display", icon: "showmask", gui: ["@tab"] },
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
            "@chroma_spot": { local: true, conf: "darkroom/modules/channelmixerrgb/chroma" },
            // area E: "calibrate with a color checker" (channelmixerrgb.c:4669-4740, checker.c);
            // "recompute" puts the chart on the photo and measures it, validate and accept use it.
            "@checker": { local: true, conf: "darkroom/modules/channelmixerrgb/colorchecker" },
            "@optimize": { local: true, conf: "darkroom/modules/channelmixerrgb/optimization" },
            "@safety": { local: true, conf: "darkroom/modules/channelmixerrgb/safety" },
            "@recompute": { tool: "checker", kind: "chart", gui: ["@checker", "@optimize", "@safety"], extra: { action: "profile" },
                            keepActive: true,
                            hint: "recompute the profile: move the chart's corners on the photo onto the color checker,\nthe module measures its patches" },
            "@validate": { tool: "checker", kind: "button", chartBox: true, gui: ["@checker", "@optimize", "@safety"], extra: { action: "validate" },
                           hint: "check the output delta E" },
            "@accept": { tool: "checker", kind: "button", chartBox: true, gui: ["@checker", "@optimize", "@safety"], extra: { action: "accept" },
                         hint: "accept the computed profile and set it in the module" }
        },
        // lens.cc:4386/4398: the cameras and lenses lensfun finds for the EXIF names, as a menu
        // (engine list "find_camera"/"find_lens" of catalogModel.requestChoices).
        lens: {
            "@find_camera": { choices: "find_camera", kind: "button", icon: "▾" },
            "@find_lens": { choices: "find_lens", kind: "button", icon: "▾" },
            // area E: lens.cc:4485 / _use_latest_md_algo_callback 2463, offered while an edit
            // uses the first embedded-metadata algorithm (gui_changed 4300).
            "@use_latest_algorithm": { set: { md_version: 1, scale_md_v1: 0 }, kind: "button",
                                       when: { field: "md_version", in: [0] } }
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
        // ashift.c:4706-4740 the fit buttons with the variants darktable reaches by ctrl/shift
        // (_event_fit_*_button_clicked 5323-5466); "auto" chooses detected structure
        // (_event_structure_auto_clicked), otherwise the lines or rectangle drawn on the photo.
        ashift: {
            "@auto": { local: true },
            "@vertical": { tool: "fit", kind: "button", gui: ["@auto"], report: "message",
                           hint: "automatically correct for vertical perspective distortion\nctrl+click to only fit rotation\nshift+click to only fit lens shift",
                           menu: [{ label: "rotation and lens shift", gui: { dir: 19 } }, { label: "only rotation (ctrl)", gui: { dir: 17 } },
                                  { label: "only lens shift (shift)", gui: { dir: 18 } }] },
            "@horizontal": { tool: "fit", kind: "button", gui: ["@auto"],
                             hint: "automatically correct for horizontal perspective distortion\nctrl+click to only fit rotation\nshift+click to only fit lens shift",
                             menu: [{ label: "rotation and lens shift", gui: { dir: 37 } }, { label: "only rotation (ctrl)", gui: { dir: 33 } },
                                    { label: "only lens shift (shift)", gui: { dir: 36 } }] },
            "@both": { tool: "fit", kind: "button", gui: ["@auto"],
                       hint: "automatically correct for vertical and horizontal perspective distortions, fitting rotation,\nlens shift in both directions, and shear\nctrl+click to only fit rotation\nshift+click to only fit lens shift\nctrl+shift+click to only fit rotation and lens shift",
                       menu: [{ label: "rotation, lens shift and shear", gui: { dir: 63 } }, { label: "only rotation (ctrl)", gui: { dir: 49 } },
                              { label: "only lens shift (shift)", gui: { dir: 54 } }, { label: "rotation and lens shift (ctrl+shift)", gui: { dir: 55 } }] }
        },
        // rasterfile.c:764: the raster mask traced into path shapes (mask_manager.c), which the
        // blend section's "add existing shape" then offers.
        rasterfile: { "@vectorize": { tool: "vectorize", kind: "button", report: "message",
                                      hint: "vectorize the current bitmap and create corresponding\nshapes in the mask manager" } },
        // colorbalancergb.c:2030-2068 "mask preview settings", dt_conf keys with darktable's defaults.
        colorbalancergb: {
            "@checker_color_1": { local: true, conf: ["plugins/darkroom/colorbalancergb/checker1/red",
                                                       "plugins/darkroom/colorbalancergb/checker1/green",
                                                       "plugins/darkroom/colorbalancergb/checker1/blue"], fallback: [1, 1, 1] },
            "@checker_color_2": { local: true, conf: ["plugins/darkroom/colorbalancergb/checker2/red",
                                                       "plugins/darkroom/colorbalancergb/checker2/green",
                                                       "plugins/darkroom/colorbalancergb/checker2/blue"], fallback: [0.18, 0.18, 0.18] },
            "@checker_size": { local: true, conf: "plugins/darkroom/colorbalancergb/checker/size" }
        },
        // GUI-only "scale for graph" of the curve (CurveEditor xLog/yLog), 0 = linear.
        basecurve: { "@scale_for_graph": { local: true } },
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
                      white_level: { tool: "white_level", kind: "area" },
                      // colorequal.c:3120, 3142 the showmask quads; mode by the last colour page (@channel)
                      threshold: { tool: "display_mask", kind: "display", icon: "showmask", extra: { typeBase: 4 }, gui: ["@channel"],
                                   hint: "visualize weighting function on changed output and view weighting curve.\nred shows possibly changed data, blueish parts will not be changed." },
                      param_size: { tool: "display_mask", kind: "display", icon: "showmask", extra: { typeBase: 0 }, gui: ["@channel"],
                                    hint: "visualize changed output for the selected tab.\nred shows increased values, blue decreased." } },
        // colorbalancergb.c:1982-2010 the masks over a checkerboard (mask_callback 1418)
        colorbalancergb: {
            shadows_weight: { tool: "display_mask", kind: "display", icon: "showmask", extra: { type: 0 }, gui: checkerGui,
                              hint: "displays a shadows mask, overlaid as a checkerboard\nthe still-visible area of the image (not hidden by the mask) is the area\nthat will be affected by the shadows sliders in the other tabs" },
            mask_grey_fulcrum: { tool: "display_mask", kind: "display", icon: "showmask", extra: { type: 1 }, gui: checkerGui,
                                 hint: "displays a mid-tones mask, overlaid as a checkerboard\nthe still-visible area of the image (not hidden by the mask) is the area\nthat will be affected by the mid-tones sliders in the other tabs" },
            highlights_weight: { tool: "display_mask", kind: "display", icon: "showmask", extra: { type: 2 }, gui: checkerGui,
                                 hint: "displays a highlights mask, overlaid as a checkerboard\nthe still-visible area of the image (not hidden by the mask) is the area\nthat will be affected by the highlights sliders in the other tabs" }
        },
        retouch: { fill_color: { tool: "fill_color", kind: "point", hint: "pick fill color from image" } },
        // highlights.c:1289-1320 the showmask quads (type: dt_highlights_mask_t)
        highlights: {
            clip: { tool: "display_mask", kind: "display", icon: "showmask", extra: { type: 4 },
                    hint: "visualize clipped highlights in a false color representation.\nthe effective clipping level also depends on the reconstruction method." },
            combine: { tool: "display_mask", kind: "display", icon: "showmask", extra: { type: 1 },
                       hint: "visualize the combined segments in a false color representation." },
            candidating: { tool: "display_mask", kind: "display", icon: "showmask", extra: { type: 2 },
                           hint: "visualize segments that are considered to have a good candidate in a false color representation." },
            strength: { tool: "display_mask", kind: "display", icon: "showmask", extra: { type: 3 },
                        hint: "show the effect that is added to already reconstructed data." }
        },
        // demosaic.c:1766, 1811, 1818
        demosaic: {
            dual_thrs: { tool: "display_mask", kind: "display", icon: "showmask", extra: { type: 1 }, hint: "toggle mask visualization" },
            cs_thrs: { tool: "display_mask", kind: "display", icon: "showmask", extra: { type: 2 }, hint: "visualize sharpened areas" },
            cs_boost: { tool: "display_mask", kind: "display", icon: "showmask", extra: { type: 3 }, hint: "visualize the overall radius" }
        },
        // lens.cc:4589
        lens: { v_strength: { tool: "display_mask", kind: "display", icon: "showmask", extra: { type: 1 },
                              hint: "show applied optical vignette correction mask" } },
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
    function storeConf(key, value) {
        if (Array.isArray(key)) { for (let i = 0; i < key.length; ++i) storeConf(key[i], Array.isArray(value) ? value[i] : value); return }
        if (key) confStore.setValue(key, value)
    }
    // A GUI-only row's stored value: one key, or one key per channel of a colour.
    function confValues(spec, fallback) {
        if (!spec || !spec.conf) return fallback
        if (Array.isArray(spec.conf)) return spec.conf.map((k, i) => confValue(k, (spec.fallback || [])[i] || 0))
        return confValue(spec.conf, fallback)
    }
    function confOf(operation, field) { const s = (rowTools[operation] || {})[field]; return s && s.conf ? s.conf : "" }
    // The vectorscope's harmony guide and plot (tools_vectorscope.c), from the latest result.
    property var harmonyGuide: ({ type: 0, rotation: 0, width: 0 })
    property var vectorscope: null
    function setHarmonyGuide(type, rotation, width) {
        harmonyGuide = { type: type, rotation: rotation, width: width }
        send("gamma", 0, "harmony_guide", null, { type: type, rotation: rotation, width: width })
    }
    function requestVectorscope() { send("gamma", 0, "vectorscope", null, null) }
    // color calibration's colour checker layout (checker.c checker_layout) for ChartOverlay
    property var chartLayout: null
    function chartSettings(gui) {
        if (!active || active.tool !== "checker") return
        const chartChanged = (active.gui || {})["@checker"] !== gui["@checker"]
        active = Object.assign({}, active, { gui: Object.assign({}, active.gui, gui) })
        if (chartChanged) send(active.operation, active.instance, "checker_layout", null, gui)
    }
    // The blend section's display mask / switch off mask of one module at a time (blend_display.c).
    property var blendDisplay: null    // { operation, instance, mask, suppress }
    function blendDisplayOf(operation, instance) {
        return blendDisplay && blendDisplay.operation === operation && blendDisplay.instance === instance ? blendDisplay : null
    }
    function setBlendDisplay(operation, instance, mask, suppress) {
        blendDisplay = mask || suppress ? { operation: operation, instance: instance, mask: mask, suppress: suppress } : null
        // One preview at a time (the engine switches the module's own preview off as well).
        if (mask || suppress) moduleDisplay = null
        send(operation, instance, "blend_display", null, { mask: mask ? 1 : 0, suppress: suppress ? 1 : 0 })
    }
    // A module's own mask preview (module_display.c): toneequal "display exposure mask", filmic
    // rgb "display highlight reconstruction mask", color zones "display selection", color
    // balance rgb's three mask quads, color equalizer's two quads. darktable shows it for the
    // focused module only and drops it when the module loses focus (collapsed here).
    property var moduleDisplay: null   // { operation, instance, key, spec }
    function displayKey(spec) { return spec.tool + JSON.stringify(spec.extra || {}) }
    function displayActive(operation, instance, spec) {
        return !!moduleDisplay && moduleDisplay.operation === operation && moduleDisplay.instance === instance
               && moduleDisplay.key === displayKey(spec)
    }
    function displayGui(spec, gui) {
        const g = { display: 1 }
        const extra = spec.extra || {}
        if (extra.type !== undefined) g.type = extra.type
        if (extra.typeBase !== undefined) g.type = extra.typeBase + (Number(gui["@channel"]) || 0) + 1
        if (gui["@tab"] !== undefined) g.channel = Number(gui["@tab"]) || 0
        // colorbalancergb's checkerboard, under darktable's dt_conf names
        const keys = rowTools.colorbalancergb
        if (gui["@checker_size"] !== undefined) g[keys["@checker_size"].conf] = Number(gui["@checker_size"])
        for (const name of ["@checker_color_1", "@checker_color_2"])
            if (Array.isArray(gui[name])) for (let i = 0; i < 3; ++i) g[keys[name].conf[i]] = Number(gui[name][i])
        return g
    }
    function toggleDisplay(operation, instance, spec, gui) {
        if (displayActive(operation, instance, spec)) { hideDisplay(); return }
        // toneequal, colorzones and colorbalancergb refuse while the blend section shows its mask
        // (toneequal.c:1940, colorzones.c:2379, colorbalancergb.c:1423).
        if (blendDisplayOf(operation, instance) && blendDisplayOf(operation, instance).mask
                && ["toneequal", "colorzones", "colorbalancergb", "retouch"].indexOf(operation) >= 0) {
            message = "cannot display masks when the blending mask is displayed"
            return
        }
        blendDisplay = null
        moduleDisplay = { operation: operation, instance: instance, key: displayKey(spec), spec: spec }
        send(operation, instance, spec.tool, null, displayGui(spec, gui))
    }
    function hideDisplay() {
        if (!moduleDisplay) return
        const d = moduleDisplay
        moduleDisplay = null
        send(d.operation, d.instance, d.spec.tool, null, { display: 0 })
    }
    // The shown page or the checkerboard changed: darktable redraws the preview.
    function updateDisplay(operation, instance, gui) {
        if (!moduleDisplay || moduleDisplay.operation !== operation || moduleDisplay.instance !== instance) return
        send(operation, instance, moduleDisplay.spec.tool, null, displayGui(moduleDisplay.spec, gui))
    }
    function moduleCollapsed(operation, instance) {
        if (moduleDisplay && moduleDisplay.operation === operation && moduleDisplay.instance === instance) hideDisplay()
    }
    signal runRequested(string operation, int instance, var request)

    function key(operation, instance, tool) { return operation + "/" + instance + "/" + tool }
    function implemented(operation, spec) { return !!spec && !!spec.tool && !!supported[operation + "/" + spec.tool] }
    function rowTool(operation, field) {
        const s = (rowTools[operation] || {})[field]
        return s && (s.local && !s.tool || s.choices || s.set || implemented(operation, s)) ? s : null
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
    // area E: a colour checker's four corners start 10 px inside the view (_init_bounding_box)
    function defaultBox(kind) {
        if (kind === "chart") return [0.01, 0.01, 0.99, 0.01, 0.99, 0.99, 0.01, 0.99]
        return kind === "area" ? [0.02, 0.02, 0.98, 0.98] : [0.5, 0.5, 0.5, 0.5]
    }
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
        // area E: a chart's eight corner coordinates travel as "corners" (checker.c)
        if (box && box.length === 8) gui = Object.assign({}, gui || {}, { corners: box })
        else if (box) request.box = box
        if (gui && Object.keys(gui).length) request.gui = gui
        runRequested(operation, instance, request)
    }
    // A picker button: switch it on (and apply it with its last box) or off. A button runs once;
    // `extraGui` carries a menu choice.
    function toggle(operation, instance, spec, gui, extraGui) {
        if (spec.kind === "display") { toggleDisplay(operation, instance, spec, gui || {}); return }
        const g = Object.assign({}, gui || {}, spec.extra || {}, extraGui || {})
        if (spec.kind === "button") {
            // area E: validate / accept use the corners of the chart shown on the photo
            send(operation, instance, spec.tool, spec.chartBox ? (boxes[key(operation, instance, spec.tool)] || defaultBox("chart")) : null, g)
            return
        }
        if (isActive(operation, instance, spec.tool)) { cancel(); return }
        active = { operation: operation, instance: instance, tool: spec.tool, kind: spec.kind, gui: g,
                   oneShot: !!spec.oneShot, keepActive: !!spec.keepActive }
        if (spec.kind === "chart") send(operation, instance, "checker_layout", null, g)
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
        if (tool === "checker_layout") { if (result.chart) chartLayout = result.chart; return }
        const next = Object.assign({}, results)
        next[key(operation, instance, tool)] = result
        results = next
        message = error === 6 ? "the picked area holds no pixels" : ""
        // Optimisers that darktable switches off once they have run.
        if (isActive(operation, instance, tool) && active.oneShot) cancel()
    }
}
