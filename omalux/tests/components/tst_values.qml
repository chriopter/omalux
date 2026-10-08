import QtQuick
import QtTest
import "../../ui/components"

// Displayed conversions ("@" paths), runtime lists and the colour checker patches:
//   QT_QPA_PLATFORM=offscreen /usr/lib/qt6/bin/qmltestrunner -input omalux/tests/components/tst_values.qml

Item {
    id: top
    width: 420; height: 1000
    EditorTheme { id: th }

    function row(field, path, widget, extra) {
        return Object.assign({ field: field, path: path, label: field, tab: null, section: null, widget: widget, unit: "",
                               factor: 1, offset: 0, digits: 1, min: 0, max: 360, soft_min: 0, soft_max: 360, default: 0,
                               values: null, visible_when: null, tier: "primary", colors: "", custom: null }, extra || {})
    }

    // A catalog entry as module_catalog.c describes it, with "derived" and "labels".
    ModuleCatalog {
        id: cat
        catalog: JSON.stringify([
            { operation: "colorbalance", instance: 0, label: "color balance", enabled: true, hidden: false,
              parameters: [{ name: "lift", path: "lift", value: [1, 0.5, 1.5, 0.5] }],
              derived: { "@lift_hue": 120, "@lift_saturation": 50 } },
            { operation: "lens", instance: 0, label: "lens correction", enabled: true, hidden: false,
              parameters: [{ name: "camera", path: "camera", value: "Canon EOS 6D" }],
              labels: { camera: "Canon, EOS 6D" } },
            { operation: "colorchecker", instance: 0, label: "color look up table", enabled: true, hidden: false,
              parameters: [{ name: "num_patches", path: "num_patches", value: 3 },
                           { name: "source_L", path: "source_L", value: [10, 50, 90] },
                           { name: "source_a", path: "source_a", value: [1, 2, 3] },
                           { name: "source_b", path: "source_b", value: [4, 5, 6] },
                           { name: "target_L", path: "target_L", value: [10, 55, 90] },
                           { name: "target_a", path: "target_a", value: [1, 2, 3] },
                           { name: "target_b", path: "target_b", value: [4, 5, 6] }],
              derived: { "@changed[0]": 0, "@changed[1]": 1, "@changed[2]": 0,
                         "@source_rgb[0][0]": .1, "@source_rgb[0][1]": .1, "@source_rgb[0][2]": .1,
                         "@source_rgb[1][0]": .5, "@source_rgb[1][1]": .5, "@source_rgb[1][2]": .5,
                         "@source_rgb[2][0]": .9, "@source_rgb[2][1]": .9, "@source_rgb[2][2]": .9,
                         "@target_L[0][1]": 5, "@target_L[1][1]": 55 } }
        ])
        property var asked: []
        onChoicesRequested: (operation, instance, list, query) => asked = asked.concat([[operation, instance, list, query]])
    }

    property var balanceChanges: null
    GeneratedRows {
        id: balance
        width: 360
        theme: th
        catalogModel: cat
        moduleState: cat.states["colorbalance/0"]
        module: ({ operation: "colorbalance", name: "color balance", tabs: [], rows: [
            top.row("@lift_hue", "@lift_hue", "slider"),
            top.row("@lift_saturation", "@lift_saturation", "slider", { max: 100, soft_max: 5 }),
            // reported only for some images (white balance finetune): hidden without a value
            top.row("@finetune", "@finetune", "slider", { min: -9, max: 9, soft_min: -9, soft_max: 9 })
        ] })
        onChangesRequested: c => top.balanceChanges = c
    }

    property var lensChanges: null
    GeneratedRows {
        id: lens
        y: 200; width: 360
        expanded: true
        theme: th
        catalogModel: cat
        moduleState: cat.states["lens/0"]
        module: ({ operation: "lens", name: "lens correction", tabs: [], rows: [
            top.row("camera", "camera", "choice", { custom: { kind: "choice", fields: [], list: "camera", search: true, browse: null, filters: [] } })
        ] })
        onChangesRequested: c => top.lensChanges = c
    }

    property var checkerChanges: null
    GeneratedRows {
        id: checker
        y: 260; width: 360
        theme: th
        catalogModel: cat
        moduleState: cat.states["colorchecker/0"]
        expanded: true
        moreOpen: true
        module: ({ operation: "colorchecker", name: "color look up table", tabs: [], rows: [
            top.row("patches", null, "patches", { tier: "detail", custom: { kind: "patches", fields: [] } }),
            top.row("@patch", "@patch", "text", { tier: "detail", custom: { kind: "text", fields: ["num_patches"], dynamic: true } }),
            top.row("target_L", "@target_L[@absolute_target][@patch]", "slider", { tier: "detail", min: -100, max: 200 }),
            top.row("@absolute_target", "@absolute_target", "combobox", { tier: "advanced",
                values: [{ value: 0, label: "relative" }, { value: 1, label: "absolute" }] })
        ] })
        onChangesRequested: c => top.checkerChanges = c
    }

    ChoiceRow {
        id: choice
        y: 600; width: 360
        theme: th
        label: "camera model"
        valueText: "none"
        fileFilters: ["images (*.png)"]
        property var asked: []
        property int picked: -1
        onRequested: q => asked = asked.concat([q])
        onChosen: i => picked = i
    }

    PatchGrid {
        id: grid
        y: 660; width: 240
        theme: th
        count: 7
        colors: [[1, 0, 0], [0, 1, 0], [0, 0, 1], [1, 1, 0], [0, 1, 1], [1, 0, 1], [.5, .5, .5]]
        changed: [false, true]
        property int selected: -1
        property int resetIndex: -1
        property int removed: -1
        onPatchSelected: i => selected = i
        onPatchReset: i => resetIndex = i
        onPatchRemoved: i => removed = i
    }

    function find(item, name) {
        if (!item) return null
        if (item.objectName === name) return item
        const kids = item.children || []
        for (let i = 0; i < kids.length; ++i) {
            const found = find(kids[i], name)
            if (found) return found
        }
        return null
    }

    TestCase {
        name: "values"; when: windowShown

        function test_catalog() {
            const state = cat.states["colorbalance/0"]
            compare(cat.resolve(state, "@lift_hue"), 120)
            verify(cat.writable("@lift_hue", state))
            verify(!cat.writable("@nothing", state))
            verify(!cat.writable("@lift_hue"))
            compare(cat.states["lens/0"].labels.camera, "Canon, EOS 6D")
            cat.requestChoices("lens", 0, "camera", "eos")
            compare(cat.asked[cat.asked.length - 1].join("|"), "lens|0|camera|eos")
            verify(cat.choiceResults["lens/0/camera"].loading)
            cat.receiveChoices("lens", 0, "camera", "eos", JSON.stringify({ items: [{ label: "Canon, EOS 6D", set: { camera: "Canon EOS 6D" } }], current: 0, more: 2 }))
            compare(cat.choiceResults["lens/0/camera"].items.length, 1)
            compare(cat.choiceResults["lens/0/camera"].more, 2)
            verify(!cat.choiceResults["lens/0/camera"].loading)
        }

        function test_derivedSliders() {
            // both reported conversions are shown, the unreported one is hidden
            compare(balance.visibility.filter(v => v).length, 2)
            verify(balance.canWrite("@lift_hue"))
            balance.editRaw(balance.module.rows[0], 200)
            compare(top.balanceChanges["@lift_hue"], 200)
        }

        function test_choiceRows() {
            const row = top.find(lens, "choice-lens-camera")
            verify(row !== null)
            compare(row.valueText, "Canon, EOS 6D")
            row.open()
            compare(cat.asked[cat.asked.length - 1].join("|"), "lens|0|camera|")
            cat.receiveChoices("lens", 0, "camera", "", JSON.stringify({ items: [
                { label: "Canon, EOS 6D", set: { camera: "Canon EOS 6D", crop: 1, has_been_set: 1 } },
                { label: "Canon, EOS 7D", set: { camera: "Canon EOS 7D", crop: 1.6, has_been_set: 1 } }], current: 0, more: 0 }))
            compare(row.items.length, 2)
            row.pick(1)
            compare(top.lensChanges.camera, "Canon EOS 7D")
            compare(top.lensChanges.crop, 1.6)
            verify(!row.popup.visible)
        }

        function test_choiceRowWidget() {
            mouseClick(choice, 40, 12)
            verify(choice.popup.visible)
            compare(choice.asked.join("|"), "")
            choice.items = [{ label: "first" }, { label: "second", detail: "x", section: "group" }]
            choice.current = 0
            wait(50)
            const search = top.find(choice.popup.contentItem, "choice-search")
            verify(search !== null)
            search.text = "sec"
            search.textEdited()
            tryCompare(choice, "asked", ["", "sec"])
            keyClick(Qt.Key_Down)
            keyClick(Qt.Key_Return)
            compare(choice.picked, 1)
            verify(!choice.popup.visible)
            // a row without a list opens only the file dialog
            choice.listed = false
            choice.open()
            verify(!choice.popup.visible)
        }

        function test_patchGrid() {
            // 6 × 4 cells for up to 24 patches; patch 4 is the fifth cell of the first row
            const cw = grid.width / 6, ch = grid.height / 4
            mouseClick(grid, cw * 4.5, ch * .5)
            compare(grid.selected, 4)
            mouseClick(grid, cw * .5, ch * 1.5)
            compare(grid.selected, 6)
            mouseClick(grid, cw * 1.5, ch * .5, Qt.RightButton)
            compare(grid.removed, 1)
            mouseDoubleClickSequence(grid, cw * 2.5, ch * .5)
            compare(grid.resetIndex, 2)
            // outside the patches nothing happens
            grid.selected = -1
            mouseClick(grid, cw * 5.5, ch * 3.5)
            compare(grid.selected, -1)
        }

        function test_checkerRows() {
            compare(checker.subst("@target_L[@absolute_target][@patch]"), "@target_L[0][0]")
            checker.setGui("@patch", 1)
            compare(checker.raw("@target_L[@absolute_target][@patch]"), 5)
            checker.setGui("@absolute_target", 1)
            compare(checker.raw("@target_L[@absolute_target][@patch]"), 55)
            const patches = top.find(checker, "patches-colorchecker")
            verify(patches !== null)
            compare(patches.count, 3)
            compare(patches.changed.join(","), "false,true,false")
            patches.patchSelected(2)
            compare(checker.gui["@patch"], 2)
            // double-click: the target goes back to the source
            patches.patchReset(1)
            compare(top.checkerChanges["target_L[1]"], 50)
            // right-click on patch 0: later patches move up, the count drops
            patches.patchRemoved(0)
            compare(top.checkerChanges["num_patches"], 2)
            compare(top.checkerChanges["source_L[0]"], 50)
            compare(top.checkerChanges["target_L[1]"], 90)
            compare(checker.gui["@patch"], 1)
        }
    }
}
