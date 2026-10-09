import QtQuick
import QtQuick.Controls
import QtTest
import "../../ui"
import "../../ui/components"
import "editor-fixture.js" as Fixture

// Drives the keyboard through every sidebar pane, wired like Main.qml, against a recorded
// control registry and a backend stand-in:
//   XDG_CONFIG_HOME=$(mktemp -d) QT_QPA_PLATFORM=offscreen \
//     /usr/lib/qt6/bin/qmltestrunner -input omalux/tests/components/tst_keyboard.qml
Item {
    id: top
    width: 1000; height: 760

    EditorTheme { id: theme }
    property string activeControl: "exposure"
    property var log: []
    function logged(kind) { return top.log.filter(entry => entry[0] === kind) }

    QtObject {
        id: backend
        readonly property bool developerMode: true
        readonly property var controls: Fixture.controls
        property var controlValues: ({})
        readonly property var styles: [
            { id: "neutral/style.dtstyle", name: "neutral", description: "", previewUrl: "", error: "", modules: [] },
            { id: "film/one/style.dtstyle", name: "film one", description: "first", previewUrl: "", error: "", modules: [] },
            { id: "film/two/style.dtstyle", name: "film two", description: "second", previewUrl: "", error: "", modules: [] },
            { id: "monochrome/grey/style.dtstyle", name: "grey", description: "", previewUrl: "", error: "", modules: [] }
        ]
        readonly property bool stylesReady: true
        readonly property string preview: "image://preview/1"
        property bool styleBusy: false
        property string activeStyle: ""
        readonly property string applyingStyle: ""
        readonly property string styleError: ""
        property string moduleCatalog: JSON.stringify(Fixture.modules)
        readonly property string displayData: "{}"
        readonly property var metadata: ({ width: 1500, height: 1000, camera: "test" })
        readonly property string layoutData: JSON.stringify(Fixture.layout)
        signal moduleUpdated(string operation, int instance, string moduleJson)
        function setParameters(operation, instance, values) {
            for (const path in values) top.log.push(["parameter", operation, path, values[path]])
        }
        function resetModule(operation, instance) { top.log.push(["resetModule", operation, instance]) }
        property var history: [0, 1, 2, 3].map(step => ({ step: step, label: "step " + step, enabled: true,
                                                         active: true, current: step === 3, operation: "exposure" }))
        readonly property var cameraDefaults: []
        function initial(id) { return Fixture.controls.find(c => c.id === id).initial }
        function setControl(id, value) {
            let next = Object.assign({}, controlValues); next[id] = value; controlValues = next
            top.log.push(["set", id, value])
        }
        function setControls(values) { for (const id in values) setControl(id, values[id]) }
        function resetControl(id) { top.log.push(["reset", id]); setControl(id, initial(id)) }
        function hoverStyle(id, active) { top.log.push(["hover", id, active]) }
        function applyStyle(id) { top.log.push(["apply", id]) }
        function selectHistory(step) { top.log.push(["history", step]) }
        function setInteractive(active) {}
        function applyHalation() { top.log.push(["halation"]) }
        function setParameter(operation, instance, field, value) { top.log.push(["parameter", operation, field, value]) }
        Component.onCompleted: {
            let values = {}
            for (const c of Fixture.controls) values[c.id] = c.initial
            controlValues = values
        }
    }

    // Wiring as in Main.qml.
    KeyboardNavigator {
        id: keyboard
        scope: sidebar
        pane: sidebar.selectedPanel + "/" + sidebar.filterView
        shortcuts: shortcuts
    }
    Item {
        anchors.fill: parent
        Keys.onPressed: event => keyboard.handleKey(event)
        EditorSidebar {
            id: sidebar
            x: 600; width: 352; height: parent.height
            theme: theme
            backend: backend
            activeControl: top.activeControl
            iconsRoot: Qt.resolvedUrl("../../../assets/icons/")
            onControlSelected: id => top.activeControl = id
        }
    }
    EditorShortcuts {
        id: shortcuts
        navigator: keyboard
        cropping: sidebar.geometry.cropping
        panelCount: sidebar.keyOrder.length
        onPanelRequested: position => sidebar.selectedPanel = sidebar.keyOrder[position]
        onPanelStepRequested: direction => { const panels = sidebar.paneOrder; sidebar.selectedPanel = panels[(panels.indexOf(sidebar.selectedPanel) + direction + panels.length) % panels.length] }
        onControlRequested: id => { sidebar.revealControl(id); keyboard.selectId(id) }
        onGrainDetailsRequested: { sidebar.selectedPanel = 0; sidebar.toggleGrainDetails() }
        onCropApplyRequested: sidebar.geometry.apply()
        onCropCancelRequested: sidebar.geometry.cancel()
        onZoomRequested: factor => top.log.push(["zoom", factor])
        onFitRequested: top.log.push(["fit"])
        onOpenRequested: top.log.push(["open"])
        onSaveRequested: top.log.push(["save"])
        onHelpRequested: { top.log.push(["help"]); help.open() }
        onFullscreenRequested: top.log.push(["fullscreen"])
    }

    KeyboardHelp {
        id: help
        theme: theme
        shortcuts: shortcuts
        width: 600; height: 600
    }

    TestCase {
        id: test
        name: "keyboard"
        when: windowShown

        function find(item, name) {
            if (!item) return null
            if (item.objectName === name) return item
            const children = item.children
            for (let i = 0; i < children.length; ++i) {
                const found = find(children[i], name)
                if (found) return found
            }
            return null
        }
        function filters() { return find(sidebar, "filtersScroll") }
        function selected() { return keyboard.selection ? keyboard.selection.navId : "" }
        function value(id) { return backend.controlValues[id] }
        // One frame between keys, as when typing: layouts settle like on screen.
        function press(key, modifiers) { keyClick(key, modifiers || Qt.NoModifier); wait(16) }
        function order() {
            // The navigable items of the visible pane, as the navigator sees them.
            // (without the global search field, which leads every pane)
            return keyboard.targets().map(t => t.navId).filter(id => id !== "search")
        }

        function init() {
            top.log = []
            sidebar.selectedPanel = 0
            sidebar.filterView = 0
            filters().expandedDetails = ({})
            for (const c of Fixture.controls) backend.setControl(c.id, c.initial)
            backend.setControl("exposure_enabled", 1)
            top.log = []
            top.activeControl = "exposure"
            keyboard.select(null)
            keyboard._remembered = ({})
            find(sidebar, "styleSearch").text = ""
            keyboard.forceActiveFocus()
            wait(20)
        }

        function initTestCase() {
            Qt.application.organization = "omalux-tests"
            Qt.application.domain = "omalux.test"
        }

        function test_01_focus_owner() {
            verify(keyboard.activeFocus, "navigator holds focus")
        }

        function test_02_adjust_and_reset() {
            const step = Fixture.controls.find(c => c.id === "exposure").step
            press(Qt.Key_Right)
            compare(selected(), "exposure", "the active control is the first selection")
            fuzzyCompare(value("exposure"), step, 1e-6)
            press(Qt.Key_Right, Qt.ShiftModifier)
            fuzzyCompare(value("exposure"), 11 * step, 1e-6)
            press(Qt.Key_Left, Qt.ControlModifier)
            fuzzyCompare(value("exposure"), 10.9 * step, 1e-6)
            press(Qt.Key_L)
            fuzzyCompare(value("exposure"), 11.9 * step, 1e-6)
            press(Qt.Key_R)
            compare(value("exposure"), 0)
            verify(keyboard.hintText.indexOf("[←/→] exposure") >= 0, keyboard.hintText)
            verify(keyboard.hintText.indexOf("[R] RESET VALUE") >= 0)
        }

        function test_03_order_crosses_modules() {
            const list = order()
            compare(list.slice(0, 4), ["exposure", "brightness", "contrast", "saturation"])
            verify(list.indexOf("grain_size") < 0, "collapsed parameters are not stops")
            compare(list.slice(-4), ["module:bilat", "module:denoiseprofile", "module:diffuse", "module:lut3d"])
            press(Qt.Key_Down)
            compare(selected(), "brightness")
            compare(top.activeControl, "brightness", "selection drives the active control")
            press(Qt.Key_J)
            compare(selected(), "contrast")
            press(Qt.Key_K)
            compare(selected(), "brightness")
            press(Qt.Key_End)
            compare(selected(), "module:lut3d")
            press(Qt.Key_Down)
            compare(selected(), "module:lut3d", "no wrap at the end")
            press(Qt.Key_Home)
            compare(selected(), "exposure")
            press(Qt.Key_Up)
            compare(selected(), "exposure", "no wrap at the start")
            press(Qt.Key_PageDown)
            compare(selected(), "brightness", "next module")
            press(Qt.Key_PageDown)
            press(Qt.Key_PageUp)
            compare(selected(), "brightness")
        }

        function test_04_scrolls_into_view() {
            const flick = filters().contentItem
            flick.contentY = 0
            press(Qt.Key_End)
            const owner = keyboard.selection.owner
            const p = owner.mapToItem(flick, 0, 0)
            verify(p.y >= 0 && p.y + owner.height <= flick.height, "last item visible, y " + p.y)
            verify(flick.contentY > 0)
            press(Qt.Key_Home)
            compare(flick.contentY, 0)
        }

        function test_05_modules_expand_and_reveal() {
            // Advanced modules are reachable through their heading; Enter opens them.
            press(Qt.Key_End)
            press(Qt.Key_PageUp); press(Qt.Key_PageUp); press(Qt.Key_PageUp)
            compare(selected(), "module:bilat")
            press(Qt.Key_Return)
            verify(filters().expandedDetails["bilat"])
            wait(20)                  // layout of the opened module
            press(Qt.Key_Down)
            compare(selected(), "detail", "the opened module's parameter is next")
            press(Qt.Key_Up)
            press(Qt.Key_Left)
            verify(!filters().expandedDetails["bilat"], "← collapses a module heading")
            // Direct shortcut to a hidden parameter opens its module and selects it.
            press(Qt.Key_S)
            tryVerify(() => selected() === "grain_size")
            verify(filters().expandedDetails["grain"])
            compare(top.activeControl, "grain_size")
            // Enter on a row of an open module closes it; the selection falls back into it.
            press(Qt.Key_Space)
            verify(!filters().expandedDetails["grain"])
            tryVerify(() => selected() === "grain")
        }

        function test_06_module_reset_and_toggle() {
            backend.setControl("shadows", 30)
            backend.setControl("highlights", -20)
            filters().expandedDetails = ({ shadhi: true })
            wait(20)
            keyboard.selectId("highlights")
            tryVerify(() => selected() === "highlights")
            press(Qt.Key_R, Qt.ShiftModifier)
            compare(value("shadows"), backend.initial("shadows"))
            compare(value("highlights"), backend.initial("highlights"))
            press(Qt.Key_Home)
            press(Qt.Key_E)
            compare(value("exposure_enabled"), 0)
            press(Qt.Key_E)
            compare(value("exposure_enabled"), 1)
        }

        function test_07_hidden_panes_get_no_keys() {
            sidebar.filterView = 1          // the curated pane is now hidden
            wait(20)
            verify(order().indexOf("exposure") < 0)
            press(Qt.Key_Right)
            press(Qt.Key_R)
            compare(top.logged("set").length, 0, "nothing in the hidden pane changed")
            sidebar.selectedPanel = 4       // Info: nothing to select, keys scroll
            wait(20)
            compare(order().length, 0)
            press(Qt.Key_Down); press(Qt.Key_Right); press(Qt.Key_Return)
            compare(top.log.length, 0)
            verify(keyboard.hintText.indexOf("SCROLL") >= 0)
        }

        function test_08_panes() {
            press(Qt.Key_7)
            compare(sidebar.selectedPanel, 1)
            press(Qt.Key_Tab)
            compare(sidebar.selectedPanel, 9, "Tab follows the strip: the look modules after the looks")
            press(Qt.Key_Tab)
            compare(sidebar.selectedPanel, 10, "then the camera pane")
            verify(find(sidebar, "camera-presets").visible)
            press(Qt.Key_Tab)
            compare(sidebar.selectedPanel, 3, "then History")
            press(Qt.Key_8)
            compare(sidebar.selectedPanel, 3, "8 stays History")
            press(Qt.Key_9)
            compare(sidebar.selectedPanel, 4, "9 stays Info")
            find(sidebar, "sidebar-area-styles").clicked()
            compare(sidebar.selectedPanel, 10, "the Styles area returns to its last pane")
            press(Qt.Key_7)
            compare(sidebar.selectedPanel, 1, "7: Looks")
            press(Qt.Key_2)
            compare(sidebar.selectedPanel, 5, "numbers follow the strip as well")
            press(Qt.Key_6)
            compare(sidebar.selectedPanel, 2, "6: Crop & Rotate")
            press(Qt.Key_9)
            compare(sidebar.selectedPanel, 4, "9: Info")
            press(Qt.Key_BracketLeft)
            compare(sidebar.selectedPanel, 3)
            press(Qt.Key_Question, Qt.ShiftModifier)
            compare(top.logged("help").length, 1)
            press(Qt.Key_Escape)
            tryVerify(() => keyboard.activeFocus)
            press(Qt.Key_S, Qt.ControlModifier)
            compare(top.logged("save").length, 1)
            press(Qt.Key_0)
            compare(top.logged("fit").length, 1)
            press(Qt.Key_Plus, Qt.ShiftModifier)
            compare(top.logged("zoom").length, 1)
        }

        function test_09_text_fields_swallow_keys() {
            press(Qt.Key_7)
            keyboard.selectId("style-search")
            press(Qt.Key_Return)
            const field = keyboard.Window.window.activeFocusItem
            verify(isText(field), "search has focus")
            for (const key of ["1", "r", "?", "0", "j"]) keyClick(key)
            compare(field.text, "1r?0j", "characters went to the field")
            compare(sidebar.selectedPanel, 1, "no pane switch while typing")
            compare(JSON.stringify(top.log.filter(e => e[0] !== "hover")), "[]", "no shortcut fired")
            press(Qt.Key_Up); press(Qt.Key_Left)
            compare(sidebar.selectedPanel, 1)
            press(Qt.Key_Escape)
            verify(keyboard.activeFocus, "Escape returns focus")
            compare(selected(), "style-search")
            field.text = "film"
            press(Qt.Key_Return)      // Enter on the selected search field focuses it
            verify(field.activeFocus)
            press(Qt.Key_Return)
            verify(keyboard.activeFocus, "Enter returns focus")
            compare(selected(), "group:film", "and selects the first result")
            field.text = ""
        }

        function test_10_styles() {
            press(Qt.Key_7)
            const panel = find(sidebar, "stylesPanel")
            panel.favourites = []
            panel.expandedStyleGroup = "film"
            wait(20)
            press(Qt.Key_Home)
            compare(selected(), "style-search", "the filter field, then Save beside it")
            press(Qt.Key_Down)
            compare(selected(), "save")
            verify(keyboard.hintText.indexOf("SAVE CURRENT LOOK") >= 0, keyboard.hintText)
            press(Qt.Key_Down)
            compare(selected(), "neutral/style.dtstyle", "the basic looks lead the grid; their heading is no stop")
            tryVerify(() => top.logged("hover").some(e => e[1] === "neutral/style.dtstyle" && e[2]), 1000,
                      "keyboard selection previews like hover")
            verify(keyboard.hintText.indexOf("APPLY STYLE") >= 0)
            verify(keyboard.hintText.indexOf("FAVOURITE") >= 0, keyboard.hintText)
            press(Qt.Key_Right)
            tryVerify(() => !!find(sidebar, "style-details-neutral/style.dtstyle"), 1000, "→ shows the settings")
            press(Qt.Key_Left)
            tryVerify(() => !find(sidebar, "style-details-neutral/style.dtstyle"), 1000, "← hides them")
            press(Qt.Key_Down)
            compare(selected(), "group:monochrome")
            tryVerify(() => top.logged("hover").some(e => !e[2]), 1000, "preview ends when the selection leaves")
            press(Qt.Key_Down)
            compare(selected(), "group:film", "a closed family has no stops")
            press(Qt.Key_Down)
            compare(selected(), "film/one/style.dtstyle", "the open family's looks follow")
            press(Qt.Key_Return)
            compare(top.logged("apply").length, 1)
            compare(top.logged("apply")[0][1], "film/one/style.dtstyle")
            // E marks a favourite: it joins the top of the pane and stays in its family.
            press(Qt.Key_E)
            compare(JSON.stringify(panel.favourites), JSON.stringify(["film/one/style.dtstyle"]))
            tryVerify(() => order().indexOf("favourite:film/one/style.dtstyle") >= 0, 1000, JSON.stringify(order()))
            verify(order().indexOf("favourite:film/one/style.dtstyle") < order().indexOf("neutral/style.dtstyle"), "favourites come first")
            compare(selected(), "film/one/style.dtstyle", "the selection stays in the family")
            press(Qt.Key_E)
            compare(panel.favourites.length, 0)
            press(Qt.Key_PageUp)
            compare(selected(), "group:film", "PageUp goes to the start of the family")
            press(Qt.Key_Return)      // close film, open monochrome by its heading
            press(Qt.Key_Up)
            compare(selected(), "group:monochrome")
            press(Qt.Key_Return)
            press(Qt.Key_Down)
            compare(selected(), "monochrome/grey/style.dtstyle", "one family open at a time")
            verify(order().indexOf("film/one/style.dtstyle") < 0)
            panel.expandedStyleGroup = "film"
        }

        function test_11_history() {
            press(Qt.Key_8)
            press(Qt.Key_Right)       // nothing to adjust; resolves the selection
            compare(selected(), "step-3", "starts at the current step")
            press(Qt.Key_Home)
            compare(selected(), "step-0")
            press(Qt.Key_Down)
            compare(selected(), "step-1")
            press(Qt.Key_End)
            press(Qt.Key_Return)
            compare(top.logged("history").length, 1)
            verify(keyboard.hintText.indexOf("RESTORE STEP") >= 0)
        }

        function test_12_geometry_and_crop() {
            press(Qt.Key_6)
            compare(order().slice(0, 4), ["rotation", "aspect", "edit-crop", "reset-crop"])
            press(Qt.Key_Down)
            compare(selected(), "rotation")
            press(Qt.Key_Right)
            verify(value("rotation") > 0)
            press(Qt.Key_Down)
            press(Qt.Key_Right, Qt.ShiftModifier)
            compare(sidebar.geometry.aspectRatio, 1500 / 1000, "→ chooses the next ratio (Original)")
            press(Qt.Key_Down)
            press(Qt.Key_Return)
            verify(sidebar.geometry.cropping, "Enter on Edit crop starts cropping")
            verify(!keyboard.targets().find(t => t.navId === "rotation").enabled, "rotation is locked while cropping")
            press(Qt.Key_Escape)
            verify(!sidebar.geometry.cropping, "Escape cancels")
            press(Qt.Key_Return)
            verify(sidebar.geometry.cropping)
            top.log = []
            press(Qt.Key_Return)
            verify(!sidebar.geometry.cropping, "Enter applies")
            verify(top.logged("set").some(e => e[0] === "set" && e[1] === "crop_enabled" && e[2] === 1))
        }

        function test_13_mouse_keeps_keyboard() {
            const slider = find(sidebar, "control-slider-contrast")
            mouseClick(slider, slider.width / 2, slider.height / 2)
            tryVerify(() => keyboard.activeFocus, 500, "focus returns after a click")
            const before = value("contrast")
            press(Qt.Key_Right)
            compare(selected(), "contrast", "the clicked control is the selection")
            verify(value("contrast") > before)
            // Numeric entry: typing goes to the field, Enter returns to the navigator.
            const row = find(sidebar, "filter-control-contrast")
            mouseDoubleClickSequence(row, row.width - 50, 8)
            const field = keyboard.Window.window.activeFocusItem
            verify(isText(field), "value entry has focus")
            field.selectAll()
            for (const key of ["0", ".", "5"]) keyClick(key)
            compare(sidebar.selectedPanel, 0)
            press(Qt.Key_Return)
            compare(value("contrast"), 0.5)
            tryVerify(() => keyboard.activeFocus, 500, "focus returns after value entry")
        }

        function test_14_wheel_then_keys() {
            const flick = filters().contentItem
            press(Qt.Key_Home)
            flick.contentY = flick.contentHeight - flick.height     // scrolled away by wheel
            press(Qt.Key_Right)
            compare(selected(), "exposure")
            verify(flick.contentY < 20, "adjusting brings the selection back into view")
            for (let i = 0; i < 5; ++i) press(Qt.Key_Right)          // repeated keys accumulate
            fuzzyCompare(value("exposure"), 6 * Fixture.controls[0].step, 1e-6)
        }

        function test_15_module_list() {
            sidebar.filterView = 1
            wait(20)
            const list = order()
            compare(list[0], "module-filter")
            compare(list.slice(1), ["module:cacorrectrgb/0", "module:rgbcurve/0", "module:sigmoid/0", "module:vignette/0"],
                    "collapsed modules show only their heading")
            press(Qt.Key_Down); press(Qt.Key_Down)
            compare(selected(), "module:cacorrectrgb/0")
            press(Qt.Key_Right)
            wait(20)
            compare(order().length, list.length + 5, "→ opens the module")
            press(Qt.Key_Down)
            compare(selected(), "cacorrectrgb/0/guide_channel")
            press(Qt.Key_Right)
            compare(JSON.stringify(top.logged("parameter")[0]), JSON.stringify(["parameter", "cacorrectrgb", "guide_channel", Fixture.modules[0].parameters[0].value + 1]))
            press(Qt.Key_PageDown)
            compare(selected(), "module:rgbcurve/0", "Page Down: next module")
            press(Qt.Key_Up)
            compare(selected(), "cacorrectrgb/0/refine_manifolds")
            press(Qt.Key_Space)
            compare(top.logged("parameter").length, 2, "Space toggles a switch")
            press(Qt.Key_R)
            compare(top.logged("parameter")[2][3], Fixture.modules[0].parameters[4]["default"], "R restores darktable's default")
            press(Qt.Key_E)
            compare(top.logged("parameter")[3][2], "@enabled", "E switches the module")
        }

        function test_16_dialog_returns_focus() {
            press(Qt.Key_F1)
            tryVerify(() => help.opened)
            verify(!keyboard.activeFocus, "the dialog owns the keys")
            press(Qt.Key_7)
            compare(sidebar.selectedPanel, 0, "keys do not leak past a dialog")
            press(Qt.Key_Escape)
            tryVerify(() => !help.visible)
            tryVerify(() => keyboard.activeFocus, 500, "focus returns after the dialog closes")
            press(Qt.Key_7)
            compare(sidebar.selectedPanel, 1)
        }

        function test_17_combo_box_click() {
            press(Qt.Key_6)
            const combo = find(sidebar, "aspectChoice")
            mouseClick(combo, combo.width / 2, combo.height / 2)
            tryVerify(() => combo.popup.opened, 500, "the list opens on click")
            wait(50)
            verify(combo.popup.visible, "and stays open")
            press(Qt.Key_Escape)
            tryVerify(() => !combo.popup.visible, 500, "Escape closes it")
            tryVerify(() => keyboard.activeFocus, 500, "keys return to the navigator")
            // While open, the list has the keys.
            mouseClick(combo, combo.width / 2, combo.height / 2)
            tryVerify(() => combo.popup.opened, 500)
            const before = combo.currentIndex
            press(Qt.Key_Down)
            press(Qt.Key_Return)
            tryVerify(() => !combo.popup.visible, 500)
            compare(combo.currentIndex, before + 1)
            compare(sidebar.selectedPanel, 2, "Down and Enter did not reach the sidebar")
            tryVerify(() => keyboard.activeFocus, 500)
            press(Qt.Key_1)
            compare(sidebar.selectedPanel, 0)
        }

        function test_18_generated_tab() {
            press(Qt.Key_2)
            compare(sidebar.selectedPanel, 5, "3: Tone")
            compare(order().slice(0, 2), ["sigmoid/0/middle_grey_contrast", "module:rgbcurve/0"],
                    "the summary row, then the Advanced cards without rows; sigmoid is not repeated")
            press(Qt.Key_Down)
            compare(selected(), "sigmoid/0/middle_grey_contrast")
            verify(keyboard.hintText.indexOf("[←/→] contrast") >= 0, keyboard.hintText)
            const contrast = Fixture.modules.find(m => m.operation === "sigmoid").parameters[0]
            const edits = () => top.logged("parameter").filter(e => e[1] === "sigmoid")
            press(Qt.Key_Right)
            tryVerify(() => edits().length === 1)
            compare(edits()[0][2], "middle_grey_contrast")
            fuzzyCompare(edits()[0][3], contrast.value + 0.001, 1e-6, "one step of darktable's 3 digits, raw units")
            press(Qt.Key_R)
            tryVerify(() => edits().length === 2, 1500)
            fuzzyCompare(edits()[1][3], 1.5, 1e-6, "R: the row's default")
            press(Qt.Key_E)
            tryVerify(() => edits().some(e => e[2] === "@enabled" && e[3] === 1), 1500, "E switches the module through its heading")
            press(Qt.Key_R, Qt.ShiftModifier)
            compare(JSON.stringify(top.logged("resetModule")), JSON.stringify([["resetModule", "sigmoid", 0]]), "Shift+R resets the module")
            // Enter on the summary row opens the whole module in its place.
            press(Qt.Key_Return)
            tryVerify(() => order()[0] === "module:sigmoid/0", 1500, "the module heading replaces the row: " + JSON.stringify(order()))
            keyboard.selectId("sigmoid/0/middle_grey_contrast"); wait(20)
            press(Qt.Key_Down)
            compare(selected(), "sigmoid/0/contrast_skewness", "the detail rows follow the primary row")
            press(Qt.Key_Left)
            tryVerify(() => edits().some(e => e[2] === "contrast_skewness"), 1500)
            // "more" is a stop too; it opens the advanced rows.
            press(Qt.Key_Down)
            compare(selected(), "sigmoid/0/@more")
            press(Qt.Key_Return)
            // A choice row: ← / → choose options.
            const choice = order().find(id => id === "sigmoid/0/color_processing")
            verify(choice, "choice rows are stops: " + JSON.stringify(order()))
            keyboard.selectId(choice); wait(20)
            press(Qt.Key_Right)
            tryVerify(() => edits().some(e => e[2] === "color_processing" && e[3] === 1), 1500)
            // A curve takes the keys on Enter and gives them back on Escape.
            press(Qt.Key_PageDown)
            compare(selected(), "module:rgbcurve/0")
            press(Qt.Key_Return)
            const graph = keyboard.targets().find(t => t.kind === "graph")
            verify(graph, "the curve is a stop")
            keyboard.selectId(graph.navId); wait(20)
            verify(keyboard.hintText.indexOf("EDIT POINTS") >= 0, keyboard.hintText)
            console.log("GRAPH enabled", graph.enabled)
            if (graph.enabled) {
                press(Qt.Key_Return)
                verify(graph.owner.activeFocus, "Enter lends the keys to the curve")
                press(Qt.Key_Escape)
                tryVerify(() => keyboard.activeFocus, 500, "Escape gives them back")
                compare(selected(), graph.navId)
            }
            // The global search: / focuses it, typing stays in it, Escape clears and leaves.
            press(Qt.Key_Slash)
            const search = find(sidebar, "module-search")
            verify(search.activeFocus)
            for (const key of ["s", "k", "e", "w"]) keyClick(key)
            compare(search.text, "skew")
            compare(sidebar.selectedPanel, 5)
            press(Qt.Key_Return)
            verify(keyboard.activeFocus, "Enter leaves the search and keeps it")
            compare(search.text, "skew")
            press(Qt.Key_Slash)
            press(Qt.Key_Escape)
            compare(search.text, "", "Escape clears the search")
            tryVerify(() => keyboard.activeFocus, 500, "and leaves it")
        }

        // A person holds the button down for a moment: the event loop runs between press and
        // release. The navigator must not take focus back in between, or the click is lost.
        function slowClick(item) {
            mousePress(item, item.width / 2, item.height / 2)
            wait(80)
            mouseRelease(item, item.width / 2, item.height / 2)
        }
        function test_30_slow_clicks() {
            slowClick(find(sidebar, "sidebar-tab-5"))
            compare(sidebar.selectedPanel, 5, "one click on Tone")
            slowClick(find(sidebar, "sidebar-area-styles"))
            compare(sidebar.selectedPanel, 1, "one click on Styles")
            slowClick(find(sidebar, "sidebar-area-edit"))
            compare(sidebar.selectedPanel, 5, "Edit returns to its last pane")
            slowClick(find(sidebar, "sidebar-area-details"))
            compare(sidebar.selectedPanel, 3)
            slowClick(find(sidebar, "sidebar-area-styles"))
            slowClick(find(sidebar, "sidebar-area-details"))
            compare(sidebar.selectedPanel, 3, "Details returns to its last pane")
            slowClick(find(sidebar, "sidebar-area-edit"))
            compare(sidebar.selectedPanel, 5)
            tryVerify(() => keyboard.activeFocus, 500, "focus returns after the release")
            press(Qt.Key_Tab)
            compare(sidebar.selectedPanel, 6, "keys work after clicks (Tab: Tone → Color)")
        }

        function isText(item) { return !!item && typeof item.selectAll === "function" && item.cursorPosition !== undefined }
    }
}
