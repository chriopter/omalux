import QtQuick
import QtTest
import QtQuick.Controls
import "../../ui/components"

// Interaction checks for the module widgets:
//   QT_QPA_PLATFORM=offscreen /usr/lib/qt6/bin/qmltestrunner -input omalux/tests/components/tst_components.qml

Item {
    width: 700; height: 900
    EditorTheme { id: th }
    property var edits: []
    property var lastNodes: null
    property int interactions: 0
    property int resets: 0
    CurveEditor {
        id: curve
        width: 300
        theme: th
        nodes: [{ x: 0, y: 0 }, { x: 0.5, y: 0.5 }, { x: 1, y: 1 }]
        onNodesEdited: n => { lastNodes = n; nodes = n }
        onInteractionChanged: a => interactions++
        onResetRequested: resets++
    }
    GraphView {
        id: graph
        y: 340; width: 300
        theme: th
        xs: [0, 1, 2, 3]; ys: [0, 0, 0, 0]; yMin: -1; yMax: 1
        editable: true
        onValueEdited: (i, v) => { const a = ys.slice(); a[i] = v; ys = a }
    }
    ModuleTabs { id: tabs; y: 520; width: 300; theme: th; tabs: ["a", "b", "c"]; property int got: -1; onTabSelected: i => got = i }
    ChannelChooser { id: chooser; y: 560; theme: th; options: [{label:"R",value:"r"},{label:"G",value:"g"}]; current: "r"; property var got; onChosen: v => got = v }
    ColorSwatch { id: sw; y: 600; width: 300; theme: th; color: [1, 0, 0]; property var got; onColorEdited: c => got = c }

    // Generated module rows against a hand-written layout and catalog.
    ModuleCatalog {
        id: cat
        catalog: JSON.stringify([{ operation: "demo", instance: 0, label: "demo", enabled: true, hidden: false,
            parameters: [{ name: "gain", path: "gain", value: 0.5 }, { name: "mode", path: "mode", value: 1 },
                         { name: "mix", path: "mix", value: [0.1, 0.2, 0.3] }] }])
    }
    property var demoChanges: null
    GeneratedRows {
        id: rows
        y: 650; width: 320
        theme: th
        catalogModel: cat
        moduleState: cat.states["demo/0"]
        module: ({ operation: "demo", name: "demo", tabs: [], rows: [
            { field: "gain", path: "gain", label: "gain", tab: null, section: null, widget: "slider", unit: "", factor: 100,
              offset: 0, digits: 1, min: 0, max: 1, soft_min: 0, soft_max: 1, default: 0, values: null, visible_when: null,
              tier: "primary", colors: "", custom: null },
            { field: "mode", path: "mode", label: "mode", tab: null, section: null, widget: "combobox", unit: "", factor: 1,
              offset: 0, digits: 0, min: null, max: null, soft_min: null, soft_max: null, default: 0,
              values: [{ value: 0, label: "a" }, { value: 1, label: "b" }], visible_when: null, tier: "detail", colors: "", custom: null },
            { field: "@sel", path: "@sel", label: "channel", tab: null, section: null, widget: "combobox", unit: "", factor: 1,
              offset: 0, digits: 0, min: null, max: null, soft_min: null, soft_max: null, default: 2,
              values: [{ value: 0, label: "x" }, { value: 1, label: "y" }, { value: 2, label: "z" }], visible_when: null, tier: "detail", colors: "", custom: null },
            { field: "mix", path: "mix[@sel]", label: "mix", tab: null, section: null, widget: "slider", unit: "", factor: 1,
              offset: 0, digits: 2, min: 0, max: 1, soft_min: 0, soft_max: 1, default: 0, values: null,
              visible_when: { field: "mode", in: [1] }, tier: "advanced", colors: "", custom: null },
            { field: "@pick", path: null, label: "pick", tab: null, section: null, widget: "picker", unit: "", factor: 1,
              offset: 0, digits: 0, min: null, max: null, soft_min: null, soft_max: null, default: null, values: null,
              visible_when: null, tier: "detail", colors: "", custom: { kind: "picker", fields: [] } }
        ] })
        onChangesRequested: c => demoChanges = c
    }

    ControlChoice {
        id: maskChoice
        x: 330; y: 0; width: 300
        theme: th; label: "blend"; value: 0; editable: true
        options: [{ value: 0, label: "off" }, { value: 1, label: "uniformly" }, { value: 3, label: "drawn mask" },
                  { value: 7, label: "drawn & parametric mask" }]
    }

    TestCase {
        name: "widgets"; when: windowShown
        // The colour popup takes the keys while open: Escape closes it.
        function test_swatch_escape() {
            sw.openPicker()
            const popup = sw.children.concat(sw.data).find(c => c && c.opened !== undefined && c.width === 220)
            verify(popup, "colour popup")
            tryVerify(() => popup.opened, 1000)
            keyClick(Qt.Key_Escape)
            tryVerify(() => !popup.visible, 1000, "Escape closes the colour popup")
        }
        // darktable's "&" is text, not a menu mnemonic.
        function test_choice_menu_text() {
            mouseClick(maskChoice, maskChoice.width / 2, maskChoice.height / 2)
            let texts = []
            function walk(item) {
                if (!item) return
                if (String(item).startsWith("MenuItem")) texts.push(item.text)
                for (const child of item.children || []) walk(child)
            }
            tryVerify(() => { texts = []; walk(maskChoice.Window.contentItem); return texts.length === 4 }, 1000, "menu opens")
            verify(texts.indexOf("drawn && parametric mask") >= 0, JSON.stringify(texts))
            keyClick(Qt.Key_Escape)
        }
        function px(x) { return curve.toPx(x) }
        function py(y) { return curve.y + 22 + curve.toPy(y) }
        function test_curve() {
            // drag the middle node up
            mousePress(curve, px(0.5), py(0.5) - curve.y)
            mouseMove(curve, px(0.5), py(0.8) - curve.y)
            mouseRelease(curve, px(0.5), py(0.8) - curve.y)
            verify(lastNodes !== null)
            compare(lastNodes.length, 3)
            fuzzyCompare(lastNodes[1].y, 0.8, 0.02)
            compare(interactions, 2)
            // drag cannot pass the right neighbour
            mousePress(curve, px(0.5), py(0.8) - curve.y)
            mouseMove(curve, px(1.0) + 20, py(0.8) - curve.y)
            mouseRelease(curve, px(1.0) + 20, py(0.8) - curve.y)
            verify(lastNodes[1].x < 1)
            compare(lastNodes.length, 3)
            // double-click empty adds
            mouseDoubleClickSequence(curve, px(0.25), py(0.75) - curve.y)
            compare(lastNodes.length, 4)
            // right-click a node removes
            mouseClick(curve, px(0.25), py(0.75) - curve.y, Qt.RightButton)
            compare(lastNodes.length, 3)
            // minNodes respected
            curve.minNodes = 3
            mouseClick(curve, px(0), py(0) - curve.y, Qt.RightButton)
            compare(lastNodes.length, 3)
            // keyboard
            curve.activeIndex = 0
            curve.forceActiveFocus()
            keyClick(Qt.Key_Up)
            fuzzyCompare(lastNodes[0].y, 0.005, 1e-6)
        }
        function test_graph() {
            const gy = 22 + graph.toPy(0.5)
            mousePress(graph, graph.toPx(2), gy)
            mouseRelease(graph, graph.toPx(2), gy)
            fuzzyCompare(graph.ys[2], 0.5, 0.03)
            compare(graph.ys[0], 0)
        }
        function test_generated() {
            compare(cat.resolve(cat.states["demo/0"], "mix[2]"), 0.3)
            verify(cat.writable("mix[1]"))
            verify(!cat.writable("@sel"))
            // collapsed: only the primary slider
            compare(rows.visibility.filter(v => v).length, 1)
            rows.expanded = true
            // expanded: + mode, channel selector and the picker notice; mix is advanced
            compare(rows.visibility.filter(v => v).length, 4)
            rows.moreOpen = true
            compare(rows.visibility.filter(v => v).length, 5)
            compare(rows.subst("mix[@sel]"), "mix[2]")
            compare(rows.raw("mix[@sel]"), 0.3)
            rows.edit("mix[@sel]", 0.9)
            compare(demoChanges["mix[2]"], 0.9)
            rows.term = "gai"
            compare(rows.visibility.filter(v => v).length, 1)
            rows.term = ""
        }
        function test_small() {
            mouseClick(tabs, 250, 13); compare(tabs.got, 2)
            mouseClick(chooser, 40, 11); compare(chooser.got, "g")
            sw.hsvToRgb(0, 1, 1)
            compare(sw.hex, "#ff0000")
        }
    }
}
