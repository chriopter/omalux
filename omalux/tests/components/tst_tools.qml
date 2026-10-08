import QtQuick
import QtTest
import "../../ui/components"

// Pickers and module buttons without an engine: ModuleTools state, the picker overlay, the
// tool buttons, the histogram and the rows GeneratedRows builds from them.
//   QT_QPA_PLATFORM=offscreen /usr/lib/qt6/bin/qmltestrunner -input omalux/tests/components/tst_tools.qml

Item {
    width: 400; height: 900
    EditorTheme { id: th }
    property var requests: []
    ModuleTools {
        id: tools
        available: JSON.stringify([{ operation: "rgblevels", tool: "black" }, { operation: "rgblevels", tool: "auto" },
                                   { operation: "rgblevels", tool: "auto_region" }, { operation: "rgblevels", tool: "histogram" },
                                   { operation: "colorbalance", tool: "auto_luma" }, { operation: "rgbcurve", tool: "show_color" },
                                   { operation: "filmicrgb", tool: "white_point_source" }])
        onRunRequested: (operation, instance, request) => requests = requests.concat([{ operation: operation, instance: instance, request: request }])
    }
    ModuleCatalog {
        id: cat
        tools: tools
        catalog: JSON.stringify([{ operation: "rgblevels", instance: 0, label: "rgb levels", enabled: true, hidden: false,
            parameters: [{ name: "autoscale", path: "autoscale", value: 0 },
                         { name: "levels", path: "levels", value: [[0, 0.5, 1], [0, 0.5, 1], [0, 0.5, 1]] }] }])
    }
    GeneratedRows {
        id: rows
        width: 320
        theme: th
        catalogModel: cat
        moduleState: cat.states["rgblevels/0"]
        expanded: true
        module: ({ operation: "rgblevels", name: "rgb levels", tabs: [], rows: [
            { field: "@black", path: null, label: "black", tab: null, section: null, widget: "picker", unit: "", factor: 1, offset: 0,
              digits: 0, min: null, max: null, soft_min: null, soft_max: null, default: null, values: null, visible_when: null,
              tier: "detail", colors: "", custom: { kind: "picker", fields: [] } },
            { field: "@auto", path: null, label: "auto", tab: null, section: null, widget: "button", unit: "", factor: 1, offset: 0,
              digits: 0, min: null, max: null, soft_min: null, soft_max: null, default: null, values: null, visible_when: null,
              tier: "detail", colors: "", custom: null },
            { field: "@gray", path: null, label: "gray", tab: null, section: null, widget: "picker", unit: "", factor: 1, offset: 0,
              digits: 0, min: null, max: null, soft_min: null, soft_max: null, default: null, values: null, visible_when: null,
              tier: "detail", colors: "", custom: { kind: "picker", fields: [] } }
        ] })
    }
    PickerOverlay {
        id: overlay
        y: 300; width: 200; height: 100
        property var got: null
        property int mods: 0
        onBoxEdited: (box, modifiers) => { got = box; mods = modifiers }
    }
    ModuleToolButtons {
        id: buttons
        y: 420; width: 300
        theme: th
        entries: [{ label: "auto", kind: "button" }, { label: "menu", kind: "button", menu: [{ label: "a" }, { label: "b" }] },
                  { label: "pick", kind: "area", active: true }]
        property var got: []
        onTriggered: (index, choice) => got = got.concat([[index, choice]])
    }
    HistogramView {
        id: histogram
        y: 470; width: 256; height: 64
        theme: th
        histogram: ({ channels: [[1, 2, 3], [0, 0, 0], [0, 0, 0], [0, 0, 0]], max: [3, 0, 0, 0] })
        markers: [0, .5, 1]
    }

    TestCase {
        name: "tools"; when: windowShown
        function init() { requests = []; tools.cancel(); tools.results = ({}); tools.boxes = ({}) }
        function test_specs() {
            verify(tools.rowTool("rgblevels", "@black"))
            verify(!tools.rowTool("rgblevels", "@white"))           // not implemented by this engine list
            verify(tools.sliderTool("filmicrgb", "white_point_source"))
            compare(tools.histogramField("rgblevels"), "levels")
            compare(tools.histogramField("levels"), "")
        }
        function test_picker_cycle() {
            const spec = tools.rowTool("rgblevels", "@black")
            tools.toggle("rgblevels", 0, spec, { "@tab": 1 })
            verify(tools.isActive("rgblevels", 0, "black"))
            compare(requests.length, 1)
            // a point picker starts at the image centre, darktable's default point
            compare(requests[0].request.box, [0.5, 0.5, 0.5, 0.5])
            compare(requests[0].request.gui["@tab"], 1)
            tools.setBox([0.1, 0.2, 0.1, 0.2], Qt.ControlModifier)
            compare(requests.length, 2)
            compare(requests[1].request.gui["@picker_modifier"], 1)
            compare(tools.activeBox(), [0.1, 0.2, 0.1, 0.2])
            // editing the module's parameters switches its picker off
            tools.parameterEdited("rgblevels", 0)
            verify(!tools.active)
            // the box is remembered for the next activation
            tools.toggle("rgblevels", 0, spec, {})
            compare(requests[2].request.box, [0.1, 0.2, 0.1, 0.2])
            tools.toggle("rgblevels", 0, spec, {})
            verify(!tools.active)
        }
        function test_area_default_and_buttons() {
            tools.toggle("rgblevels", 0, tools.rowTool("rgblevels", "@auto_region"), {})
            compare(requests[0].request.box, [0.02, 0.02, 0.98, 0.98])
            tools.toggle("rgblevels", 0, tools.rowTool("rgblevels", "@auto"), {})
            compare(requests[1].request.tool, "auto")
            verify(requests[1].request.box === undefined)
            // a button does not change the active picker
            verify(tools.isActive("rgblevels", 0, "auto_region"))
        }
        function test_keep_active_and_one_shot() {
            tools.toggle("rgbcurve", 0, tools.rowTool("rgbcurve", "@show_color"), {})
            tools.parameterEdited("rgbcurve", 0)
            verify(tools.active)                                      // keep-active picker
            tools.toggle("colorbalance", 0, tools.rowTool("colorbalance", "@auto_luma"), {})
            verify(tools.isActive("colorbalance", 0, "auto_luma"))
            tools.accept("colorbalance", 0, "auto_luma", JSON.stringify({ changed: ["colorbalance"], gui: { "@luma_lift": .1 } }), 0)
            verify(!tools.active)                                     // the optimiser switched itself off
            compare(tools.results["colorbalance/0/auto_luma"].gui["@luma_lift"], .1)
        }
        function test_histogram_result() {
            tools.accept("rgblevels", 0, "histogram", JSON.stringify({ histogram: { channels: [[1]], max: [1] } }), 0)
            compare(tools.histogramData["rgblevels/0"].max[0], 1)
        }
        function test_rows() {
            // black and auto share one row (darktable's button box), gray follows it
            const kinds = rows.items.map(it => it.kind)
            compare(kinds.filter(k => k === "tools").length, 1)
            const toolsItem = rows.items.find(it => it.kind === "tools")
            compare(toolsItem.specs.map(s => s.tool), ["black", "auto"])
            verify(rows.items.some(it => it.kind === "notice"))       // gray is not implemented here
            rows.runTool(toolsItem.specs[1], -1)
            compare(requests[requests.length - 1].request.tool, "auto")
        }
        function test_overlay() {
            overlay.kind = "area"
            overlay.box = [0.8, 0.8, 0.9, 0.9]   // outside the box: draw a new one
            mousePress(overlay, 20, 20); mouseMove(overlay, 100, 60); mouseRelease(overlay, 100, 60)
            fuzzyCompare(overlay.got[0], .1, .01); fuzzyCompare(overlay.got[3], .6, .01)
            // inside darktable's default box (almost the whole photo) a drag draws a new area too
            overlay.box = [0.02, 0.02, 0.98, 0.98]
            mousePress(overlay, 50, 40); mouseMove(overlay, 150, 80); mouseRelease(overlay, 150, 80)
            fuzzyCompare(overlay.got[0], .25, .01); fuzzyCompare(overlay.got[2], .75, .01)
            fuzzyCompare(overlay.got[1], .4, .01); fuzzyCompare(overlay.got[3], .8, .01)
            // a corner resizes it
            overlay.box = overlay.got
            mousePress(overlay, 150, 80); mouseMove(overlay, 180, 90); mouseRelease(overlay, 180, 90)
            fuzzyCompare(overlay.got[0], .25, .01); fuzzyCompare(overlay.got[2], .9, .01)
            // a click of a point-or-area picker picks a point, with the modifiers
            overlay.kind = "pointarea"
            mousePress(overlay, 100, 50, Qt.LeftButton, Qt.ControlModifier)
            mouseRelease(overlay, 100, 50, Qt.LeftButton, Qt.ControlModifier)
            compare(overlay.got[0], overlay.got[2])
            fuzzyCompare(overlay.got[0], .5, .01)
            compare(overlay.mods & Qt.ControlModifier, Qt.ControlModifier)
        }
        function test_buttons() {
            mouseClick(buttons, 30, 14)
            compare(buttons.got[0], [0, -1])
            verify(histogram.visible)
        }
    }
}
