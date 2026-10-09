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
        // A family named in the catalogue (family.json): the backend hands its display name and
        // place with every look, because "Dhh" from the folder would be wrong.
        const declared = { "dhh/black-and-white": ["DHH · Black & white", 25], "dhh/kodak": ["DHH · Kodak", 21], "dhh/fuji": ["DHH · Fuji", 44] }
        for (const f in declared)
            for (let i = 0; i < declared[f][1]; ++i)
                out.push({ id: f + "/" + f.replace("/", "-") + "-" + i + "/style.dtstyle", family: declared[f][0], familyOrder: 1,
                           name: (f === "dhh/kodak" ? "Kodak Portra " : f === "dhh/fuji" ? "Fuji Superia " : "Ilford HP") + (i + 1) })
        return out.map(s => Object.assign({ description: "", previewUrl: "", error: "", modules: [], family: "", familyOrder: 0 }, s))
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
            compare(top.looks.length, 210)
            compare(JSON.stringify(panel.families.map(f => f.name)),
                    JSON.stringify(["Monochrome", "Film", "Portrait", "Series · Late summer", "Series · Movie",
                                    "DHH · Black & white", "DHH · Fuji", "DHH · Kodak", "Experimental"]),
                    "families from the folders (series by their sub-folder), monochrome first, experimental last; "
                    + "a declared family carries its own name and comes after the undeclared ones")
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
        function test_declared_family() {
            const heading = find(panel, "style-group-dhh/kodak")
            verify(heading, "the folder stays the id")
            compare(heading.Accessible.name, "DHH · Kodak", "the heading reads the declared name, not “Dhh · Kodak”")
            mouseClick(heading)
            tryVerify(() => tiles(panel) === 2 + 12, 1000, "the first 12 of 21")
            const tile = find(panel, "style-apply-dhh/kodak/dhh-kodak-0/style.dtstyle").parent
            compare(tile.shortName, "Portra 1", "the last word of the declared name is dropped: Kodak Portra 1")
            mouseClick(find(panel, "style-group-dhh/fuji"))
            tryVerify(() => !!find(panel, "style-show-all-dhh/fuji"), 1000)
            wait(300)  // the grid settles before the button below it is where a click lands
            mouseClick(find(panel, "style-show-all-dhh/fuji"))
            tryVerify(() => tiles(panel) === 2 + 44, 1000, "show all of a large group")
            panel.styleQuery = "dhh"
            wait(20)
            compare(JSON.stringify(panel.families.map(f => f.id)), JSON.stringify(["dhh/black-and-white", "dhh/fuji", "dhh/kodak"]),
                    "the declared name is searchable")
            compare(tiles(panel), 90, "every look of the family, all groups open")
            panel.styleQuery = "black & white"
            wait(20)
            compare(tiles(panel), 25)
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
