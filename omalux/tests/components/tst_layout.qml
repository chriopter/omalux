import QtQuick
import QtTest
import QtQuick.Controls
import "../../ui/components"

// Readable layout of the module widgets: wrapping page tabs, notebook pages without "more",
// captions that do not repeat a row label.
//   QT_QPA_PLATFORM=offscreen /usr/lib/qt6/bin/qmltestrunner -input omalux/tests/components/tst_layout.qml

Item {
    width: 700; height: 900
    EditorTheme { id: th }

    ModuleTabs { id: fit; width: 300; theme: th; tabs: ["simple", "advanced", "masking"] }
    ModuleTabs { id: wrap; y: 40; width: 280; theme: th; tabs: ["CAT", "R", "G", "B", "colorfulness", "brightness", "gray"]
                 property int got: -1; onTabSelected: i => got = i }

    ModuleCatalog {
        id: cat
        catalog: JSON.stringify([{ operation: "demo", instance: 0, label: "demo", enabled: true, hidden: false,
            parameters: [{ name: "a", path: "a", value: 0.5 }, { name: "b", path: "b", value: 0.5 },
                         { name: "c", path: "c", value: 1 }, { name: "base", path: "base", value: [0.2, 0.3, 0.4] }] }])
    }
    function row(field, label, extra) {
        return Object.assign({ field: field, path: field, label: label, tab: null, section: null, widget: "slider", unit: "",
                               factor: 1, offset: 0, digits: 2, min: 0, max: 1, soft_min: 0, soft_max: 1, default: 0,
                               values: null, visible_when: null, tier: "detail", colors: "", custom: null }, extra || {})
    }
    GeneratedRows {
        id: rows
        y: 120; width: 320
        theme: th
        catalogModel: cat
        moduleState: cat.states["demo/0"]
        expanded: true
        module: ({ operation: "demo", name: "demo", tabs: ["simple", "advanced"], rows: [
            row("a", "a", { tab: "simple", tier: "primary" }),
            row("b", "b", { tab: "advanced", tier: "advanced" }),
            row("c", "settings", { tab: "simple", section: "settings", widget: "combobox",
                                   values: [{ value: 0, label: "x" }, { value: 1, label: "y" }] }),
            row("base", "color of the film base", { tab: "simple", section: "color of the film base", widget: "color",
                                                    custom: { kind: "color", fields: ["base"] } })
        ] })
    }

    TestCase {
        name: "layout"; when: windowShown
        function test_tabs_wrap() {
            compare(fit.layout.rows, 1, "short labels stay on one row")
            verify(wrap.layout.rows > 1, "long labels wrap")
            const metrics = Qt.createQmlObject("import QtQuick; FontMetrics {}", wrap)
            metrics.font = th.textFont
            for (let i = 0; i < wrap.tabs.length; ++i)
                verify(wrap.layout.boxes[i].w >= metrics.advanceWidth(wrap.tabs[i]), wrap.tabs[i] + " fits its tab")
            verify(wrap.implicitHeight > fit.implicitHeight)
            const last = wrap.layout.boxes[wrap.tabs.length - 1]
            mouseClick(wrap, last.x + last.w / 2, last.y + 11)
            compare(wrap.got, wrap.tabs.length - 1)
        }
        function test_advanced_page() {
            const b = rows.items.findIndex(it => it.row && it.row.field === "b")
            compare(rows.items[b].tier, "detail", "a page of only advanced rows shows them")
            verify(!rows.hasAdvanced)
            rows.tabIndex = 1
            verify(rows.visibility[b])
            rows.tabIndex = 0
        }
        function test_captions() {
            const captions = rows.items.filter(it => it.kind === "section").map(it => it.text)
            verify(captions.indexOf("settings") < 0, "no caption repeating the row label")
            verify(captions.indexOf("color of the film base") >= 0, "the swatch keeps darktable's caption")
            verify(rows.items.find(it => it.kind === "color").underCaption, "and has no label of its own")
        }
    }
}
