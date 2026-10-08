import QtQuick
import QtTest
import "../../ui/components"

// Area E without an engine: the remaining pickers (slider, swatch and patch pickers, the band
// a marking picker shows), the tone equalizer wands, color mapping's clusters, color
// harmonizer's vectorscope with its two-way sync, and GUI state kept under darktable's conf keys.
//   QT_QPA_PLATFORM=offscreen /usr/lib/qt6/bin/qmltestrunner -input omalux/tests/components/tst_rest.qml

Item {
    id: top
    width: 400; height: 1400
    EditorTheme { id: th }
    property var requests: []
    property var changes: []
    ModuleTools {
        id: tools
        available: JSON.stringify([{ operation: "relight", tool: "center" }, { operation: "borders", tool: "frame_color" },
                                   { operation: "toneequal", tool: "exposure_boost" }, { operation: "colormapping", tool: "acquire_source" },
                                   { operation: "colormapping", tool: "acquire_target" }, { operation: "colorchecker", tool: "patch" },
                                   { operation: "colorharmonizer", tool: "set_from_vectorscope" }, { operation: "exposure", tool: "spot" }])
        onRunRequested: (operation, instance, request) => requests = requests.concat([{ operation: operation, instance: instance, request: request }])
    }
    function row(field, widget, path, extra) {
        return Object.assign({ field: field, path: path, label: field, tab: null, section: null, widget: widget, unit: "", factor: 1, offset: 0,
                               digits: 2, min: 0, max: 1, soft_min: 0, soft_max: 1, default: 0, values: null, visible_when: null,
                               tier: "detail", colors: "", custom: null }, extra || {})
    }
    ModuleCatalog {
        id: cat
        tools: tools
        catalog: JSON.stringify([
            { operation: "relight", instance: 0, label: "fill light", enabled: true, hidden: false,
              parameters: [{ name: "center", path: "center", value: 0.2 }] },
            { operation: "borders", instance: 0, label: "framing", enabled: true, hidden: false,
              parameters: [{ name: "frame_color", path: "frame_color", value: [1, 1, 1] }] },
            { operation: "colormapping", instance: 0, label: "color mapping", enabled: true, hidden: false,
              parameters: [{ name: "n", path: "n", value: 2 }, { name: "source_mean", path: "source_mean", value: [[10, 20], [-5, 3], [0, 0], [0, 0], [0, 0]] },
                           { name: "source_var", path: "source_var", value: [[2, 2], [1, 1], [0, 0], [0, 0], [0, 0]] },
                           { name: "target_mean", path: "target_mean", value: [[0, 0], [0, 0], [0, 0], [0, 0], [0, 0]] },
                           { name: "target_var", path: "target_var", value: [[0, 0], [0, 0], [0, 0], [0, 0], [0, 0]] }] },
            { operation: "colorharmonizer", instance: 0, label: "color harmonizer", enabled: true, hidden: false,
              parameters: [{ name: "rule", path: "rule", value: 3 }, { name: "num_custom_nodes", path: "num_custom_nodes", value: 2 }],
              derived: { "@anchor_hue": 30, "@custom_hue[0]": 10, "@custom_hue[1]": 190 } }])
    }
    Column {
        width: 360
        GeneratedRows {
            id: relightRows
            width: 360; theme: th; catalogModel: cat; expanded: true
            moduleState: cat.states["relight/0"]
            module: ({ operation: "relight", name: "fill light", tabs: [], rows: [top.row("center", "slider", "center")] })
        }
        GeneratedRows {
            id: bordersRows
            width: 360; theme: th; catalogModel: cat; expanded: true
            moduleState: cat.states["borders/0"]
            module: ({ operation: "borders", name: "framing", tabs: [], rows: [top.row("frame_color", "color", "frame_color",
                      { custom: { kind: "color", fields: ["frame_color"] }, default: [1, 1, 1] })] })
        }
        GeneratedRows {
            id: mappingRows
            width: 360; theme: th; catalogModel: cat; expanded: true
            moduleState: cat.states["colormapping/0"]
            module: ({ operation: "colormapping", name: "color mapping", tabs: [], rows: [top.row("source", "button", null), top.row("target", "button", null)] })
        }
        GeneratedRows {
            id: harmonizerRows
            width: 360; theme: th; catalogModel: cat; expanded: true; moreOpen: true
            moduleState: cat.states["colorharmonizer/0"]
            module: ({ operation: "colorharmonizer", name: "color harmonizer", tabs: [], rows: [
                top.row("@sync_to_vectorscope", "toggle", "@sync_to_vectorscope", { default: 1, tier: "advanced" }),
                top.row("@set_from_vectorscope", "button", null, { tier: "advanced" })] })
            onChangesRequested: c => top.changes = top.changes.concat([c])
        }
    }
    VectorscopeView {
        id: scope
        y: 900; width: 240
        theme: th
        property var edits: []
        property var turned: []
        guide: ({ type: 0, rotation: 30, width: 0 })
        onGuideEdited: (t, r, w) => edits = edits.concat([[t, r, w]])
        onCustomRotated: t => turned = turned.concat([t])
    }
    PickerBand { id: band; y: 1200; width: 200; height: 6; theme: th }
    ClusterPreview { id: clusters; y: 1220; width: 150; theme: th; count: 2; means: [[10, 20], [-5, 3]]; sigmas: [[2, 2], [1, 1]] }

    TestCase {
        name: "rest"; when: windowShown
        function init() { requests = []; changes = []; tools.cancel(); tools.results = ({}) }
        function test_specs() {
            verify(tools.sliderTool("relight", "center").band)
            verify(tools.sliderTool("borders", "frame_color"))
            compare(tools.sliderTool("toneequal", "exposure_boost").icon, "wand")
            verify(!tools.sliderTool("colorize", "hue"))                  // not in this engine list
            verify(tools.rowTool("colorchecker", "@patch").local)
            compare(tools.rowTool("colormapping", "source").before, "clusters")
        }
        function test_band() {
            const spec = tools.sliderTool("relight", "center")
            tools.toggle("relight", 0, spec, {})
            compare(requests[0].request.tool, "center")
            tools.accept("relight", 0, "center", JSON.stringify({ changed: [], band: { axis: "lightness", min: .1, mean: .4, max: .7 } }), 0)
            compare(tools.band("relight", 0, "center").mean, .4)
            tools.cancel()
            verify(!tools.band("relight", 0, "center"))               // shown only while picking
            band.band = { axis: "colorequal_hue", min: .9, mean: .95, max: .1 }   // wraps around
            verify(band.visible)
        }
        function test_swatch_picker() {
            const picker = findChildByPrefix(bordersRows, "module-tool-borders/0/frame_color/@picker")
            verify(picker)
            mouseClick(picker)
            compare(requests[0].operation, "borders")
            compare(requests[0].request.tool, "frame_color")
            compare(requests[0].request.box, [0.5, 0.5, 0.5, 0.5])   // a point picker at the centre
        }
        function findChildByPrefix(parent, prefix) {
            if (parent.objectName && parent.objectName.indexOf(prefix) === 0) return parent
            for (const c of (parent.children || [])) { const f = findChildByPrefix(c, prefix); if (f) return f }
            return null
        }
        function test_colormapping() {
            const kinds = mappingRows.items.map(it => it.kind)
            compare(kinds[0], "clusters")
            const toolsItem = mappingRows.items.find(it => it.kind === "tools")
            compare(toolsItem.specs.map(s => s.tool), ["acquire_source", "acquire_target"])
            mappingRows.runTool(toolsItem.specs[0], -1)
            compare(requests[0].request.tool, "acquire_source")
            verify(!requests[0].request.box)                          // a button, not a picker
            const rgb = clusters.labToRgb(53.39, 0, 0)
            fuzzyCompare(rgb[0], rgb[2], .02)                        // a = b = 0 is grey
        }
        function test_conf_persistence() {
            tools.storeConf("darkroom/modules/exposure/lightness", 63)
            compare(tools.confValue("darkroom/modules/exposure/lightness", 50), 63)
            compare(tools.confOf("exposure", "@lightness"), "darkroom/modules/exposure/lightness")
            compare(tools.confValue("darkroom/modules/does/not/exist", 7), 7)
            // the sync toggle reads its stored key when the rows are built
            compare(harmonizerRows.gui["@sync_to_vectorscope"], 1)
            harmonizerRows.setGui("@sync_to_vectorscope", 0)
            compare(tools.confValue("plugins/darkroom/colorharmonizer/sync_to_vectorscope", 1), 0)
            harmonizerRows.setGui("@sync_to_vectorscope", 1)
        }
        function test_vectorscope_view() {
            scope.edits = []
            scope.guide = { type: 7, rotation: 30, width: 0 }
            scope.setType(7)                                          // the active harmony: off
            compare(scope.edits[0], [0, 30, 0])
            scope.setType(4)
            compare(scope.edits[1], [4, 30, 0])
            scope.scroll(1, Qt.NoModifier)                            // coarse: to the next 15°
            compare(scope.edits[2][1], 45)
            scope.scroll(-1, Qt.ControlModifier)
            compare(scope.edits[3][1], 29)
            scope.scroll(1, Qt.ShiftModifier)
            compare(scope.edits[4][2], 1)
            scope.scroll(1, Qt.AltModifier)
            compare(scope.edits[5][0], 8)
            scope.customAngles = [.1, .6]
            scope.scroll(1, Qt.NoModifier)
            fuzzyCompare(scope.turned[0], 15 / 360, 1e-6)
            scope.customAngles = []
            // the triad guide's three sectors, darktable's normal width (0.5/12 of a turn)
            scope.guide = { type: 7, rotation: 90, width: 0 }
            compare(scope.sectors.length, 3)
            fuzzyCompare(scope.sectors[0].a1, 90 / 360 - 0.5 / 12, 1e-6)
            fuzzyCompare(scope.sectors[1].len, .5, 1e-6)
            scope.guide = { type: 0, rotation: 0, width: 0 }
            compare(scope.sectors.length, 0)
        }
        function test_harmonizer_sync() {
            const kinds = harmonizerRows.items.map(it => it.kind)
            compare(kinds[0], "vectorscope")
            // sync on, module enabled: its rule (complementary) and anchor go to the scope
            compare(tools.harmonyGuide.type, 4)
            compare(tools.harmonyGuide.rotation, 30)
            verify(requests.length === 0 || requests.every(r => r.request.tool === "harmony_guide" || r.request.tool === "vectorscope"))
            // a guide chosen in the scope becomes the rule and anchor
            const view = findChildByPrefix(harmonizerRows, "vectorscope-colorharmonizer")
            view.guideEdited(7, 120, 0)
            compare(changes[changes.length - 1].rule, 6)
            compare(changes[changes.length - 1]["@anchor_hue"], 120)
            compare(requests[requests.length - 1].request.gui.type, 7)
            // set from vectorscope is disabled while the sync is on
            const toolsItem = harmonizerRows.items.find(it => it.kind === "tools")
            compare(harmonizerRows.toolEntries(toolsItem.specs)[0].enabled, false)
            harmonizerRows.setGui("@sync_to_vectorscope", 0)
            compare(harmonizerRows.toolEntries(toolsItem.specs)[0].enabled, true)
            const before = changes.length
            view.guideEdited(2, 0, 0)                                 // without sync the module stays
            compare(changes.length, before)
            harmonizerRows.setGui("@sync_to_vectorscope", 1)
        }
    }
}
