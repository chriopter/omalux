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
    signal previewRequested(string id, bool active)
    onVisibleChanged: if (!visible) previewRequested("", false)
    signal saveRequested()
    signal exportRequested(string id)
    signal deleteRequested(string id, string name)
    signal applyRequested(string id)
    // darktable modules that belong to this pane (generated layout).
    property var catalogModel: null
    property var states: ({})
    property var overrides: ({})
    property string term: ""
    property string activeControl: ""
    signal changesRequested(string operation, int instance, var changes)
    signal enableRequested(string operation, int instance, bool enabled)
    signal moduleResetRequested(string operation, int instance, var module)
    signal moduleInteractionChanged(bool active)
    signal controlSelected(string id)
    // What darktable set up for this camera (backend.cameraDefaults); this pane shows the
    // Omalux camera presets among it, read-only.
    property var cameraDefaults: []
    property string camera: ""
    property bool cameraOpen: false
    readonly property var cameraPresets: (cameraDefaults || []).filter(e => e.group === "Camera presets")
    property string styleQuery: ""
    property string expandedStyleGroup: "monochrome"
    property var expandedStyleDetails: ({})

    function styleGroup(id) {
        let parts = id.split("/")
        return parts.length > 2 ? parts.slice(0, -2).join("/") : ""
    }
    function groupName(id) { return id.replace(/\//g, " · ").replace(/-/g, " ") }
    function groupOpen(id) { return id === "" || styleQuery.trim() !== "" || expandedStyleGroup === id }
    function toggleGroup(id) { expandedStyleGroup = expandedStyleGroup === id ? "" : id }
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
            // Camera presets darktable applied automatically, before any style.
            Rectangle {
                objectName: "camera-presets"
                width: parent.width
                height: cameraBlock.implicitHeight + 14
                radius: 4
                color: root.theme.surface
                visible: root.photoReady
                Column {
                    id: cameraBlock
                    x: 10; y: 7
                    width: parent.width - 20
                    spacing: 5
                    ToolButton {
                        id: cameraHeading
                        objectName: "camera-presets-toggle"
                        width: parent.width
                        padding: 0
                        hoverEnabled: true
                        onClicked: { cameraNav.claim(); root.cameraOpen = !root.cameraOpen }
                        Accessible.name: "Camera presets"
                        NavTarget {
                            id: cameraNav
                            navId: "camera"; label: "camera presets"; kind: "group"
                            activateLabel: root.cameraOpen ? "COLLAPSE" : "EXPAND"
                            onActivate: root.cameraOpen = !root.cameraOpen
                            onAdjust: steps => root.cameraOpen = steps > 0
                        }
                        Accessible.description: root.cameraOpen ? "Collapse" : "Expand"
                        contentItem: RowLayout {
                            spacing: 6
                            Text {
                                text: "camera"
                                color: cameraHeading.hovered || cameraNav.current ? root.theme.accent : root.theme.ink
                                font: root.theme.moduleHeadingFont
                            }
                            Text {
                                Layout.fillWidth: true
                                text: root.camera || "not identified"
                                color: root.theme.muted; font: root.theme.textFont
                                elide: Text.ElideRight
                            }
                            Text {
                                text: { const n = root.cameraPresets.filter(e => e.enabled).length; return n + (n === 1 ? " preset" : " presets") }
                                color: root.theme.muted; font: root.theme.textFont
                            }
                            DisclosureButton {
                                theme: root.theme
                                expanded: root.cameraOpen
                                onClicked: root.cameraOpen = !root.cameraOpen
                                Accessible.name: (root.cameraOpen ? "Hide" : "Show") + " camera presets"
                            }
                        }
                        background: Item {}
                    }
                    Text {
                        visible: root.cameraOpen && root.cameraPresets.length === 0
                        width: parent.width
                        text: "No Omalux camera preset matches this camera."
                        color: root.theme.muted; font: root.theme.textFont; wrapMode: Text.WordWrap
                    }
                    Repeater {
                        model: root.cameraOpen ? root.cameraPresets : []
                        RowLayout {
                            required property var modelData
                            width: cameraBlock.width
                            spacing: 8
                            opacity: modelData.enabled ? 1 : .55
                            Rectangle {
                                width: 5; height: 5; radius: 2.5
                                color: modelData.enabled ? root.theme.accent : root.theme.muted
                            }
                            Text { text: modelData.label; color: root.theme.ink; font: root.theme.textFont }
                            Text {
                                Layout.fillWidth: true
                                horizontalAlignment: Text.AlignRight
                                text: modelData.enabled ? modelData.value : modelData.value + " · not applied"
                                color: modelData.enabled ? root.theme.ink : root.theme.muted
                                font: root.theme.textFont; elide: Text.ElideRight
                            }
                        }
                    }
                }
            }
            Button {
                id: saveButton
                text: "Save current look…"; enabled: root.photoReady && !root.busy; onClicked: root.saveRequested()
                highlighted: saveNav.current
                NavTarget { id: saveNav; navId: "save"; label: "save current look"; enabled: saveButton.enabled; onActivate: root.saveRequested() }
            }
            TextField {
                id: styleSearch
                objectName: "styleSearch"
                width: parent.width; height: 32
                placeholderText: "Search styles"; color: root.theme.ink
                placeholderTextColor: root.theme.muted; font: root.theme.textFont
                onTextChanged: root.styleQuery = text
                NavTarget { id: searchNav; navId: "style-search"; kind: "search"; label: "search styles"; input: styleSearch; onActivate: styleSearch.forceActiveFocus() }
                background: Rectangle { color: "#181825"; border.color: parent.activeFocus || searchNav.current ? root.theme.accent : root.theme.line; radius: 3 }
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
                        id: groupButton
                        objectName: "style-group-" + groupSection.modelData.id
                        width: parent.width; height: 36; padding: 0
                        visible: groupSection.modelData.id !== ""
                        onClicked: { groupNav.claim(); root.toggleGroup(groupSection.modelData.id) }
                        NavTarget {
                            id: groupNav
                            navId: "group:" + groupSection.modelData.id
                            label: groupSection.modelData.name
                            kind: "group"
                            group: groupSection.modelData.id
                            activateLabel: root.groupOpen(groupSection.modelData.id) ? "COLLAPSE" : "EXPAND"
                            onActivate: root.toggleGroup(groupSection.modelData.id)
                            onAdjust: steps => { if ((steps > 0) !== root.groupOpen(groupSection.modelData.id)) root.toggleGroup(groupSection.modelData.id) }
                        }
                        Accessible.name: groupSection.modelData.name
                        Accessible.description: root.groupOpen(groupSection.modelData.id) ? "Collapse group" : "Expand group"
                        background: Rectangle { color: parent.hovered ? "#313244" : "transparent"; border.color: parent.activeFocus || groupNav.current ? root.theme.accent : "transparent"; radius: 3 }
                        contentItem: RowLayout {
                            spacing: 12
                            Text { Layout.preferredWidth: 14; text: root.groupOpen(groupSection.modelData.id) ? "▾" : "▸"; color: root.theme.muted; font: root.theme.textFont }
                            Text { Layout.fillWidth: true; text: groupSection.modelData.name.toUpperCase(); color: root.theme.ink; font.family: root.theme.textFont.family; font.pixelSize: root.theme.textFont.pixelSize; font.bold: true }
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
                            navTarget.group: groupSection.modelData.id
                        }
                    }
                }
            }
            Loader {
                active: !!root.catalogModel
                x: 8
                width: parent.width - 8
                sourceComponent: ModuleList {
                    theme: root.theme
                    groups: root.catalogModel.groupsForTab("styles")
                    catalogModel: root.catalogModel
                    states: root.states
                    overrides: root.overrides
                    editable: !root.busy && root.photoReady
                    // Only while the pane is on screen (search opens every match).
                    term: root.visible ? root.term : ""
                    activeControl: root.activeControl
                    settingsKey: "styles"
                    caption: "LOOK MODULES"
                    onChangesRequested: (operation, instance, changes) => root.changesRequested(operation, instance, changes)
                    onEnableRequested: (operation, instance, enabled) => root.enableRequested(operation, instance, enabled)
                    onResetRequested: (operation, instance, module) => root.moduleResetRequested(operation, instance, module)
                    onInteractionChanged: active => root.moduleInteractionChanged(active)
                    onControlSelected: id => root.controlSelected(id)
                }
            }
            Text { width: parent.width; visible: root.errorMessage !== ""; text: root.errorMessage; color: "#f9d58b"; font: root.theme.textFont; wrapMode: Text.WordWrap }
        }
    }
}
