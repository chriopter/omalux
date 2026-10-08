import QtQuick
import QtTest
import "../../ui/components"

// The blend section, the parametric-mask range and the multi-instance button against a
// hand-written catalog (the engine side is covered by omalux/tests/blending.json):
//   QT_QPA_PLATFORM=offscreen /usr/lib/qt6/bin/qmltestrunner -input omalux/tests/components/tst_blending.qml

Item {
    id: top
    width: 400; height: 1400
    EditorTheme { id: th }

    // ---- BlendifRange ----------------------------------------------------------------------
    property var rangeValues: [0.1, 0.3, 0.8, 1]
    property var rangeEdits: []
    property int polarityToggles: 0
    property int rangeResets: 0
    BlendifRange {
        id: range
        width: 320
        theme: th
        label: "input"
        values: top.rangeValues
        stops: [[0, "#000000"], [1, "#808080"]]
        onValuesEdited: v => { top.rangeEdits.push(v); top.rangeValues = v }
        onPolarityToggled: neg => { top.polarityToggles++; negative = neg }
        onResetRequested: top.rangeResets++
    }

    // ---- BlendSection ----------------------------------------------------------------------
    function slider(field, label, unit, factor, digits, min, max, smin, smax, def) {
        return { field: field, path: field, label: label, section: null, widget: "slider", unit: unit, factor: factor,
                 offset: 0, digits: digits, min: min, max: max, soft_min: smin, soft_max: smax, default: def }
    }
    readonly property var blendRows: [
        { field: "mask_mode", label: "blend mask mode", widget: "combobox", default: 0,
          values: [{ value: 0, label: "off" }, { value: 1, label: "uniformly" }, { value: 3, label: "drawn mask" },
                   { value: 5, label: "parametric mask" }, { value: 7, label: "drawn & parametric mask" }, { value: 9, label: "raster mask" }] },
        { field: "blend_mode", label: "mode", widget: "combobox", default: 24 },
        { field: "blend_reverse", label: "toggle blend order", widget: "toggle", default: 0 },
        slider("blend_parameter", "fulcrum", " EV", 1, 3, -18, 18, -3, 3, 0),
        slider("opacity", "opacity", "%", 1, 0, 0, 100, 0, 100, 100),
        { field: "mask_id", label: "drawn mask", widget: "text" },
        { field: "drawn_polarity", label: "toggle polarity of drawn mask", widget: "toggle", default: 0 },
        { field: "@notice", label: "drawn mask shapes are drawn on the image — not available yet", widget: "notice" },
        { field: "raster_mask", label: "raster mask", widget: "text" },
        { field: "raster_mask_invert", label: "toggle polarity of raster mask", widget: "toggle", default: 0 },
        { field: "blendif_output", label: "output", widget: "graph" },
        { field: "blendif_input", label: "input", widget: "graph" },
        slider("boost_factor", "boost factor", " EV", 1, 3, 0, 18, 0, 3, 0),
        { field: "mask_combine", label: "combine masks", widget: "combobox", default: 0,
          values: [{ value: 0, label: "exclusive" }, { value: 2, label: "inclusive" }, { value: 1, label: "exclusive & inverted" },
                   { value: 3, label: "inclusive & inverted" }] },
        slider("details", "details threshold", "%", 100, 0, -1, 1, -1, 1, 0),
        { field: "feathering_guide", label: "feathering guide", widget: "combobox", default: 5,
          values: [{ value: 2, label: "output before blur" }, { value: 1, label: "input before blur" },
                   { value: 6, label: "output after blur" }, { value: 5, label: "input after blur" }] },
        slider("feathering_radius", "feathering radius", " px", 1, 1, 0, 250, 0, 250, 0),
        slider("blur_radius", "blurring radius", " px", 1, 1, 0, 100, 0, 100, 0),
        slider("brightness", "mask opacity", "%", 100, 0, -1, 1, -1, 1, 0),
        slider("contrast", "mask contrast", "%", 100, 0, -1, 1, -1, 1, 0)
    ]
    function blendState(mode) {
        const params = []
        for (let ch = 0; ch < 16; ++ch) params.push(0, 0, 1, 1)
        return { masks: true, parametric: true, csp: 4, default_cst: 4, raw: false, mask_mode: mode, blend_cst: 4,
                 blend_mode: 24, reverse: false, blend_parameter: 0, fulcrum: false, opacity: 100, mask_combine: 0,
                 drawn_polarity: false, blendif: 0, blendif_parameters: params,
                 boost_factors: [0, 0, 0, 0, 0, 0, 0, 0, -6.64385619, -6.64385619, 0, 0, -6.64385619, -6.64385619, 0, 0],
                 outputs_used: false, details: 0, feathering_guide: 5, feathering_radius: 0, blur_radius: 0, brightness: 0,
                 contrast: 0, mask_id: 0, drawn_shapes: 0, drawn_available: false, raster_mask_source: "",
                 raster_mask_instance: 0, raster_mask_id: -1, raster_mask_invert: false, raster_linked: false, raster_masks: [],
                 blend_modes: [{ value: 24, label: "normal" }, { value: 4, label: "multiply" }],
                 channels: [{ label: "g", name: "gray", in: 0, out: 4, boost: true, boost_offset: 0, scale: "default", increment: 1 / 255 },
                            { label: "Jz", name: "luminance", in: 8, out: 12, boost: true, boost_offset: -6.64385619, scale: "default", increment: .01 },
                            { label: "hz", name: "hue", in: 10, out: 14, boost: false, boost_offset: 0, scale: "hue", increment: 1 / 360 }] }
    }
    function catalogJson(mode) {
        const early = blendState(1)
        early.raster_masks = [{ id: 0, label: "early" }]
        return JSON.stringify([
            { operation: "early", instance: 0, position: 0, label: "early", enabled: true, hidden: false, parameters: [], blend: early,
              multi_name: "", instance_label: "", can_new: true, can_delete: false, can_move_up: true, can_move_down: false },
            { operation: "demo", instance: 0, position: 1, label: "demo", enabled: true, hidden: false, parameters: [], blend: blendState(mode),
              multi_name: "", instance_label: "", can_new: true, can_delete: false, can_move_up: false, can_move_down: true }])
    }
    ModuleCatalog {
        id: cat
        blendLayoutText: JSON.stringify({ rows: top.blendRows })
        catalog: top.catalogJson(0)
    }
    property var blendChanges: []
    BlendSection {
        id: section
        y: 120; width: 340
        theme: th
        moduleState: cat.states["demo/0"]
        catalogModel: cat
        navGroup: "demo/0"
        overridePrefix: "demo/0/"
        onChangesRequested: c => top.blendChanges.push(c)
    }

    // ---- InstanceButton --------------------------------------------------------------------
    property var instanceActions: []
    InstanceButton {
        id: instances
        x: 360; y: 0
        theme: th
        moduleState: cat.states["demo/0"]
        navGroup: "demo/0"
        onInstanceRequested: (action, name) => top.instanceActions.push([action, name])
    }

    function find(item, name) {
        if (!item) return null
        if (item.objectName === name) return item
        for (const child of item.children) { const found = find(child, name); if (found) return found }
        return null
    }

    TestCase {
        name: "blending"; when: windowShown

        function test_range() {
            compare(range.format(0.3), "30.0")
            range.scale = "hue"; compare(range.format(0.5), "180"); range.scale = "ab"
            compare(range.format(0.5), "0.0"); range.scale = "default"
            const bar = top.find(range, "blendif-bar-input")
            verify(bar)
            const x = v => bar.inset + v * (bar.width - 2 * bar.inset)
            const by = range.mapFromItem(bar, 0, 0).y
            // Drag the filled marker at 0.3 (upper half) to 0.5.
            mousePress(range, x(0.3), by + 8)
            mouseMove(range, x(0.5), by + 8)
            mouseRelease(range, x(0.5), by + 8)
            verify(top.rangeEdits.length > 0)
            fuzzyCompare(top.rangeValues[1], 0.5, 0.02)
            // A marker never passes its neighbour.
            mousePress(range, x(0.5), by + 8)
            mouseMove(range, x(0.95), by + 8)
            mouseRelease(range, x(0.95), by + 8)
            verify(top.rangeValues[1] <= top.rangeValues[2])
            // Keys: → moves the active marker by the channel increment.
            range.forceActiveFocus()
            range.activeMarker = 0
            const before = top.rangeValues[0]
            keyClick(Qt.Key_Right)
            fuzzyCompare(top.rangeValues[0], before + range.increment, 1e-6)
            keyClick(Qt.Key_Down)
            compare(range.activeMarker, 1)
            // Polarity and double-click reset.
            mouseClick(top.find(range, "blendif-polarity-input"))
            compare(top.polarityToggles, 1)
            verify(range.negative)
            mouseDoubleClickSequence(range, x(0.5), by + 15)
            compare(top.rangeResets, 1)
        }

        function test_section() {
            const visibleItem = name => { const i = top.find(section, name); return !!i && i.visible }
            verify(section.visible)
            verify(visibleItem("blend-mask-mode-demo/0"), "the mask mode is always offered")
            verify(!visibleItem("blend-opacity-demo/0"), "off: no opacity")
            // The catalog reports a parametric mask: blend rows and the input range appear.
            cat.catalog = top.catalogJson(5)
            verify(section.parametric)
            verify(visibleItem("blend-opacity-demo/0"))
            verify(visibleItem("blendif-input-demo/0"))
            compare(section.channels.length, 3)
            // Jz shows its boost relative to darktable's offset (−6.64 stored reads 0).
            section.tab = 1
            fuzzyCompare(section.boostOf(8) - section.channel.boost_offset, 0, 1e-6)
            section.tab = 0
            // A range edit sends the channel's four markers in one change.
            top.blendChanges = []
            section.rangeEdited(0, [0, 0.2, 1, 1])
            compare(Object.keys(top.blendChanges[0]).length, 4)
            compare(top.blendChanges[0]["blend.blendif_parameters[1]"], 0.2)
            // Overrides from the parameter queue win until the catalog catches up.
            section.overrides = { "demo/0/blend.opacity": 40 }
            compare(section.num("opacity", 100), 40)
            section.overrides = ({})
            // The polarity of a channel is its blendif bit (ch + 16).
            compare(section.negative(0), false)
            section.overrides = { "demo/0/blend.polarity[0]": 1 }
            compare(section.negative(0), true)
            section.overrides = ({})
            // Raster masks come from earlier modules only.
            compare(section.rasterSources.length, 1)
            compare(section.rasterSources[0].operation, "early")
            // Fulcrum: RGB (scene) with multiply.
            verify(!section.fulcrum)
            section.overrides = { "demo/0/blend.blend_mode": 4 }
            verify(section.fulcrum)
            section.overrides = ({})
            // Drawn mask without the drawing tools: the notice stands in for the shape buttons.
            cat.catalog = top.catalogJson(3)
            verify(section.drawn)
            verify(!section.parametric)
            cat.catalog = top.catalogJson(0)
            verify(!section.maskEnabled)
        }

        function test_instances() {
            mouseClick(instances, 5, 5, Qt.RightButton)
            compare(JSON.stringify(top.instanceActions), JSON.stringify([["new", ""]]), "right-click: new instance")
            // Rename: type a name, Enter keeps it.
            instances.rename()
            keyClick(Qt.Key_S); keyClick(Qt.Key_K); keyClick(Qt.Key_Y)
            keyClick(Qt.Key_Return)
            tryCompare(top.instanceActions, "length", 2)
            compare(JSON.stringify(top.instanceActions[1]), JSON.stringify(["rename", "sky"]))
            verify(instances.moduleState.canMoveDown)
            verify(!instances.moduleState.canDelete)
        }
    }
}
