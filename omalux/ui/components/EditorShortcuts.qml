import QtQuick
import "keyboard.js" as Keyboard

// Every key of the editor in one table: KeyboardNavigator hands each key it receives to
// handle(), the keyboard reference lists the same rows. The first matching row that applies
// wins, so context rows (crop, fullscreen) come first. Rows without `run` only document
// behaviour that lives elsewhere (text fields, dialogs, mouse).
QtObject {
    id: root
    property var navigator: null       // KeyboardNavigator
    property bool cropping: false
    property bool fullscreen: false
    // A drawing tool on the photo is active (shape, line, warp being added).
    property bool canvasDrawing: false
    // A module colour picker is running on the photo.
    property bool picking: false
    // The view shows the photograph as opened (before/after).
    property bool comparing: false
    property int panelCount: 5
    signal compareRequested()
    signal panelRequested(int index)
    signal panelStepRequested(int direction)
    signal controlRequested(string id)
    signal grainDetailsRequested()
    signal zoomRequested(real factor)
    signal fitRequested()
    signal fullscreenRequested()
    signal fullscreenExitRequested()
    signal cropApplyRequested()
    signal cropCancelRequested()
    signal canvasCancelRequested()
    signal pickerCancelRequested()
    signal openRequested()
    signal saveRequested()
    signal helpRequested()

    readonly property var sections: ["Sidebar", "Selected item", "Panes", "Direct", "Photograph", "Crop", "File", "Text fields and dialogs"]
    readonly property var bindings: [
        // Context first: while cropping, Enter and Escape belong to the crop frame.
        { section: "Crop", scope: "crop", keys: ["Return", "Enter"], label: "Apply crop", run: () => cropApplyRequested() },
        { section: "Crop", scope: "crop", keys: ["Escape"], label: "Cancel crop (restores the previous crop)", run: () => cropCancelRequested() },
        { section: "Photograph", scope: "fullscreen", keys: ["F", "Escape"], label: "Leave photograph fullscreen", run: () => fullscreenExitRequested() },
        { section: "Photograph", scope: "canvas", keys: ["Escape"], label: "Leave the drawing tool on the photo (shape, line or warp being added)", run: () => canvasCancelRequested() },
        { section: "Photograph", scope: "picker", keys: ["Escape"], label: "End the running colour picker", run: () => pickerCancelRequested() },
        { section: "Photograph", scope: "compare", keys: ["Escape"], label: "Back from “before” to the edit", run: () => compareRequested() },

        { section: "Sidebar", keys: ["Up", "K"], label: "Previous item (crosses modules; collapsed parameters are skipped)", run: () => navigator.move(-1) },
        { section: "Sidebar", keys: ["Down", "J"], label: "Next item", run: () => navigator.move(1) },
        { section: "Sidebar", keys: ["PgUp"], label: "Previous module / group", run: () => navigator.moveGroup(-1) },
        { section: "Sidebar", keys: ["PgDown"], label: "Next module / group", run: () => navigator.moveGroup(1) },
        { section: "Sidebar", keys: ["Home"], label: "First item", run: () => navigator.moveEdge(false) },
        { section: "Sidebar", keys: ["End"], label: "Last item", run: () => navigator.moveEdge(true) },
        { section: "Sidebar", keys: ["/", "Ctrl+F"], label: "Search modules and controls (the field under the tabs)", run: () => navigator.focusSearch() },

        { section: "Selected item", keys: ["Left", "H"], label: "Decrease · previous option · collapse", run: () => navigator.adjust(-1) },
        { section: "Selected item", keys: ["Right", "L"], label: "Increase · next option · expand", run: () => navigator.adjust(1) },
        { section: "Selected item", keys: ["Shift+Left", "Shift+H"], label: "Decrease by 10 steps", run: () => navigator.adjust(-10) },
        { section: "Selected item", keys: ["Shift+Right", "Shift+L"], label: "Increase by 10 steps", run: () => navigator.adjust(10) },
        { section: "Selected item", keys: ["Ctrl+Left", "Alt+Left"], label: "Decrease by a tenth step", run: () => navigator.adjust(-0.1) },
        { section: "Selected item", keys: ["Ctrl+Right", "Alt+Right"], label: "Increase by a tenth step", run: () => navigator.adjust(0.1) },
        { section: "Selected item", keys: ["Return", "Enter", "Space"], label: "Activate: expand module, apply style, restore history step, press button, toggle", run: () => navigator.activate() },
        { section: "Selected item", keys: ["R"], label: "Reset selected parameter to darktable's default", run: () => navigator.reset() },
        { section: "Selected item", keys: ["Shift+R"], label: "Reset every parameter of the selected module", run: () => navigator.resetGroup() },
        { section: "Selected item", keys: ["E"], label: "Switch the selected module on/off", run: () => navigator.toggleGroup() },

        { section: "Panes", keys: ["1", "2", "3", "4", "5", "6", "7", "8", "9"], label: "Pane in reading order: Edit panes, Styles, History, Info", run: i => { if (i < panelCount) panelRequested(i) } },
        { section: "Panes", keys: ["Tab", "]"], label: "Next pane", run: () => panelStepRequested(1) },
        { section: "Panes", keys: ["Shift+Tab", "["], label: "Previous pane", run: () => panelStepRequested(-1) },

        { section: "Direct", keys: ["G"], label: "Grain strength", run: () => controlRequested("grain") },
        { section: "Direct", keys: ["S"], label: "Grain coarseness (opens grain details)", run: () => controlRequested("grain_size") },
        { section: "Direct", keys: ["M"], label: "Grain mid-tones bias (opens grain details)", run: () => controlRequested("grain_midtones") },
        { section: "Direct", keys: ["A"], label: "Grain details open/closed", run: () => grainDetailsRequested() },

        { section: "Photograph", scope: "view", keys: ["+", "=", "Ctrl++", "Ctrl+="], label: "Zoom in", run: () => zoomRequested(1.25) },
        { section: "Photograph", scope: "view", keys: ["-", "Ctrl+-"], label: "Zoom out", run: () => zoomRequested(.8) },
        { section: "Photograph", scope: "view", keys: ["0", "Ctrl+0"], label: "Fit photograph", run: () => fitRequested() },
        { section: "Photograph", scope: "view", keys: ["B"], label: "Before / after: show the photograph as it was opened; any edit returns to the result", run: () => compareRequested() },
        { section: "Photograph", keys: ["F"], label: "Photograph fullscreen", run: () => fullscreenRequested() },

        { section: "File", keys: ["O", "Ctrl+O"], label: "Open photograph", run: () => openRequested() },
        { section: "File", keys: ["Ctrl+S"], label: "Export photograph", run: () => saveRequested() },
        { section: "File", scope: "view", keys: ["?", "F1"], label: "This keyboard reference", run: () => helpRequested() },

        { section: "Text fields and dialogs", keys: ["Escape"], label: "Leave a text field (search, style name); keys type text until then. The module search is cleared as well" },
        { section: "Text fields and dialogs", keys: ["Return", "Down"], label: "Leave a search field and select the item below it" },
        { section: "Text fields and dialogs", keys: ["Escape"], label: "Close a dialog, menu or value entry; keys return to the sidebar" },
        { section: "Text fields and dialogs", keys: ["Enter on a curve", "Escape"], label: "Edit the curve's or graph's points with its own arrow keys; Escape gives the keys back" },
        { section: "Text fields and dialogs", keys: ["Double-click value"], label: "Type a value within darktable's full range" },
        { section: "Text fields and dialogs", keys: ["Right-click"], label: "Reset menu of a parameter" }
    ]

    function available(binding) {
        switch (binding.scope) {
        case "crop": return cropping
        case "fullscreen": return fullscreen
        case "canvas": return canvasDrawing
        case "picker": return picking
        case "compare": return comparing
        case "view": return true
        default: return !fullscreen
        }
    }
    function handle(event) {
        for (const binding of bindings) {
            if (!binding.run || !available(binding)) continue
            const index = binding.keys.findIndex(key => Keyboard.matches(event, key))
            if (index < 0) continue
            binding.run(index)
            return true
        }
        return false
    }
    // Keys as shown in the reference and the hint line.
    function display(key) {
        return key.replace("PgUp", "Page Up").replace("PgDown", "Page Down").replace("Return", "Enter")
                  .replace(/Left$/, "←").replace(/Right$/, "→").replace(/^Up$/, "↑").replace(/^Down$/, "↓")
    }
    function displayKeys(binding) {
        return [...new Set(binding.keys.map(display))].join(" / ")
    }
}
