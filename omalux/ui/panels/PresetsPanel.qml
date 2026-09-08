import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

import "../components"

SidebarScrollView {
    id: root
    required property var theme
    required property var presets
    required property bool presetsReady
    required property bool photoReady
    required property bool busy
    required property string appliedStyle
    required property string applyingPreset
    required property string errorMessage
    readonly property bool textEditing: presetSearch.activeFocus
    signal saveRequested()
    signal exportRequested(string id)
    signal deleteRequested(string id, string name)
    signal applyRequested(string id)
    property string presetQuery: ""
    property string expandedPresetGroup: "monochrome"
    property var expandedPresetDetails: ({})

    function presetGroup(id) {
        let parts = id.split("/")
        return parts.length > 2 ? parts.slice(0, -2).join("/") : ""
    }
    function groupName(id) { return id.replace(/\//g, " · ").replace(/-/g, " ") }
    function groupOpen(id) { return id === "" || presetQuery.trim() !== "" || expandedPresetGroup === id }
    function togglePresetDetails(id) {
        let next = Object.assign({}, expandedPresetDetails)
        next[id] = !next[id]
        expandedPresetDetails = next
    }
    function showDetails(id) {
        expandedPresetGroup = presetGroup(id)
        let next = Object.assign({}, expandedPresetDetails)
        next[id] = true
        expandedPresetDetails = next
    }
    readonly property var visiblePresets: root.presets.filter(function(p) {
        return (p.name + " " + p.description + " " + root.groupName(root.presetGroup(p.id))).toLowerCase().indexOf(presetQuery.trim().toLowerCase()) !== -1
    })
    readonly property var presetGroups: {
        let result = []
        for (let preset of visiblePresets) {
            let key = presetGroup(preset.id)
            let group = result.find(g => g.id === key)
            if (!group) {
                group = { id: key, name: groupName(key), presets: [] }
                result.push(group)
            }
            group.presets.push(preset)
        }
        function rank(id) {
            if (id === "") return 0
            if (id === "my-presets") return 1
            if (id === "monochrome") return 2
            if (id === "experimental") return 4
            return 3
        }
        result.sort((a, b) => rank(a.id) - rank(b.id) || a.id.localeCompare(b.id))
        for (let group of result)
            group.presets.sort((a, b) => a.id === "neutral/preset.dtstyle" ? -1 : b.id === "neutral/preset.dtstyle" ? 1 : a.name.localeCompare(b.name))
        return result
    }
    Column {
        width: root.availableWidth; padding: 10
        Column {
            width: parent.width - 20; spacing: 10
            RowLayout {
                width: parent.width
                Text { text: "PRESETS"; color: root.theme.ink; font.bold: true; font.letterSpacing: 2; Layout.fillWidth: true }
                Text { text: root.presets.length; color: root.theme.muted; font: root.theme.textFont }
            }
            Button { text: "Save current look…"; enabled: root.photoReady && !root.busy; onClicked: root.saveRequested() }
            TextField {
                id: presetSearch
                width: parent.width; height: 32
                placeholderText: "Search presets"; color: root.theme.ink
                placeholderTextColor: root.theme.muted; font: root.theme.textFont
                onTextChanged: root.presetQuery = text
                background: Rectangle { color: "#181825"; border.color: parent.activeFocus ? root.theme.accent : root.theme.line; radius: 3 }
                Accessible.name: "Search presets"
            }
            Text {
                width: parent.width; visible: !root.presetsReady || root.visiblePresets.length === 0
                text: !root.presetsReady ? "Loading presets…" : root.presets.length === 0 ? "No presets in presets/." : "No matching presets."
                color: root.theme.muted; font: root.theme.textFont; wrapMode: Text.WordWrap
            }
            Repeater {
                model: root.presetGroups
                delegate: Column {
                    id: groupSection
                    required property var modelData
                    width: parent.width; spacing: 2
                    Button {
                        objectName: "preset-group-" + groupSection.modelData.id
                        width: parent.width; height: 36; padding: 0
                        visible: groupSection.modelData.id !== ""
                        onClicked: root.expandedPresetGroup = root.expandedPresetGroup === groupSection.modelData.id ? "" : groupSection.modelData.id
                        Accessible.name: groupSection.modelData.name
                        Accessible.description: root.groupOpen(groupSection.modelData.id) ? "Collapse group" : "Expand group"
                        background: Rectangle { color: parent.hovered ? "#313244" : "transparent"; border.color: parent.activeFocus ? root.theme.accent : "transparent"; radius: 3 }
                        contentItem: RowLayout {
                            spacing: 12
                            Text { Layout.preferredWidth: 14; text: root.groupOpen(groupSection.modelData.id) ? "▾" : "▸"; color: root.theme.muted; font: root.theme.textFont }
                            Text { Layout.fillWidth: true; text: groupSection.modelData.name.toUpperCase(); color: root.theme.ink; font.family: root.theme.textFont.family; font.pixelSize: 11; font.bold: true }
                            Text { text: groupSection.modelData.presets.length; color: root.theme.muted; font: root.theme.textFont }
                        }
                    }
                    Repeater {
                        model: root.groupOpen(groupSection.modelData.id) ? groupSection.modelData.presets : []
                        delegate: PresetCard {
                            required property var modelData
                            theme: root.theme
                            preset: modelData
                            photoReady: root.photoReady
                            busy: root.busy
                            appliedStyle: root.appliedStyle
                            applyingPreset: root.applyingPreset
                            expanded: !!root.expandedPresetDetails[modelData.id]
                            onExportRequested: root.exportRequested(modelData.id)
                            onDeleteRequested: root.deleteRequested(modelData.id, modelData.name)
                            onApplyRequested: root.applyRequested(modelData.id)
                            onDetailsToggleRequested: root.togglePresetDetails(modelData.id)
                        }
                    }
                }
            }
            Text { width: parent.width; visible: root.errorMessage !== ""; text: root.errorMessage; color: "#f9d58b"; font: root.theme.textFont; wrapMode: Text.WordWrap }
        }
    }
}
