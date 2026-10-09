import QtQuick

// Makes the item it is declared in reachable from the keyboard. Put one inside a control and
// connect the hooks it needs; KeyboardNavigator finds it by walking the visible sidebar, so no
// registration and no reference to the navigator is required:
//
//     NavTarget {
//         id: nav
//         navId: root.control.id; label: root.control.label; kind: "slider"; group: "exposure"
//         onAdjust: steps => root.edited(root.value + steps * root.control.step)
//         onReset: root.resetRequested()
//     }
//     // and on mouse press: nav.claim(), so the clicked control becomes the keyboard selection
//
// Order follows the displayed position, top to bottom. Hidden items (collapsed modules,
// inactive panes) are skipped; a direct shortcut asks a hidden one to reveal itself first.
QtObject {
    id: target
    readonly property bool isNavTarget: true

    // Identity, stable across re-creation of delegates; unique within a pane.
    property string navId: ""
    // darktable's label of the thing, shown in the key hints.
    property string label: ""
    // slider · choice · switch · module · group · style · step · button · search · graph
    property string kind: "button"
    // Items of one module (or one style group) share a group: PageUp/PageDown, Shift+R and E
    // work on it.
    property string group: ""
    // false: skipped while moving, keys do nothing (e.g. a slider without a photograph).
    property bool enabled: true
    // The component considers this its current item (e.g. the active control or the current
    // history step); used as the selection when nothing was selected yet.
    property bool active: false
    // For kind "search": the text field that Enter focuses and Escape leaves.
    property var input: null
    // false: not among the ↑/↓ stops (the global search, reached with / or Ctrl+F).
    property bool listed: true
    // For kind "graph": the item that takes the keys on Enter (its own point editing) until
    // Escape gives them back to the navigator.
    property Item focusItem: null

    // What the bottom bar offers for this item: [[keys, action], …]. Override for new kinds.
    property string adjustLabel: kind === "slider" || kind === "choice" ? label
                               : kind === "switch" ? "TOGGLE"
                               : kind === "module" || kind === "group" || kind === "style" ? "COLLAPSE/EXPAND" : ""
    property string activateLabel: kind === "module" || kind === "group" ? "EXPAND"
                                 : kind === "style" ? "APPLY STYLE"
                                 : kind === "step" ? "RESTORE STEP"
                                 : kind === "switch" ? "TOGGLE"
                                 : kind === "search" ? "SEARCH"
                                 : kind === "graph" ? "EDIT POINTS"
                                 : kind === "button" ? label.toUpperCase() : ""
    property bool resettable: kind === "slider" || kind === "choice" || kind === "switch" || kind === "graph"
    property bool groupActions: false        // module: E switches it, Shift+R resets it, I its instances
    // Further [keys, action] pairs of this item (a style tile: E marks a favourite).
    property var extraHints: []
    readonly property var hints: {
        const h = []
        if (adjustLabel !== "") h.push(["←/→", adjustLabel])
        if (activateLabel !== "") h.push(["⏎", activateLabel])
        if (resettable) h.push(["R", "RESET VALUE"])
        if (groupActions) h.push(["E", "ON/OFF"], ["⇧R", "RESET MODULE"], ["I", "INSTANCES"])
        for (const x of extraHints) h.push(x)
        return h
    }

    // Set by the navigator: this is the keyboard selection. Bind the highlight to it.
    property bool current: false
    property var navigator: null
    property Item owner: null
    property real claimedAt: 0

    signal adjust(real steps)       // ←/→: ±1 step, Shift ±10, Ctrl/Alt ±0.1
    signal activate()               // Enter / Space
    signal reset()                  // R
    signal resetGroup()             // Shift+R
    signal toggleGroup()            // E
    signal selected()               // became the keyboard selection
    signal revealRequested()        // a direct shortcut targets this hidden item

    // Call on mouse interaction: the item becomes the keyboard selection, focus stays with
    // the navigator.
    function claim() {
        claimedAt = Date.now()
        if (navigator) navigator.claimed(target)
    }
}
