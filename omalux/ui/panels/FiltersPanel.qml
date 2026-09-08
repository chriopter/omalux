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
                    visible: group.modelData.name === "Advanced"
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
                            expanded: !!root.expandedDetails[modelData.key]
                            onExpansionRequested: root.setExpanded("expandedDetails", modelData.key, !root.expandedDetails[modelData.key])
                            onControlSelected: id => root.controlSelected(id)
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
