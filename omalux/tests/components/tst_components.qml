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
    // ---- the open module: heading, section rows, body ----------------------------------------
    property var headerGot: []
    Item {
        x: 360; y: 600; width: 300; height: 200
        ModuleHeader {
            id: header
            x: 0; width: 300
            theme: th
            operation: "exposure"; name: "contrast brightness saturation"
            moduleState: ({ canNew: true })
            expanded: true
            onToggleRequested: headerGot.push("toggle")
            onExpansionRequested: headerGot.push("fold")
            onResetRequested: headerGot.push("reset")
        }
        SectionRow {
            id: sectionRow
            objectName: "demo-section"
            y: 40; width: 286
            theme: th
            label: "blending"; summary: "off"
            property int got: 0
            onRequested: { ++got; open = !open }
        }
        ModuleBody {
            id: moduleBody
            x: 0; y: 80; width: 300
            Rectangle { id: bodyFirst; width: 50; height: 20 }
            Rectangle { id: bodySecond; width: 50; height: 60; visible: false }
        }
    }
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
            parameters: [{ name: "gain", path: "gain", value: 0.5, default: 0, reset: 0.25 }, { name: "mode", path: "mode", value: 1 },
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

    // Sliders as darktable steps and stores them, and one range for a row wherever it is shown.
    property real stepped: NaN
    ControlSlider {
        id: evSlider
        x: 330; y: 60; width: 300
        theme: th; editable: true; value: 0
        control: ({ id: "ev", label: "exposure", unit: " EV", colors: "", decimals: 3, step: .01, factor: 1, offset: 0,
                    minimum: -18, maximum: 18, softMinimum: -3, softMaximum: 4, section: "exposure" })
        onEdited: v => stepped = v
    }
    ControlSlider {
        id: anySlider
        x: 330; y: 120; width: 300
        theme: th; editable: true; value: 0
        control: ({ id: "any", label: "any", unit: "", colors: "", decimals: 2, step: .01, minimum: 0, maximum: 1,
                    softMinimum: 0, softMaximum: 1, section: "any" })
    }
    FilterModule {
        id: vignetting
        x: 330; y: 180; width: 300
        theme: th; editable: true; activeControl: ""; expanded: false
        values: ({ vignette: -0.5, vignette_scale: 80, vignette_enabled: 1 })
        section: ({ module: "vignette", key: "vignette", name: "vignetting", primary: ["vignette"], shortTitle: true, controls: [
            { id: "vignette", label: "brightness", module: "vignette", unit: "", colors: "light", decimals: 3, step: .01, factor: 1, offset: 0,
              minimum: -1, maximum: 1, softMinimum: -1, softMaximum: 1, section: "vignetting", initial: -0.5 },
            { id: "vignette_scale", label: "fall-off start", module: "vignette", unit: "%", colors: "", decimals: 2, step: 1, factor: 1, offset: 0,
              minimum: 0, maximum: 200, softMinimum: 0, softMaximum: 200, section: "vignetting", initial: 80 }] })
    }

    TestCase {
        name: "widgets"; when: windowShown
        function slider(item, id) {
            if (item.objectName === "control-slider-" + id) return item
            for (const child of item.children) { const f = slider(child, id); if (f) return f }
            return null
        }
        // bauhaus.c dt_bauhaus_slider_get_step: a hundredth of the shown range as 1 or 5 of a
        // decade of the displayed value, one native unit from a range of 100.
        function test_darktable_step() {
            const shown = (lo, hi, factor, offset) => {
                anySlider.control = Object.assign({}, anySlider.control, { softMinimum: lo, softMaximum: hi, minimum: lo, maximum: hi,
                                                                             factor: factor, offset: offset })
                return anySlider.keyStep
            }
            fuzzyCompare(evSlider.keyStep, 0.05, 1e-9, "exposure -3 … 4 EV")
            fuzzyCompare(shown(0.7, 3, 1, 0), 0.01, 1e-9, "sigmoid contrast")
            fuzzyCompare(shown(-100, 100, 1, 0), 1, 1e-9, "shadows -100 … 100")
            fuzzyCompare(shown(-10, 10, 1, 0), 0.1, 1e-9, "white point adjustment")
            fuzzyCompare(shown(1901, 25000, 1, 0), 1, 1e-9, "temperature in K")
            fuzzyCompare(shown(0, 100, 100, 0), 1, 1e-9, "a 0 … 1 parameter shown in percent")
            fuzzyCompare(shown(0, 500, 100, 100), 5, 1e-9, "local contrast detail, -1 … 4 shown as 0 … 500 %")
            fuzzyCompare(shown(20, 6400, 213.2, 0), 50, 1e-9, "grain coarseness in ISO")
            fuzzyCompare(shown(-0.1, 0.1, 1, 0), 0.001, 1e-12, "black level correction")
        }
        // A drag stores what darktable would: the value rounded to the digits it shows.
        function test_slider_stores_shown_digits() {
            const s = slider(evSlider, "ev")
            stepped = NaN
            mousePress(s, 100, s.height / 2); mouseMove(s, 137, s.height / 2); mouseMove(s, 171.3, s.height / 2)
            mouseRelease(s, 171.3, s.height / 2)
            verify(!isNaN(stepped), "the drag edits")
            compare(stepped, Number(stepped.toFixed(3)), "three digits, as displayed")
            evSlider.value = 0.2
            evSlider.step(1)
            compare(stepped, 0.25, "one key step")
            evSlider.step(-0.1)
            compare(stepped, 0.195, "a tenth step")
            evSlider.value = 0
        }
        // The same value puts the knob at the same place collapsed and unfolded.
        function test_same_range_collapsed_and_unfolded() {
            const s = slider(vignetting, "vignette")
            const collapsed = [s.from, s.to, s.visualPosition]
            vignetting.expanded = true
            compare([s.from, s.to, s.visualPosition], collapsed)
            compare([s.from, s.to], [-1, 1], "darktable's range")
            fuzzyCompare(s.visualPosition, 0.25, 1e-6)
            vignetting.expanded = false
        }
        function named(item, name) {
            if (item.objectName === name) return item
            for (const child of item.children) { const f = named(child, name); if (f) return f }
            return null
        }
        // Unfolding keeps the main row where and how it was; the module's other parameters come
        // beneath it. The main row's own parameter is not repeated.
        function test_kept_main_row() {
            vignetting.controlDefault = id => id === "vignette" ? -0.5 : 80
            const main = named(vignetting, "filter-control-vignette")
            verify(main && main.visible)
            const before = [main.mapToItem(vignetting.parent, 0, 0).y, main.height, main.valueText, main.navTarget.navId]
            verify(!named(vignetting, "filter-sub-vignette").visible, "folded: the row alone")
            vignetting.expanded = true
            wait(250)
            compare([main.mapToItem(vignetting.parent, 0, 0).y, main.height, main.valueText, main.navTarget.navId], before, "the row did not move or change")
            verify(main.detailsExpanded)
            const repeated = named(vignetting, "filter-sub-vignette"), other = named(vignetting, "filter-sub-vignette_scale")
            verify(!repeated.visible, "the main row's parameter is not listed again")
            verify(other.visible && other.sub && !other.linked)
            verify(other.mapToItem(vignetting, 0, 0).x > main.mapToItem(vignetting, 0, 0).x, "sub-rows are indented")
            verify(other.navTarget.listed && main.navTarget.listed)
            verify(!named(vignetting, "control-linked-vignette_scale"), "no mark on an ordinary sub-row")
            compare(named(vignetting, "module-strip-vignette").text, "vignetting")
            verify(!named(vignetting, "module-toggle-vignette").visible, "no card heading above a kept row")
            vignetting.values = ({ vignette: 0.25, vignette_scale: 80, vignette_enabled: 1 })
            compare(main.valueText, "0.250")
            // The row's reset appears on hover, away from the default only, and asks for that
            // one parameter; nothing moves when it appears.
            const y = main.mapToItem(vignetting.parent, 0, 0).y, labelWidth = named(main, "control-slider-vignette").width
            mouseMove(main, 20, 8)
            tryVerify(() => !!named(main, "control-reset-vignette"))
            compare(main.mapToItem(vignetting.parent, 0, 0).y, y)
            compare(named(main, "control-slider-vignette").width, labelWidth)
            verify(!main.atDefault)
            vignetting.values = ({ vignette: -0.5, vignette_scale: 80, vignette_enabled: 1 })
            verify(main.atDefault)
            tryVerify(() => !named(main, "control-reset-vignette"), 1000, "at the default: no button")
            mouseMove(vignetting, 5, vignetting.height + 30)
            vignetting.expanded = false
            tryVerify(() => !other.visible)
            compare(main.mapToItem(vignetting.parent, 0, 0).y, before[0])
        }
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
            // Reset restores the module's own default for this image, where the engine has one.
            compare(rows.defaultOf(rows.module.rows[0]), 0.25)
            compare(rows.defaultOf(rows.module.rows[1]), 0, "the layout's default otherwise")
            rows.term = "gai"
            compare(rows.visibility.filter(v => v).length, 1)
            rows.term = ""
        }
        function find(item, name) {
            if (item.objectName === name) return item
            for (const child of item.children) { const f = find(child, name); if (f) return f }
            return null
        }
        function test_open_module() {
            // The mark, icon and name switch the module; the rest of the heading and the chevron
            // fold it; reset is offered while it is open.
            const toggle = find(header, "module-toggle-exposure"), fold = find(header, "module-fold-exposure")
            const chevron = find(header, "module-details-exposure"), reset = find(header, "module-reset-exposure")
            verify(toggle && fold && chevron && reset)
            headerGot = []
            mouseClick(toggle); compare(headerGot, ["toggle"])
            const gap = header.mapFromItem(toggle, toggle.width, 0).x + 12
            verify(gap < header.mapFromItem(reset, 0, 0).x, "a long name leaves room to fold")
            mouseClick(header, gap, header.height / 2); compare(headerGot, ["toggle", "fold"])
            mouseClick(chevron); compare(headerGot, ["toggle", "fold", "fold"])
            mouseClick(reset); compare(headerGot, ["toggle", "fold", "fold", "reset"])
            // The chevron stands in the disclosure column, where a collapsed row has it.
            compare(header.mapFromItem(chevron, chevron.width, 0).x, header.width)
            header.expanded = false
            verify(!reset.visible)
            header.hasDetails = false
            mouseClick(header, gap, header.height / 2); compare(headerGot.length, 4)
            header.hasDetails = true; header.expanded = true
            // A section row: one click target across the row, summary only while closed.
            mouseClick(sectionRow, 5, 20); compare(sectionRow.got, 1); verify(sectionRow.open)
            mouseClick(sectionRow, sectionRow.width - 8, 20); compare(sectionRow.got, 2); verify(!sectionRow.open)
            // The body grows in a short movement and gives height back at once.
            compare(moduleBody.height, 20)
            bodySecond.visible = true
            tryVerify(() => moduleBody.moving)
            tryCompare(moduleBody, "height", 80, 1000)
            verify(!moduleBody.clip)
            bodySecond.visible = false
            tryCompare(moduleBody, "height", 20, 1000)
            verify(!moduleBody.moving)
        }
        function test_small() {
            mouseClick(tabs, 250, 13); compare(tabs.got, 2)
            mouseClick(chooser, 40, 11); compare(chooser.got, "g")
            sw.hsvToRgb(0, 1, 1)
            compare(sw.hex, "#ff0000")
        }
    }
}
