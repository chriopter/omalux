import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

import "../components"

SidebarScrollView {
    id: root
    required property var theme
    required property var styles
    required property bool stylesReady
    required property bool photoReady
    required property bool busy
    required property string appliedStyle
    required property string applyingStyle
    required property string errorMessage
    readonly property bool textEditing: styleSearch.activeFocus
    signal previewRequested(string id, bool active)
    onVisibleChanged: if (!visible) previewRequested("", false)
    signal saveRequested()
    signal exportRequested(string id)
    signal deleteRequested(string id, string name)
    signal applyRequested(string id)
    property string styleQuery: ""
    property string expandedStyleGroup: "monochrome"
    property var expandedStyleDetails: ({})

    function styleGroup(id) {
        let parts = id.split("/")
        return parts.length > 2 ? parts.slice(0, -2).join("/") : ""
    }
    function groupName(id) { return id.replace(/\//g, " · ").replace(/-/g, " ") }
    function groupOpen(id) { return id === "" || styleQuery.trim() !== "" || expandedStyleGroup === id }
    function toggleStyleDetails(id) {
        let next = Object.assign({}, expandedStyleDetails)
        next[id] = !next[id]
        expandedStyleDetails = next
    }
    function showDetails(id) {
        expandedStyleGroup = styleGroup(id)
        let next = Object.assign({}, expandedStyleDetails)
        next[id] = true
        expandedStyleDetails = next
    }
    readonly property var visibleStyles: root.styles.filter(function(p) {
        return (p.name + " " + p.description + " " + root.groupName(root.styleGroup(p.id))).toLowerCase().indexOf(styleQuery.trim().toLowerCase()) !== -1
    })
    readonly property var styleGroups: {
        let result = []
        for (let style of visibleStyles) {
            let key = styleGroup(style.id)
            let group = result.find(g => g.id === key)
            if (!group) {
                group = { id: key, name: groupName(key), styles: [] }
                result.push(group)
            }
            group.styles.push(style)
        }
        function rank(id) {
            if (id === "") return 0
            if (id === "my-styles") return 1
            if (id === "monochrome") return 2
            if (id === "experimental") return 4
            return 3
        }
        result.sort((a, b) => rank(a.id) - rank(b.id) || a.id.localeCompare(b.id))
        for (let group of result)
            group.styles.sort((a, b) => a.id === "neutral/style.dtstyle" ? -1 : b.id === "neutral/style.dtstyle" ? 1 : a.name.localeCompare(b.name))
        return result
    }
    Column {
        width: root.availableWidth; padding: 10
        Column {
            width: parent.width - 20; spacing: 10
            RowLayout {
                width: parent.width
                Text { text: "STYLES"; color: root.theme.ink; font.bold: true; font.letterSpacing: 2; Layout.fillWidth: true }
                Text { text: root.styles.length; color: root.theme.muted; font: root.theme.textFont }
            }
            Button { text: "Save current look…"; enabled: root.photoReady && !root.busy; onClicked: root.saveRequested() }
            TextField {
                id: styleSearch
                width: parent.width; height: 32
                placeholderText: "Search styles"; color: root.theme.ink
                placeholderTextColor: root.theme.muted; font: root.theme.textFont
                onTextChanged: root.styleQuery = text
                background: Rectangle { color: "#181825"; border.color: parent.activeFocus ? root.theme.accent : root.theme.line; radius: 3 }
                Accessible.name: "Search styles"
            }
            Text {
                width: parent.width; visible: !root.stylesReady || root.visibleStyles.length === 0
                text: !root.stylesReady ? "Loading styles…" : root.styles.length === 0 ? "No styles in styles/." : "No matching styles."
                color: root.theme.muted; font: root.theme.textFont; wrapMode: Text.WordWrap
            }
            Repeater {
                model: root.styleGroups
                delegate: Column {
                    id: groupSection
                    required property var modelData
                    width: parent.width; spacing: 2
                    Button {
                        objectName: "style-group-" + groupSection.modelData.id
                        width: parent.width; height: 36; padding: 0
                        visible: groupSection.modelData.id !== ""
                        onClicked: root.expandedStyleGroup = root.expandedStyleGroup === groupSection.modelData.id ? "" : groupSection.modelData.id
                        Accessible.name: groupSection.modelData.name
                        Accessible.description: root.groupOpen(groupSection.modelData.id) ? "Collapse group" : "Expand group"
                        background: Rectangle { color: parent.hovered ? "#313244" : "transparent"; border.color: parent.activeFocus ? root.theme.accent : "transparent"; radius: 3 }
                        contentItem: RowLayout {
                            spacing: 12
                            Text { Layout.preferredWidth: 14; text: root.groupOpen(groupSection.modelData.id) ? "▾" : "▸"; color: root.theme.muted; font: root.theme.textFont }
                            Text { Layout.fillWidth: true; text: groupSection.modelData.name.toUpperCase(); color: root.theme.ink; font.family: root.theme.textFont.family; font.pixelSize: 11; font.bold: true }
                            Text { text: groupSection.modelData.styles.length; color: root.theme.muted; font: root.theme.textFont }
                        }
                    }
                    Repeater {
                        model: root.groupOpen(groupSection.modelData.id) ? groupSection.modelData.styles : []
                        delegate: StyleCard {
                            required property var modelData
                            theme: root.theme
                            style: modelData
                            photoReady: root.photoReady
                            busy: root.busy
                            appliedStyle: root.appliedStyle
                            applyingStyle: root.applyingStyle
                            expanded: !!root.expandedStyleDetails[modelData.id]
                            onExportRequested: root.exportRequested(modelData.id)
                            onDeleteRequested: root.deleteRequested(modelData.id, modelData.name)
                            onPreviewRequested: active => root.previewRequested(modelData.id, active)
                            onApplyRequested: root.applyRequested(modelData.id)
                            onDetailsToggleRequested: root.toggleStyleDetails(modelData.id)
                        }
                    }
                }
            }
            Text { width: parent.width; visible: root.errorMessage !== ""; text: root.errorMessage; color: "#f9d58b"; font: root.theme.textFont; wrapMode: Text.WordWrap }
        }
    }
}
