import QtQuick
import QtTest
import "../../ui/components"

// Detail and Effects modules on the real generated layout: rows that darktable shows only in
// some state of the module.
//   QT_QPA_PLATFORM=offscreen QML_XHR_ALLOW_FILE_READ=1 /usr/lib/qt6/bin/qmltestrunner -input omalux/tests/components/tst_detail.qml
Item {
    id: top
    width: 400; height: 900
    EditorTheme { id: th }
    function read(path) {
        const request = new XMLHttpRequest()
        request.open("GET", Qt.resolvedUrl(path), false)
        request.send()
        return request.responseText
    }
    function retouch(algorithm, fillMode) {
        return JSON.stringify([{ operation: "retouch", instance: 0, label: "retouch", enabled: true, hidden: false,
            parameters: [{ name: "algorithm", path: "algorithm", value: algorithm }, { name: "fill_mode", path: "fill_mode", value: fillMode },
                         { name: "fill_color", path: "fill_color", value: [0, 0, 0] }, { name: "fill_brightness", path: "fill_brightness", value: 0 },
                         { name: "blur_type", path: "blur_type", value: 0 }, { name: "blur_radius", path: "blur_radius", value: 10 }] }])
    }
    ModuleCatalog { id: cat; layoutText: top.read("../../design/layout.json"); catalog: top.retouch(2, 0) }
    GeneratedRows {
        id: rows
        width: 320
        theme: th
        catalogModel: cat
        moduleState: cat.states["retouch/0"]
        expanded: true
        module: cat.modulesByOperation["retouch"]
    }
    TestCase {
        name: "Detail"
        when: windowShown
        function shown() {
            return rows.items.map((it, i) => rows.visibility[i] && it.row ? it.row.field : "").filter(f => f !== "")
        }
        // retouch.c rt_show_hide_controls: fill rows with the fill algorithm, blur rows with blur.
        function test_retouch_rows_follow_the_algorithm() {
            cat.catalog = top.retouch(2, 0)
            const heal = shown()
            verify(heal.indexOf("@canvas") >= 0, "the tool row: " + JSON.stringify(heal))
            for (const f of ["fill_mode", "fill_color", "fill_brightness", "blur_type", "blur_radius"])
                verify(heal.indexOf(f) < 0, "heal hides " + f)
            cat.catalog = top.retouch(1, 0)
            verify(shown().indexOf("blur_radius") < 0 && shown().indexOf("fill_mode") < 0, "clone hides them too")
            cat.catalog = top.retouch(3, 0)
            const blur = shown()
            verify(blur.indexOf("blur_type") >= 0 && blur.indexOf("blur_radius") >= 0 && blur.indexOf("fill_mode") < 0, JSON.stringify(blur))
            cat.catalog = top.retouch(4, 0)
            const erase = shown()
            verify(erase.indexOf("fill_mode") >= 0 && erase.indexOf("fill_brightness") >= 0 && erase.indexOf("blur_type") < 0, JSON.stringify(erase))
            verify(erase.indexOf("fill_color") < 0, "the colour only with fill mode color")
            cat.catalog = top.retouch(4, 1)
            verify(shown().indexOf("fill_color") >= 0, JSON.stringify(shown()))
        }
        function test_retouch_tool_row_is_named_like_darktables_section() {
            const row = cat.modulesByOperation["retouch"].rows.find(r => r.field === "@canvas")
            compare(row.label, "retouch tools")
        }
    }
}
