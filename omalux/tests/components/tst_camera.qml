import QtQuick
import QtTest
import "../../ui/panels"
import "../../ui/components"
import "../../ui/components/keyboard.js" as Keys

// The Camera pane of the Styles area: the presets matched for this camera on top, and below
// them every camera preset by maker, to put on the photograph by hand.
//   QT_QPA_PLATFORM=offscreen /usr/lib/qt6/bin/qmltestrunner -input omalux/tests/components/tst_camera.qml

Item {
    id: top
    width: 352; height: 900
    EditorTheme { id: th }
    function preset(maker, model, title, operation, module, more) {
        return Object.assign({ name: "Omalux " + maker + " " + model + ": " + title, title: title,
                               description: (maker + " " + model).trim() + ": " + title, maker: maker, model: model,
                               operation: operation, module: module, matches: false, makerMatches: false,
                               general: false, applied: false, available: true, reason: "" }, more || {})
    }
    // As the engine lists them for a JPEG without camera data.
    function unmatched() {
        return [
            preset("Every camera", "", "colour noise reduction", "nlmeans", "astrophoto denoise", { general: true }),
            preset("Every camera", "", "capture sharpening", "sharpen", "sharpen", { general: true }),
            preset("Canon", "EOS 6D", "exposure", "rgblevels", "rgb levels"),
            preset("Canon", "EOS 70D", "exposure", "rgblevels", "rgb levels"),
            preset("Fujifilm", "", "embedded lens correction", "lens", "lens correction",
                   { available: false, reason: "this photo carries no embedded lens data" }),
            preset("Fujifilm", "X-T10", "exposure", "rgblevels", "rgb levels"),
            preset("Fujifilm", "X-T10", "input profile rebuilt from the camera matrix", "colorin", "input color profile")]
    }
    // For a Fujifilm X-T10 RAW: three presets came with the image.
    function matched() {
        return unmatched().map(p => Object.assign({}, p,
            p.general || (p.maker === "Fujifilm" && p.operation !== "colorin") ? { matches: true, applied: true, available: true, reason: "" } : {},
            p.maker === "Fujifilm" ? { makerMatches: true } : {}))
    }
    // A film profile as the engine lists it (a preset for LUT 3D).
    function film(brand, stock, variant, title, more) {
        return Object.assign({ name: "Omalux DHH: " + title, title: title, description: "", film: true, group: "DHH",
                               brand: brand, stock: stock, variant: variant, id: "film-" + title.toLowerCase().replace(/[^a-z0-9]+/g, "-"),
                               operation: "lut3d", module: "LUT 3D", applied: false, available: true, reason: "" }, more || {})
    }
    function films() {
        return [film("Fuji", "Fuji Velvia 50", "C", "Fuji Velvia 50 - C"), film("Fuji", "Fuji Velvia 50", "L", "Fuji Velvia 50 - L"),
                film("Fuji", "Fuji Reala 100", "C", "Fuji Reala 100 C"),
                film("Kodak", "Kodak Portra 1600", "C", "Kodak Portra 1600 2C"), film("Kodak", "Kodak Portra 1600", "L", "Kodak Portra 1600 L"),
                film("Kodak", "Kodak Portra 400", "C", "Kodak Portra 400 2C"), film("Kodak", "Kodak Portra 400", "L", "Kodak Portra 400 L"),
                film("Kodak", "Portra 800⁺¹", "C", "Portra 800⁺¹ - C"), film("Kodak", "Portra 800⁺¹", "L", "Portra 800⁺¹ - L"),
                film("Kodak", "Ektar 25", "C", "Ektar 25 - C", { available: false, reason: "its lookup table file is missing" })]
    }
    function withFilm(title) { return top.unmatched().concat(top.films().map(f => f.title === title ? Object.assign({}, f, { applied: true }) : f)) }
    property var requests: []
    StylesPanel {
        id: panel
        anchors.fill: parent
        theme: th
        view: "camera"
        styles: []
        stylesReady: true
        photoReady: true
        busy: false
        appliedStyle: ""
        applyingStyle: ""
        errorMessage: ""
        camera: ""
        cameraDefaults: []
        allCameraPresets: top.unmatched()
        onCameraPresetRequested: (name, on) => top.requests.push([name, on])
    }

    TestCase {
        name: "camera"; when: windowShown
        function find(item, name) {
            if (!item) return null
            if (item.objectName === name && item.visible) return item
            for (let i = 0; i < item.children.length; ++i) { const f = find(item.children[i], name); if (f) return f }
            return null
        }
        function names(item, prefix, out) {
            out = out || []
            if (item.objectName && item.objectName.indexOf(prefix) === 0 && item.visible) out.push(item.objectName.slice(prefix.length))
            for (let i = 0; i < item.children.length; ++i) names(item.children[i], prefix, out)
            return out
        }
        // A click and the frame after it (positioners place new rows once per frame).
        function click(name) {
            const item = find(panel, name)
            verify(item, name)
            mouseClick(item)
            wait(40)
        }
        function init() {
            // The film list is shown only while there are films: close it before they go.
            const list = find(panel, "film-profile-list")
            if (list) { list.openGroup = ""; list.openBrand = ""; list.query = "" }
            panel.camera = ""
            panel.cameraDefaults = []
            panel.allCameraPresets = top.unmatched()
            panel.busy = false
            top.requests = []
            wait(20)
            find(panel, "camera-preset-list").openMaker = "Every camera"
            panel.view = "camera"
            panel.styles = []
            panel.appliedStyle = ""
            wait(40)
        }

        function test_01_nothing_matched() {
            const none = find(panel, "camera-presets-none")
            verify(none, "the pane says that nothing matched")
            verify(none.text.indexOf("automatically") >= 0 && none.text.indexOf("below") >= 0, none.text)
            // Makers as collapsible groups: the presets for every camera first, then by maker.
            compare(names(panel, "camera-maker-").filter(n => n.indexOf("count-") !== 0), ["Every camera", "Canon", "Fujifilm"])
            compare(find(panel, "camera-preset-list").openMaker, "Every camera", "the first group is open")
            compare(names(panel, "camera-preset-").filter(n => n.indexOf("state-") && n.indexOf("note-") && n.indexOf("list")).length, 2,
                    "only the open maker's presets are built")
            compare(find(panel, "camera-maker-count-Canon").text, "2")
        }

        function test_02_one_maker_open() {
            click("camera-maker-Fujifilm")
            compare(find(panel, "camera-preset-list").openMaker, "Fujifilm")
            verify(!find(panel, "camera-preset-Omalux Every camera : capture sharpening"), "the other maker closed")
            verify(find(panel, "camera-preset-Omalux Fujifilm X-T10: exposure"))
            click("camera-maker-Fujifilm")
            compare(find(panel, "camera-preset-list").openMaker, "", "a second click closes it")
        }

        function test_03_apply_and_take_off() {
            click("camera-maker-Fujifilm")
            const name = "Omalux Fujifilm X-T10: exposure"
            const row = find(panel, "camera-preset-" + name)
            compare(find(panel, "camera-preset-note-" + name).text, "rgb levels", "the darktable module, not a file name")
            click("camera-preset-" + name)
            compare(top.requests, [[name, true]], "one click applies")
            // The engine answers with the new state.
            panel.allCameraPresets = top.unmatched().map(p => p.name === name ? Object.assign({}, p, { applied: true }) : p)
            wait(20)
            compare(find(panel, "camera-preset-list").openMaker, "Fujifilm", "the open maker stays open")
            compare(find(panel, "camera-maker-count-Fujifilm").text, "1 of 3")
            const applied = find(panel, "camera-preset-" + name)
            verify(applied.applied)
            click("camera-preset-" + name)
            compare(top.requests[1], [name, false], "a second click takes it off")
        }

        function test_04_unavailable_says_why() {
            click("camera-maker-Fujifilm")
            const name = "Omalux Fujifilm : embedded lens correction"
            const row = find(panel, "camera-preset-" + name)
            verify(row && !row.enabled)
            compare(find(panel, "camera-preset-note-" + name).text, "this photo carries no embedded lens data")
            mouseClick(row); wait(40)
            compare(top.requests.length, 0, "a greyed preset does nothing")
            // Nor does any row while the engine is busy.
            panel.busy = true
            wait(20)
            click("camera-preset-Omalux Fujifilm X-T10: exposure")
            compare(top.requests.length, 0)
        }

        function test_05_matched_camera() {
            panel.camera = "Fujifilm X-T10"
            panel.cameraDefaults = [
                { group: "Camera presets", module: "lens", label: "lens correction", value: "embedded lens correction", enabled: true, present: true },
                { group: "Camera presets", module: "rgblevels", label: "rgb levels", value: "exposure", enabled: true, present: true }]
            panel.allCameraPresets = top.matched()
            wait(20)
            verify(!find(panel, "camera-presets-none"), "matched presets stay on top")
            verify(find(panel, "camera-matched-rgblevels").applied)
            // Another preset replaced the matched one on rgb levels: the module is still on, the
            // matched preset is not.
            panel.cameraDefaults = panel.cameraDefaults.map(e => e.module === "rgblevels" ? Object.assign({}, e, { enabled: false }) : e)
            wait(20)
            verify(!find(panel, "camera-matched-rgblevels").applied)
            verify(find(panel, "camera-matched-lens").applied)
            const list = find(panel, "camera-preset-list")
            compare(list.openMaker, "Fujifilm", "the photograph's maker is open")
            compare(names(panel, "camera-maker-").filter(n => n.indexOf("count-") !== 0)[0], "Fujifilm", "and comes first")
            compare(find(panel, "camera-maker-count-Fujifilm").text, "2 of 3")
            const name = "Omalux Fujifilm X-T10: exposure"
            compare(find(panel, "camera-preset-note-" + name).text, "rgb levels · matches this camera")
            click("camera-preset-" + name)
            compare(top.requests, [[name, false]], "an automatically applied preset can be taken off")
            // The unmatched input profile of the same camera is offered by hand.
            click("camera-preset-Omalux Fujifilm X-T10: input profile rebuilt from the camera matrix")
            compare(top.requests[1][1], true)
        }

        function test_06_keyboard_stops() {
            click("camera-maker-Fujifilm")
            const stops = Keys.collect(panel, false).filter(t => t.navId.indexOf("camera-") === 0)
            compare(stops.filter(t => t.kind === "group").length, 3, "each maker is a stop")
            const rows = stops.filter(t => t.navId.indexOf("camera-preset:") === 0)
            compare(rows.length, 3)
            compare(rows.filter(t => t.enabled).length, 2, "a greyed preset is skipped")
            const row = rows.find(t => t.navId === "camera-preset:Omalux Fujifilm X-T10: exposure")
            row.activate()
            compare(top.requests, [["Omalux Fujifilm X-T10: exposure", true]], "Enter applies")
            const group = stops.find(t => t.navId === "camera-maker:Canon")
            group.activate()
            compare(find(panel, "camera-preset-list").openMaker, "Canon")
        }
    
        function test_07_films_are_a_group_of_their_own() {
            panel.allCameraPresets = top.unmatched().concat(top.films())
            wait(40)
            compare(names(panel, "camera-maker-").filter(n => n.indexOf("count-") !== 0), ["Every camera", "Canon", "Fujifilm"],
                    "films are not listed among the camera makers")
            compare(find(panel, "camera-maker-count-Canon").text, "2")
            verify(find(panel, "camera-films-DHH"), "the group reads DHH")
            compare(find(panel, "camera-films-state-DHH").text, "6", "six film stocks")
            verify(!find(panel, "film-filter-DHH"), "closed until asked for")
            click("camera-films-DHH")
            verify(find(panel, "film-filter-DHH"))
            compare(names(panel, "film-brand-").filter(n => n.indexOf("count-") !== 0), ["DHH|Fuji", "DHH|Kodak"])
            compare(find(panel, "film-brand-count-DHH|Kodak").text, "4")
            compare(names(panel, "film-variant-").length, 0, "no rows until a brand is open")
            click("film-brand-DHH|Kodak")
            // One row per film stock, in natural order, without the brand's name; variants as chips.
            compare(names(panel, "film-").filter(n => !/^(variant|brand|filter|note|none|profile)/.test(n)),
                    ["Ektar 25", "Kodak Portra 400", "Portra 800⁺¹", "Kodak Portra 1600"])
            compare(names(panel, "film-variant-").length, 7)
            click("film-brand-DHH|Fuji")
            compare(names(panel, "film-variant-"), ["Omalux DHH: Fuji Reala 100 C", "Omalux DHH: Fuji Velvia 50 - C", "Omalux DHH: Fuji Velvia 50 - L"],
                    "one brand is open at a time; a film may have one variant")
        }

        function test_08_one_film_at_a_time() {
            panel.allCameraPresets = top.unmatched().concat(top.films())
            wait(40)
            click("camera-films-DHH")
            click("film-brand-DHH|Kodak")
            click("film-variant-Omalux DHH: Kodak Portra 400 2C")
            compare(top.requests, [["Omalux DHH: Kodak Portra 400 2C", true]], "a click on a variant puts the film on")
            panel.allCameraPresets = top.withFilm("Kodak Portra 400 2C")
            wait(40)
            verify(find(panel, "film-variant-Omalux DHH: Kodak Portra 400 2C").applied)
            compare(find(panel, "camera-films-state-DHH").text, "Kodak Portra 400 2C", "the heading names the film that is on")
            click("film-variant-Omalux DHH: Kodak Portra 400 L")
            compare(top.requests[1], ["Omalux DHH: Kodak Portra 400 L", true], "the other variant replaces it")
            click("film-variant-Omalux DHH: Kodak Portra 400 2C")
            compare(top.requests[2], ["Omalux DHH: Kodak Portra 400 2C", false], "a click on the one that is on takes it off")
            // A film whose lookup table is missing says so and does nothing.
            compare(find(panel, "film-note-Ektar 25").text, "its lookup table file is missing")
            const chip = find(panel, "film-variant-Omalux DHH: Ektar 25 - C")
            verify(!chip.enabled)
            mouseClick(chip); wait(40)
            compare(top.requests.length, 3)
            panel.busy = true
            wait(20)
            click("film-variant-Omalux DHH: Kodak Portra 400 L")
            compare(top.requests.length, 3, "nothing while the engine is busy")
        }

        function test_09_film_filter() {
            panel.allCameraPresets = top.unmatched().concat(top.films())
            wait(40)
            click("camera-films-DHH")
            const list = find(panel, "film-profile-list")
            list.query = "portra"
            wait(40)
            compare(names(panel, "film-brand-").filter(n => n.indexOf("count-") !== 0), ["DHH|Kodak"], "only brands with a match")
            compare(find(panel, "film-brand-count-DHH|Kodak").text, "3 of 4")
            compare(names(panel, "film-variant-").length, 6, "every match is shown without opening the brand")
            list.query = "velvia l"
            wait(40)
            compare(names(panel, "film-variant-").length, 2, "a film matches by its variants' names too")
            list.query = "nothing like this"
            wait(40)
            verify(find(panel, "film-none-DHH"))
            list.query = ""
        }

        function test_10_film_keyboard_stops() {
            panel.allCameraPresets = top.withFilm("Kodak Portra 400 2C")
            wait(40)
            click("camera-films-DHH")
            click("film-brand-DHH|Kodak")
            const stops = Keys.collect(panel, false)
            verify(stops.find(t => t.navId === "camera-films:DHH" && t.kind === "group"))
            verify(stops.find(t => t.navId === "film-filter:DHH" && t.kind === "search"))
            compare(stops.filter(t => t.navId.indexOf("film-brand:") === 0).length, 2)
            const rows = stops.filter(t => t.navId.indexOf("film:DHH:") === 0)
            compare(rows.length, 4, "one stop per film stock")
            compare(rows.filter(t => t.enabled).length, 3, "a film that cannot run is skipped")
            top.requests = []
            const on = rows.find(t => t.navId === "film:DHH:Kodak Portra 400")
            verify(on.active)
            on.adjust(1)
            compare(top.requests[0], ["Omalux DHH: Kodak Portra 400 L", true], "→ steps to the next variant")
            on.activate()
            compare(top.requests[1], ["Omalux DHH: Kodak Portra 400 2C", false], "Enter takes the film off")
            rows.find(t => t.navId === "film:DHH:Portra 800⁺¹").activate()
            compare(top.requests[2], ["Omalux DHH: Portra 800⁺¹ - C", true], "Enter puts the first variant on")
        }

        function test_11_a_look_offers_its_film() {
            panel.styles = [{ id: "dhh/look-a/style.dtstyle", name: "Look A", description: "", previewUrl: "", error: "", modules: [], film: "film-kodak-portra-400-2c" },
                            { id: "dhh/look-b/style.dtstyle", name: "Look B", description: "", previewUrl: "", error: "", modules: [], film: "" }]
            panel.allCameraPresets = top.unmatched().concat(top.films())
            panel.view = "looks"
            wait(40)
            verify(!find(panel, "style-film-offer"), "nothing is offered before the look is applied")
            panel.appliedStyle = "Look A"
            wait(40)
            const offer = find(panel, "style-film-offer")
            verify(offer, "the applied look names a film")
            compare(panel.lookFilm.title, "Kodak Portra 400 2C")
            compare(top.requests.length, 0, "the film is never put on by itself")
            mouseClick(offer); wait(40)
            compare(top.requests, [["Omalux DHH: Kodak Portra 400 2C", true]])
            panel.allCameraPresets = top.withFilm("Fuji Velvia 50 - L")
            wait(40)
            verify(!find(panel, "style-film-offer"), "no offer while a film is on")
            panel.allCameraPresets = top.unmatched().concat(top.films())
            panel.appliedStyle = "Look B"
            wait(40)
            verify(!find(panel, "style-film-offer"), "a look without a film offers none")
        }
    }
}
