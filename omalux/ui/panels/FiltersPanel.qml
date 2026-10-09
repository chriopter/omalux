import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import QtCore
import "../components"

SidebarScrollView {
    id: root
    objectName: "filtersScroll"
    required property var theme
    required property var controls
    required property var values
    required property bool editable
    required property string activeControl
    property bool restoringPreferences: true
    property var expandedDetails: ({})
    // Generated layout and catalog state, for the parameters a curated module has beyond the
    // registry (shown under "more" in its expanded block).
    property var catalogModel: null
    property var states: ({})
    property var overrides: ({})
    property var controlDefault: null
    // Sidebar search: only matching modules stay, and they open.
    property string term: ""
    // Blocks open for the search only while this pane is on screen; matchCount (the pane's dot
    // and the search jump) always follows the term.
    readonly property string shownTerm: visible ? term : ""
    signal parameterChangesRequested(string operation, int instance, var changes)
    signal interactionChanged(bool active)
    signal halationRequested()
    signal controlSelected(string id)
    signal controlEdited(string id, real value)
    signal controlReset(string id)

    Settings {
        id: preferences
        category: "FiltersPanel"
        property string details: '{}'
    }
    Component.onCompleted: {
        try { expandedDetails = JSON.parse(preferences.details) } catch (e) {}
        restoringPreferences = false
    }
    onExpandedDetailsChanged: if (!restoringPreferences) preferences.details = JSON.stringify(expandedDetails)

    // Parameters and enablement remain native; colisa also exposes two shortcut rows.
    readonly property var sections: [
        { module: "exposure", name: "exposure", primary: ["exposure"] },
        // The last of a module's rows owns its block, so the block unfolds beneath all of them.
        { module: "colisa", key: "brightness-shortcut", name: "contrast brightness saturation", primary: ["brightness"], shortcut: true },
        { module: "colisa", key: "contrast-shortcut", name: "contrast brightness saturation", primary: ["contrast"], shortcut: true },
        { module: "colisa", name: "contrast brightness saturation", primary: ["saturation"] },
        { module: "shadhi", name: "shadows and highlights", primary: ["shadows"] },
        // temperature is darktable's conversion of the channel coefficients: they stay in view.
        { module: "temperature", name: "white balance", primary: ["temperature"], driven: ["red", "green", "blue", "various"] },
        { module: "colorbalancergb", name: "color balance rgb", primary: ["vibrance"] },
        { module: "sharpen", name: "sharpen", primary: ["sharpen_amount"] },
        { module: "grain", name: "grain", primary: ["grain"], shortTitle: true },
        { module: "bloom", name: "bloom", primary: ["bloom_strength"], shortTitle: true },
        { module: "vignette", name: "vignetting", primary: ["vignette"], shortTitle: true },
        { module: "bilat", name: "local contrast", primary: [] },
        { module: "denoiseprofile", name: "denoise (profiled)", primary: [] },
        { module: "diffuse", name: "diffuse or sharpen", primary: [] },
        { module: "lut3d", name: "LUT 3D", primary: [] }
    ].map(s => Object.assign({}, s, {
        key: s.key || s.module,
        controls: root.controls.filter(c => c.module === s.module && (!s.shortcut || s.primary.includes(c.id)) && !["System", "Curve", "Geometry"].includes(c.group))
    })).filter(s => s.controls.length)
    readonly property var groups: [
        { name: "Grouped", sections: sections.filter(s => s.primary.length > 1) },
        { name: "Single", sections: sections.filter(s => s.primary.length === 1) },
        { name: "Advanced", sections: sections.filter(s => !s.primary.length) }
    ]

    function sectionMatches(s) {
        if (!term) return true
        if (s.name.toLowerCase().indexOf(term) >= 0 || s.module.toLowerCase().indexOf(term) >= 0) return true
        if (s.controls.some(c => c.label.toLowerCase().indexOf(term) >= 0 || (c.id === "vibrance" && "vibrance".indexOf(term) >= 0))) return true
        const extra = catalogModel ? catalogModel.modulesByOperation[s.module] : null
        return !!extra && extra.rows.some(r => (r.label || "").toLowerCase().indexOf(term) >= 0)
    }
    readonly property int matchCount: {
        let n = 0
        for (const s of sections) if (!s.shortcut && sectionMatches(s)) ++n
        return n
    }
    function setExpanded(property, key, value) {
        let next = Object.assign({}, root[property]); next[key] = value; root[property] = next
    }
    // Opens the module of a secondary parameter; KeyboardNavigator selects and scrolls to it.
    function reveal(id) {
        for (let s of sections)
            if (s.controls.some(c => c.id === id) && !s.primary.includes(id))
                setExpanded("expandedDetails", s.key, true)
    }
    function toggleGrainDetails() {
        setExpanded("expandedDetails", "grain", !expandedDetails["grain"])
    }
    Column {
        width: root.availableWidth
        padding: 18; spacing: 8
        Text {
            visible: root.shownTerm !== "" && root.matchCount === 0
            width: parent.width - 36
            text: "No curated control matches “" + root.shownTerm + "”; the other panes are searched too."
            color: root.theme.muted; font: root.theme.textFont; wrapMode: Text.WordWrap
        }
        Repeater {
            model: root.groups
            delegate: Column {
                id: group
                required property var modelData
                width: parent.width - 36
                spacing: 0
                topPadding: modelData.name === "Single" && root.sections.some(s => s.primary.length > 1) ? 20 : 0
                Text {
                    visible: group.modelData.name === "Advanced" && (root.shownTerm === "" || group.modelData.sections.some(s => root.sectionMatches(s)))
                    text: "Advanced"
                    color: root.theme.muted; font: root.theme.settingsFont
                    topPadding: 12; bottomPadding: 4
                }
                Column {
                    width: parent.width; spacing: 8
                    Repeater {
                        model: group.modelData.sections
                        delegate: FilterModule {
                            required property var modelData
                            width: parent.width
                            theme: root.theme; section: modelData; values: root.values
                            editable: root.editable; activeControl: root.activeControl
                            // A shortcut row opens its module's block (under the module's own
                            // row) and stays where it is, like every main row; its chevron
                            // turns with the block.
                            readonly property string detailsKey: modelData.shortcut ? modelData.module : modelData.key
                            visible: root.shownTerm === "" || (!modelData.shortcut && root.sectionMatches(modelData))
                            detailsOpen: root.shownTerm !== "" || !!root.expandedDetails[detailsKey]
                            // The frame of an open block reaches up around the module's other
                            // rows (each a row and the gap above this one).
                            blockLift: modelData.shortcut || root.shownTerm !== "" ? 0
                                       : root.sections.filter(s => s.shortcut && s.module === modelData.module).length * 56
                            z: blockLift > 0 ? -1 : 0
                            mainIds: root.sections.filter(s => s.module === modelData.module).reduce((all, s) => all.concat(s.primary), [])
                            expanded: !modelData.shortcut && (root.shownTerm !== "" || !!root.expandedDetails[detailsKey])
                            extraModule: root.catalogModel ? root.catalogModel.modulesByOperation[modelData.module] || null : null
                            moduleState: root.states[modelData.module + "/0"]
                            catalogModel: root.catalogModel
                            overrides: root.overrides
                            controlDefault: root.controlDefault
                            term: root.shownTerm
                            moreOpen: !!root.expandedDetails[detailsKey + "-more"]
                            onMoreRequested: root.setExpanded("expandedDetails", detailsKey + "-more", !root.expandedDetails[detailsKey + "-more"])
                            onParameterChangesRequested: changes => root.parameterChangesRequested(modelData.module, 0, changes)
                            onInstanceChangesRequested: (instance, changes) => root.parameterChangesRequested(modelData.module, instance, changes)
                            onExpansionRequested: root.setExpanded("expandedDetails", detailsKey, !root.expandedDetails[detailsKey])
                            onControlSelected: id => root.controlSelected(id)
                            onInteractionChanged: active => root.interactionChanged(active)
                            onControlEdited: (id, value) => root.controlEdited(id, value)
                            onControlReset: id => root.controlReset(id)
                            onHalationRequested: root.halationRequested()
                        }
                    }
                }
            }
        }
    }
}
