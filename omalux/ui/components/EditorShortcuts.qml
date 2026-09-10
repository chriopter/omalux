import QtQuick

Item {
    id: root
    property bool active: true
    property bool filtersActive: true
    signal panelRequested(int index)
    signal panelStepRequested(int direction)
    signal controlStepRequested(int direction)
    signal valueStepRequested(int steps)
    signal resetRequested()
    signal controlRequested(string id)
    signal grainDetailsRequested()
    signal zoomRequested(real factor)
    signal fitRequested()
    signal fullscreenRequested()
    signal openRequested()
    signal saveRequested()
    signal helpRequested()
    readonly property var bindings: [
        { keys: ["1"], label: "Filters", run: () => panelRequested(0) },
        { keys: ["2"], label: "Styles", run: () => panelRequested(1) },
        { keys: ["3"], label: "Crop & Rotate", run: () => panelRequested(2) },
        { keys: ["4"], label: "History", run: () => panelRequested(3) },
        { keys: ["5"], label: "Metadata", run: () => panelRequested(4) },
        { keys: ["Tab", "]"], label: "Next panel", run: () => panelStepRequested(1) },
        { keys: ["Shift+Tab", "["], label: "Previous panel", run: () => panelStepRequested(-1) },
        { keys: ["Up", "K"], label: "Previous parameter", filters: true, run: () => controlStepRequested(-1) },
        { keys: ["Down", "J"], label: "Next parameter", filters: true, run: () => controlStepRequested(1) },
        { keys: ["Left", "H"], label: "Decrease value", filters: true, run: () => valueStepRequested(-1) },
        { keys: ["Right", "L"], label: "Increase value", filters: true, run: () => valueStepRequested(1) },
        { keys: ["Shift+Left", "Shift+H"], label: "Decrease fast", filters: true, run: () => valueStepRequested(-10) },
        { keys: ["Shift+Right", "Shift+L"], label: "Increase fast", filters: true, run: () => valueStepRequested(10) },
        { keys: ["R"], label: "Reset selected parameter", filters: true, run: () => resetRequested() },
        { keys: ["A"], label: "Grain details", filters: true, run: () => grainDetailsRequested() },
        { keys: ["G"], label: "Grain strength", run: () => controlRequested("grain") },
        { keys: ["S"], label: "Grain coarseness", run: () => controlRequested("grain_size") },
        { keys: ["M"], label: "Grain mid-tones bias", run: () => controlRequested("grain_midtones") },
        { keys: ["+", "=", "Ctrl++", "Ctrl+="], label: "Zoom in", run: () => zoomRequested(1.25) },
        { keys: ["-", "Ctrl+-"], label: "Zoom out", run: () => zoomRequested(.8) },
        { keys: ["0", "Ctrl+0"], label: "Fit photograph", run: () => fitRequested() },
        { keys: ["F"], label: "Photo fullscreen", run: () => fullscreenRequested() },
        { keys: ["O", "Ctrl+O"], label: "Open photograph", run: () => openRequested() },
        { keys: ["Ctrl+S"], label: "Save / export", run: () => saveRequested() },
        { keys: ["?", "F1"], label: "Keyboard reference", run: () => helpRequested() }
    ]
    Instantiator {
        model: root.bindings
        delegate: Shortcut {
            required property var modelData
            sequences: modelData.keys
            enabled: root.active && (!modelData.filters || root.filtersActive)
            onActivated: modelData.run()
        }
    }
}
