import QtQuick
import QtTest
import QtCore
import "../../ui/panels"
import "../../ui/components"

// The Styles pane with ~120 looks: families from the catalogue folders, one open at a time, the
// first 12 looks until "show all", favourites on top and kept in the app settings, the filter,
// short names under a family, and only the open family's tiles built.
//   QT_QPA_PLATFORM=offscreen /usr/lib/qt6/bin/qmltestrunner -input omalux/tests/components/tst_styles.qml

Item {
    id: top
    width: 352; height: 1200
    EditorTheme { id: th }
    readonly property var looks: {
        const out = [{ id: "neutral/style.dtstyle", name: "Neutral" }, { id: "chromatic/style.dtstyle", name: "Chromatic" }]
        const families = { "film": 30, "monochrome": 14, "portrait": 16, "series/late-summer": 10, "series/movie": 20, "experimental": 28 }
        for (const f in families)
            for (let i = 0; i < families[f]; ++i) {
                const name = (f === "series/late-summer" ? "Late Summer " : "") + "Look " + String(i + 1).padStart(2, "0")
                out.push({ id: f + "/" + f.replace("/", "-") + "-" + i + "/style.dtstyle", name: name })
            }
        return out.map(s => Object.assign({ description: "", previewUrl: "", error: "", modules: [] }, s))
    }
    property var applied: []
    StylesPanel {
        id: panel
        anchors.fill: parent
        theme: th
        styles: top.looks
        stylesReady: true
        photoReady: true
        busy: false
        appliedStyle: ""
        applyingStyle: ""
        errorMessage: ""
        onApplyRequested: id => top.applied.push(id)
    }
    Settings { id: stored; category: "Styles"; property string favourites: "[]" }

    TestCase {
        name: "styles"; when: windowShown
        function find(item, name) {
            if (!item) return null
            if (item.objectName === name && item.visible) return item
            for (let i = 0; i < item.children.length; ++i) { const f = find(item.children[i], name); if (f) return f }
            return null
        }
        function tiles(item) {
            let n = 0
            if (item.objectName && item.objectName.indexOf("style-apply-") === 0 && item.visible) ++n
            for (let i = 0; i < item.children.length; ++i) n += tiles(item.children[i])
            return n
        }
        function initTestCase() {
            Qt.application.organization = "omalux-tests"
            Qt.application.domain = "omalux.test"
        }
        function init() { panel.favourites = []; panel.styleQuery = ""; panel.showAll = ({}); panel.expandedStyleGroup = "film"; wait(20) }
        function test_families() {
            compare(top.looks.length, 120)
            compare(JSON.stringify(panel.families.map(f => f.name)),
                    JSON.stringify(["Monochrome", "Film", "Portrait", "Series · Late summer", "Series · Movie", "Experimental"]),
                    "families from the folders (series by their sub-folder), monochrome first, experimental last")
            compare(panel.topSections[0].name, "Basics")
        }
        function test_lazy_grid() {
            compare(tiles(panel), 2 + 12, "the basic looks and the first 12 of the open family; closed families build no tiles")
            mouseClick(find(panel, "style-show-all-film"))
            tryVerify(() => tiles(panel) === 2 + 30, 1000, "show all")
            mouseClick(find(panel, "style-group-portrait"))
            tryVerify(() => tiles(panel) === 2 + 12, 1000, "one family open at a time")
            verify(!find(panel, "style-apply-film/film-0/style.dtstyle"))
        }
        function test_favourites() {
            const id = "film/film-3/style.dtstyle"
            const tile = find(panel, "style-apply-" + id)
            mouseMove(tile, tile.width / 2, tile.height / 2)
            mouseClick(find(panel, "style-favourite-" + id))
            compare(JSON.stringify(panel.favourites), JSON.stringify([id]))
            tryVerify(() => { stored.sync(); return stored.value("favourites") === JSON.stringify([id]) }, 2000, "kept in the app settings")
            compare(panel.topSections[0].name, "★ Favourites")
            verify(find(panel, "favourite:style-apply-" + id), "shown on top")
            verify(find(panel, "style-apply-" + id), "and still in its family")
            mouseClick(find(panel, "favourite:style-apply-" + id))
            compare(top.applied[top.applied.length - 1], id, "a click on the favourite applies it")
            mouseClick(find(panel, "favourite:style-favourite-" + id))
            compare(panel.favourites.length, 0)
        }
        function test_filter_and_names() {
            panel.styleQuery = "summer"
            wait(20)
            compare(panel.families.map(f => f.id), ["series/late-summer"])
            compare(tiles(panel), 10, "matching families open with every look")
            const tile = find(panel, "style-apply-series/late-summer/series-late-summer-0/style.dtstyle").parent
            compare(tile.shortName, "Look 01", "the family's words are dropped under the family")
            panel.styleQuery = "no such look"
            wait(20)
            compare(tiles(panel), 0)
            compare(panel.matchCount("summer"), 10, "the sidebar search counts the matching looks")
        }
    }
}
