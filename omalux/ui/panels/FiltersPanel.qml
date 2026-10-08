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
    // Sidebar search: only matching modules stay, and they open.
    property string term: ""
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
        { module: "colisa", key: "brightness-shortcut", name: "contrast brightness saturation", primary: ["brightness"], shortcut: true },
        { module: "colisa", name: "contrast brightness saturation", primary: ["contrast"] },
        { module: "colisa", key: "saturation-shortcut", name: "contrast brightness saturation", primary: ["saturation"], shortcut: true },
        { module: "shadhi", name: "shadows and highlights", primary: ["shadows"] },
        { module: "temperature", name: "white balance", primary: ["temperature"] },
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
    function reveal(id) {
        if (sections.some(s => s.primary.includes(id))) {
            revealTimer.controlId = id; revealTimer.restart(); return
        }
        for (let g of groups) for (let s of g.sections) {
            if (!s.controls.some(c => c.id === id)) continue
            if (!s.primary.includes(id)) setExpanded("expandedDetails", s.key, true)
        }
        revealTimer.controlId = id; revealTimer.restart()
    }
    function toggleGrainDetails() {
        setExpanded("expandedDetails", "grain", !expandedDetails["grain"])
    }
    function navigate(direction) {
        let visible = []
        for (let g of groups) for (let s of g.sections)
            visible = visible.concat(s.controls.filter(c => expandedDetails[s.key] || s.primary.includes(c.id)))
        visible = visible.filter((c, i, all) => all.findIndex(other => other.id === c.id) === i)
        let index = visible.findIndex(c => c.id === activeControl)
        if (visible.length) {
            if (index < 0) index = direction > 0 ? -1 : 0
            const c = visible[(index + direction + visible.length) % visible.length]
            controlSelected(c.id); reveal(c.id)
        }
    }
    Timer {
        id: revealTimer
        property string controlId
        interval: 50
        onTriggered: {
            for (let i = 0; i < groupRows.count; ++i) {
                const item = groupRows.itemAt(i).findControl(controlId)
                if (!item) continue
                const flick = root.contentItem
                const point = item.mapToItem(flick.contentItem, 0, 0)
                let next = flick.contentY
                if (point.y < next) next = point.y
                else if (point.y + item.height > next + flick.height) next = point.y + item.height - flick.height
                flick.contentY = Math.max(0, Math.min(next, Math.max(0, flick.contentHeight - flick.height)))
            }
        }
    }
    Column {
        width: root.availableWidth
        padding: 18; spacing: 8
        Text {
            visible: root.term !== "" && root.matchCount === 0
            width: parent.width - 36
            text: "No curated control matches “" + root.term + "”; the other panes are searched too."
            color: root.theme.muted; font: root.theme.textFont; wrapMode: Text.WordWrap
        }
        Repeater {
            id: groupRows
            model: root.groups
            delegate: Column {
                id: group
                required property var modelData
                width: parent.width - 36
                spacing: 0
                topPadding: modelData.name === "Single" && root.sections.some(s => s.primary.length > 1) ? 20 : 0
                function findControl(id) {
                    for (let i = 0; i < modules.count; ++i) {
                        const item = modules.itemAt(i).findControl(id)
                        if (item) return item
                    }
                    return null
                }
                Text {
                    visible: group.modelData.name === "Advanced" && (root.term === "" || group.modelData.sections.some(s => root.sectionMatches(s)))
                    text: "Advanced"
                    color: root.theme.muted; font: root.theme.settingsFont
                    topPadding: 12; bottomPadding: 4
                }
                Column {
                    width: parent.width; spacing: 8
                    Repeater {
                        id: modules
                        model: group.modelData.sections
                        delegate: FilterModule {
                            required property var modelData
                            width: parent.width
                            theme: root.theme; section: modelData; values: root.values
                            editable: root.editable; activeControl: root.activeControl
                            // A shortcut row opens its module's block and stands in for it
                            // only while that block is closed.
                            readonly property string detailsKey: modelData.shortcut ? modelData.module : modelData.key
                            visible: root.term !== "" ? !modelData.shortcut && root.sectionMatches(modelData)
                                                      : !(modelData.shortcut && root.expandedDetails[detailsKey])
                            expanded: !modelData.shortcut && (root.term !== "" || !!root.expandedDetails[detailsKey])
                            extraModule: root.catalogModel ? root.catalogModel.modulesByOperation[modelData.module] || null : null
                            moduleState: root.states[modelData.module + "/0"]
                            catalogModel: root.catalogModel
                            overrides: root.overrides
                            term: root.term
                            moreOpen: !!root.expandedDetails[detailsKey + "-more"]
                            onMoreRequested: root.setExpanded("expandedDetails", detailsKey + "-more", !root.expandedDetails[detailsKey + "-more"])
                            onParameterChangesRequested: changes => root.parameterChangesRequested(modelData.module, 0, changes)
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
