import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import org.omalux

Item {
    id: panel

    required property var theme
    required property bool photoReady
    required property string catalogJson
    required property string selectedPresetId
    signal presetRequested(string id)

    property var expandedGroups: ({ monochrome: true })
    readonly property var catalogPresets: {
        try {
            return JSON.parse(catalogJson).presets || []
        } catch (error) {
            return []
        }
    }
    readonly property var defaultPreset: catalogPresets.find(
        preset => (preset.group || "basic") === "basic")
    readonly property var groups: {
        let result = []
        for (let preset of catalogPresets) {
            let key = preset.group || "basic"
            if (key === "basic") continue
            let group = result.find(group => group.id === key)
            if (!group) {
                group = { id: key, name: key.replace(/\//g, " · ").replace(/-/g, " "), presets: [] }
                result.push(group)
            }
            group.presets.push(preset)
        }
        result.sort((a, b) => {
            if (a.id === "monochrome") return -1
            if (b.id === "monochrome") return 1
            if (a.id === "experimental") return 1
            if (b.id === "experimental") return -1
            return a.id.localeCompare(b.id)
        })
        return result
    }
    readonly property var rows: {
        let result = []
        if (defaultPreset)
            result.push(Object.assign({ isGroup: false, isDefault: true }, defaultPreset))
        for (let group of groups) {
            result.push({ id: group.id, name: group.name, isGroup: true })
            if (expandedGroups[group.id]) {
                for (let preset of group.presets)
                    result.push(Object.assign({ isGroup: false }, preset))
            }
        }
        return result
    }

    function toggleGroup(id) {
        let expanded = {}
        if (!expandedGroups[id])
            expanded[id] = true
        expandedGroups = expanded
    }

    ColumnLayout {
        anchors.fill: parent
        spacing: 10

        Text {
            text: "PRESETS"
            color: panel.theme.inkColor
            font.family: panel.theme.monoFont
            font.pixelSize: 13
            font.bold: true
            font.letterSpacing: 1
        }

        ListView {
            id: presetList
            objectName: "presetList"
            Layout.fillWidth: true
            Layout.fillHeight: true
            clip: true
            spacing: 2
            model: panel.rows
            boundsBehavior: Flickable.StopAtBounds
            ScrollBar.vertical: ScrollBar {}

            SidebarScrollHandler { flickable: presetList }

            delegate: Button {
                id: entry
                required property var modelData
                width: presetList.width
                height: modelData.isGroup ? 36 : 72
                padding: 0
                enabled: modelData.isGroup || panel.photoReady
                activeFocusOnTab: false
                onClicked: {
                    if (modelData.isGroup) panel.toggleGroup(modelData.id)
                    else panel.presetRequested(modelData.id)
                }
                Accessible.name: modelData.name
                Accessible.description: modelData.isGroup
                    ? (panel.expandedGroups[modelData.id] ? "Collapse group" : "Expand group") : ""

                contentItem: RowLayout {
                    spacing: 12

                    Text {
                        visible: entry.modelData.isGroup
                        Layout.preferredWidth: 14
                        text: panel.expandedGroups[entry.modelData.id] ? "▾" : "▸"
                        color: panel.theme.mutedColor
                        font.pixelSize: 13
                    }

                    Image {
                        visible: !entry.modelData.isGroup
                        objectName: "presetPreview-" + entry.modelData.id
                        Layout.preferredWidth: 96
                        Layout.preferredHeight: 64
                        source: entry.modelData.previewUrl || ""
                        fillMode: Image.PreserveAspectFit
                        asynchronous: true
                        smooth: true
                    }

                    Text {
                        Layout.fillWidth: true
                        text: entry.modelData.isGroup ? entry.modelData.name.toUpperCase() : entry.modelData.name
                        color: !entry.modelData.isGroup && panel.selectedPresetId === entry.modelData.id
                            ? panel.theme.accentColor : panel.theme.inkColor
                        font.family: panel.theme.monoFont
                        font.pixelSize: 11
                        font.bold: entry.modelData.isGroup || panel.selectedPresetId === entry.modelData.id
                        opacity: entry.hovered ? 1 : 0.85
                        wrapMode: Text.Wrap
                        maximumLineCount: 2
                        elide: Text.ElideRight
                    }
                }

                background: null
            }
        }
    }
}
