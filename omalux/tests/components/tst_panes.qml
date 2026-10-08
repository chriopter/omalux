import QtQuick
import QtTest
import "../../ui/components"

// The Tone, Color, Detail and Effects panes on the real generated layout: a short summary with
// plain labels, every module of a pane reachable (summary or Advanced), an icon for every module
// and the summary following the tone mapper the image uses.
//   QML_XHR_ALLOW_FILE_READ=1 QT_QPA_PLATFORM=offscreen /usr/lib/qt6/bin/qmltestrunner -input omalux/tests/components/tst_panes.qml

Item {
    id: top
    function read(path) {
        const request = new XMLHttpRequest()
        request.open("GET", Qt.resolvedUrl(path), false)
        request.send()
        return request.status === 200 || request.status === 0 ? request.responseText : ""
    }
    readonly property var registry: JSON.parse(read("../../design/controls.json")).controls
    ModuleCatalog { id: cat; layoutText: top.read("../../design/layout.json") }

    TestCase {
        name: "panes"
        readonly property var tabs: ["tone", "color", "detail", "effects"]
        function moduleOf(block) {
            return block.module ? block.module.operation : top.registry.find(c => c.id === block.control).module
        }
        function test_summary_is_short_and_plain() {
            for (const tab of tabs) {
                const blocks = cat.summaryFor(tab)
                const rows = blocks.reduce((n, b) => n + (b.rows ? b.rows.length : 1), 0)
                verify(rows >= 3 && rows <= 10, tab + ": " + rows + " summary rows")
                for (const b of blocks)
                    for (const label of b.rows ? b.rows.map(r => r.label) : [b.label])
                        verify(label && !/^[-+−]?\d/.test(label) && label.length <= 18, tab + ": plain short label " + label)
            }
        }
        function test_every_module_reachable() {
            for (const tab of tabs) {
                const shown = cat.summaryFor(tab).map(b => moduleOf(b))
                const listed = shown.slice()
                for (const g of cat.advancedFor(tab, shown)) for (const m of g.modules) listed.push(m.operation)
                for (const g of cat.groupsForTab(tab))
                    for (const m of g.modules)
                        verify(listed.includes(m.operation), tab + ": " + m.operation + " is in the summary or under Advanced")
                compare(cat.advancedFor(tab, shown)[0].id, "advanced", tab + " starts with Advanced")
                for (const op of shown) {
                    const again = cat.advancedFor(tab, shown).some(g => g.modules.some(m => m.operation === op))
                    verify(!again, tab + ": " + op + " is not repeated under Advanced")
                }
            }
        }
        function test_icons() {
            for (const g of cat.layout.groups)
                for (const m of g.modules)
                    verify(top.read("../../../assets/icons/module-" + m.operation + ".svg").indexOf("<svg") === 0, "icon for " + m.operation)
        }
        function test_display_labels() {
            const te = cat.modulesByOperation["toneequal"].rows.find(r => r.field === "deep_blacks")
            compare(te.label, "-6 EV", "darktable's label stays")
            compare(te.display, "deep blacks -6 EV", "shown with what the band is")
        }
        function test_active_tone_mapper() {
            const labels = () => cat.summaryFor("tone").filter(b => b.module && b.module.operation !== "toneequal")
                                                     .map(b => b.module.operation + ":" + b.rows.map(r => r.label).join("/"))
            cat.catalog = "[]"
            compare(labels(), ["sigmoid:contrast"], "sigmoid when the image uses no tone mapper")
            cat.catalog = JSON.stringify([{ operation: "filmicrgb", instance: 0, label: "filmic rgb", enabled: true, hidden: false,
                                            parameters: [{ name: "contrast", path: "contrast", value: 1 }] }])
            compare(labels(), ["filmicrgb:contrast/white point/black point"], "the tone mapper the image uses")
            const shown = cat.summaryFor("tone").map(b => moduleOf(b))
            const advanced = cat.advancedFor("tone", shown)[0].modules.map(m => m.operation)
            verify(advanced.includes("sigmoid") && !advanced.includes("filmicrgb"), JSON.stringify(advanced))
        }
    }
}
