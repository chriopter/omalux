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
            panel.camera = ""
            panel.cameraDefaults = []
            panel.allCameraPresets = top.unmatched()
            panel.busy = false
            top.requests = []
            wait(20)
            find(panel, "camera-preset-list").openMaker = "Every camera"
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
    }
}
