import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import "panels"
import "components"

Rectangle {
    id: root
    required property var theme
    required property var backend
    required property string activeControl
    required property url iconsRoot
    property alias geometry: geometryPanel
    // Module states for the tools drawn on the photo (Main.qml passes them to the viewport).
    readonly property alias moduleStates: moduleCatalog.states
    property alias tools: moduleTools
    // A module picker belongs to the pane it was started in.
    // The group is looked up here, not read from currentGroup: that binding may still hold the
    // previous area while this handler runs, which would file the Styles pane under Edit.
    onSelectedPanelChanged: { const g = groupOf(selectedPanel); if (lastPaneInGroup[g] !== selectedPanel) { let next = Object.assign({}, lastPaneInGroup); next[g] = selectedPanel; lastPaneInGroup = next } moduleTools.cancel(); if (selectedPanel !== 2) geometryPanel.cancel(); if (selectedPanel === 2) controlSelected("rotation"); else if (selectedPanel === 0 && activeControl === "rotation") controlSelected("exposure") }
    property int selectedPanel: 0
    // 0 = the designed controls, 1 = the parameters still without one (developer mode only).
    property int filterView: 0
    property alias filterSearch: modulesPanel.search
    property alias moduleSearch: moduleSearch.text
    // Panes in strip order, grouped into three areas: editing the photograph, applying a look,
    // and reading about it. The pane indices stay as they were (Main.qml, GeometryPanel); the
    // keys 1–9 follow this order (edit panes 1–6, Styles 7, History 8, Info 9).
    readonly property var panes: [
        { index: 0, icon: "edit.svg", name: "Filters", label: "Filters", group: "edit" },
        { index: 5, icon: "tone.svg", name: "Tone · base and tone modules", label: "Tone", tab: "tone", group: "edit" },
        { index: 6, icon: "color.svg", name: "Color modules", label: "Color", tab: "color", group: "edit" },
        { index: 7, icon: "detail.svg", name: "Detail & correction · technical", label: "Detail", tab: "detail", group: "edit" },
        { index: 8, icon: "effects.svg", name: "Effects", label: "Effects", tab: "effects", group: "edit" },
        { index: 2, icon: "crop.svg", name: "Crop & Rotate", label: "Crop", group: "edit" },
        { index: 1, icon: "styles.svg", name: "Styles", label: "Styles", group: "styles" },
        { index: 3, icon: "history.svg", name: "History", label: "History", group: "details" },
        { index: 4, icon: "info.svg", name: "Info", label: "Info", group: "details" }
    ]
    readonly property var paneGroups: [
        { id: "edit", icon: "edit.svg", label: "Edit", name: "Edit the photograph" },
        { id: "styles", icon: "styles.svg", label: "Styles", name: "Apply a look" },
        { id: "details", icon: "info.svg", label: "Details", name: "History and image information" }
    ]
    function groupOf(index) { return (panes.find(p => p.index === index) || panes[0]).group }
    readonly property string currentGroup: groupOf(selectedPanel)
    readonly property var groupPanes: panes.filter(p => p.group === currentGroup)
    // The pane each area returns to when it is chosen again.
    property var lastPaneInGroup: ({ edit: 0, styles: 1, details: 3 })
    function selectGroup(id) { selectedPanel = lastPaneInGroup[id] }
    readonly property var paneOrder: panes.map(p => p.index)
    readonly property bool searchable: [3, 4].indexOf(selectedPanel) < 0
    readonly property string term: moduleSearch.text.trim().toLowerCase()
    function matchesIn(index) {
        if (!term) return 0
        switch (index) {
        case 0: return filtersPanel.matchCount
        case 1: return moduleCatalog.matchCount("styles", term) + stylesPanel.matchCount(term)
        case 2: return moduleCatalog.matchCount("geometry", term)
        case 5: return moduleCatalog.matchCount("tone", term)
        case 6: return moduleCatalog.matchCount("color", term)
        case 7: return moduleCatalog.matchCount("detail", term)
        case 8: return moduleCatalog.matchCount("effects", term)
        default: return 0
        }
    }
    // Searching jumps to the first pane with a match when the current one has none.
    Timer {
        id: searchJump
        interval: 60
        onTriggered: {
            if (!root.term || root.matchesIn(root.selectedPanel) > 0) return
            for (const index of root.paneOrder) if (root.matchesIn(index) > 0) { root.selectedPanel = index; return }
        }
    }
    onTermChanged: searchJump.restart()

    ModuleCatalog {
        id: moduleCatalog
        catalog: root.backend.moduleCatalog
        layoutText: root.backend.layoutData || "{}"
        blendLayoutText: root.backend.blendLayoutData || "{}"
        onStatesChanged: parameterQueue.acknowledge()
        // Runtime lists of module rows (profiles, lenses, files): asked here, answered below.
        onChoicesRequested: (operation, instance, list, query) => {
            if (typeof root.backend.requestChoices === "function")
                root.backend.requestChoices(operation, instance, list, query)
        }
        tools: moduleTools
    }
    ParameterQueue {
        id: parameterQueue
        backend: root.backend
        // Whether the catalog shows the value sent for "operation/instance/path" yet.
        confirmed: (key, value) => {
            const parts = key.split("/")
            const state = moduleCatalog.states[parts[0] + "/" + parts[1]]
            const path = parts.slice(2).join("/")
            if (!state) return true
            let shown
            if (path === "@enabled") shown = state.enabled
            else if (path.startsWith("blend.")) {
                const m = /^blend\.([a-z_]+)(?:\[(\d+)\])?$/.exec(path)
                shown = m && state.blend ? (m[2] !== undefined ? (state.blend[m[1]] || [])[Number(m[2])] : state.blend[m[1]]) : undefined
            } else shown = moduleCatalog.resolve(state, path)
            if (shown === undefined) return true
            if (typeof shown === "boolean") shown = shown ? 1 : 0
            if (typeof shown === "number" && typeof value === "number")
                return Math.abs(shown - value) <= 1e-4 * Math.max(1, Math.abs(value))
            return JSON.stringify(shown) === JSON.stringify(value)
        }
    }
    // darktable's pickers and module buttons (ModuleTools.qml); rows reach it as catalogModel.tools.
    ModuleTools {
        id: moduleTools
        available: typeof root.backend.moduleTools === "function" ? root.backend.moduleTools() : "[]"
        onRunRequested: (operation, instance, request) => root.backend.runModuleTool(operation, instance, request)
    }
    Connections {
        target: root.backend
        ignoreUnknownSignals: true
        function onModuleUpdated(operation, instance, moduleJson) { moduleCatalog.updateModule(operation, instance, moduleJson) }
        function onChoicesReady(operation, instance, list, query, result) { moduleCatalog.receiveChoices(operation, instance, list, query, result) }
        function onModuleToolResult(operation, instance, tool, result, error) { moduleTools.accept(operation, instance, tool, result, error) }
    }
    // The engine switches a module on when one of its parameters is edited, as darktable does.
    // Action keys ride along with the edits so every pane reaches them without extra wiring:
    // "@instance" is darktable's multi-instance menu ("@name" for rename), "@reset" a module
    // reset and "@drawn" a request to draw a mask shape of that type on the image.
    signal drawnShapeRequested(string operation, int instance, int shape)
    function changeParameters(operation, instance, changes) {
        if ("@instance" in changes) {
            if (typeof root.backend.moduleInstance === "function")
                root.backend.moduleInstance(operation, instance, changes["@instance"], changes["@name"] || "")
            return
        }
        if ("@reset" in changes) { root.resetModule(operation, instance, moduleCatalog.modulesByOperation[operation] || { rows: [] }); return }
        if ("@drawn" in changes) { root.drawnShapeRequested(operation, instance, changes["@drawn"]); return }
        moduleTools.parameterEdited(operation, instance)
        parameterQueue.send(operation, instance, changes)
    }
    function enableModule(operation, instance, enabled) {
        moduleTools.parameterEdited(operation, instance)
        parameterQueue.send(operation, instance, { "@enabled": enabled ? 1 : 0 })
    }
    function resetModule(operation, instance, module) {
        moduleTools.parameterEdited(operation, instance)
        if (typeof root.backend.resetModule === "function") { root.backend.resetModule(operation, instance); return }
        const changes = {}
        for (const r of module.rows)
            if (moduleCatalog.writable(r.path) && r.default !== null && typeof r.default !== "object" && typeof r.default !== "string")
                changes[r.path] = Number(r.default)
        if (Object.keys(changes).length) parameterQueue.send(operation, instance, changes)
    }
    signal styleSaveRequested()
    signal styleExportRequested(string id)
    signal styleDeleteRequested(string id, string name)
    signal controlSelected(string id)

    function revealControl(id) { selectedPanel = 0; filterView = 0; controlSelected(id); filtersPanel.reveal(id) }
    function toggleGrainDetails() { filtersPanel.toggleGrainDetails() }
    function showStyleDetails(id) {
        selectedPanel = 1;
        stylesPanel.showDetails(id);
    }

    implicitWidth: 352
    color: theme.background
    Rectangle {
        width: 1
        height: parent.height
        color: root.theme.line
    }
    ColumnLayout {
        anchors.fill: parent
        anchors.margins: 12
        spacing: 14
        // Area switch: editing, styles, details. The panes of the chosen area sit below it.
        Rectangle {
            Layout.fillWidth: true
            implicitHeight: areaRow.implicitHeight + 6
            radius: 7
            color: root.theme.well
            border.color: root.theme.line
            border.width: 1
            RowLayout {
                id: areaRow
                anchors.fill: parent
                anchors.margins: 3
                spacing: 3
                Repeater {
                    model: root.paneGroups
                    Button {
                        id: area
                        required property var modelData
                        objectName: "sidebar-area-" + modelData.id
                        readonly property bool current: root.currentGroup === modelData.id
                        readonly property bool hasMatches: root.term !== "" && root.panes.some(p => p.group === modelData.id && root.matchesIn(p.index) > 0)
                        Layout.fillWidth: true
                        Layout.preferredWidth: 1
                        implicitHeight: 32
                        padding: 0; leftPadding: 8; rightPadding: 8
                        hoverEnabled: true
                        onClicked: root.selectGroup(modelData.id)
                        Accessible.name: modelData.name
                        Accessible.role: Accessible.PageTab
                        Accessible.selected: current
                        // Names the area or pane before it is chosen; gone once it is current, so
                        // it never stays over the strip after the click.
                        ToolTip.visible: hovered && !pressed && !current
                        ToolTip.delay: 500
                        ToolTip.text: modelData.name
                        background: Rectangle {
                            radius: 5
                            color: area.current ? root.theme.active : area.pressed ? root.theme.active : area.hovered ? root.theme.hover : "transparent"
                            border.width: area.visualFocus || area.current ? 1 : 0
                            border.color: area.visualFocus ? root.theme.accent : root.theme.line
                        }
                        display: AbstractButton.TextBesideIcon
                        text: modelData.label
                        font.family: root.theme.textFont.family
                        font.pixelSize: root.theme.textFont.pixelSize
                        font.weight: Font.Medium
                        spacing: 6
                        icon.source: root.iconsRoot + modelData.icon
                        icon.width: 16; icon.height: 16
                        icon.color: current ? root.theme.accent : hovered ? root.theme.ink : root.theme.muted
                        palette.buttonText: current ? root.theme.accent : hovered ? root.theme.ink : root.theme.muted
                        Rectangle {
                            visible: area.hasMatches && !area.current
                            width: 4; height: 4; radius: 2
                            color: root.theme.accent
                            anchors.horizontalCenter: parent.horizontalCenter
                            anchors.bottom: parent.bottom; anchors.bottomMargin: 3
                        }
                    }
                }
            }
        }
        // Pane tabs: a recessed strip with the active pane raised inside it.
        Rectangle {
            Layout.fillWidth: true
            Layout.topMargin: -8
            // The panes of the current area, each an icon with its name beside it; hidden for an
            // area with a single pane (Styles).
            visible: root.groupPanes.length > 1
            implicitHeight: tabGrid.implicitHeight + 6
            radius: 7
            color: root.theme.well
            border.color: root.theme.line
            border.width: 1
            GridLayout {
                id: tabGrid
                anchors.fill: parent
                anchors.margins: 3
                columns: 3
                rowSpacing: 3; columnSpacing: 3
                Repeater {
                    model: root.groupPanes
                    Button {
                        id: tab
                        required property var modelData
                        readonly property int paneIndex: modelData.index
                        objectName: "sidebar-tab-" + paneIndex
                        readonly property bool current: root.selectedPanel === paneIndex
                        readonly property bool hasMatches: root.term !== "" && root.matchesIn(paneIndex) > 0
                        Layout.fillWidth: true
                        Layout.preferredWidth: 1
                        implicitHeight: 30
                        padding: 0; leftPadding: 8; rightPadding: 8
                        hoverEnabled: true
                        onClicked: root.selectedPanel = paneIndex
                        Accessible.name: modelData.name
                        Accessible.role: Accessible.PageTab
                        Accessible.selected: current
                        // Names the area or pane before it is chosen; gone once it is current, so
                        // it never stays over the strip after the click.
                        ToolTip.visible: hovered && !pressed && !current
                        ToolTip.delay: 500
                        ToolTip.text: modelData.name
                        background: Rectangle {
                            radius: 5
                            color: tab.current ? root.theme.active
                                 : tab.pressed ? root.theme.active
                                 : tab.hovered ? root.theme.hover : "transparent"
                            border.width: tab.visualFocus || tab.current ? 1 : 0
                            border.color: tab.visualFocus ? root.theme.accent : root.theme.line
                        }
                        display: AbstractButton.TextBesideIcon
                        text: modelData.label
                        font.family: root.theme.textFont.family
                        font.pixelSize: root.theme.textFont.pixelSize - 1
                        spacing: 6
                        icon.source: root.iconsRoot + modelData.icon
                        icon.width: 16; icon.height: 16
                        icon.color: current ? root.theme.accent : hovered ? root.theme.ink : root.theme.muted
                        palette.buttonText: current ? root.theme.accent : hovered ? root.theme.ink : root.theme.muted
                        // While searching, a dot marks the panes with matches.
                        Rectangle {
                            visible: tab.hasMatches && !tab.current
                            width: 4; height: 4; radius: 2
                            color: root.theme.accent
                            anchors.horizontalCenter: parent.horizontalCenter
                            anchors.bottom: parent.bottom; anchors.bottomMargin: 3
                        }
                    }
                }
            }
        }
        // One search over every module pane; Escape clears it.
        TextField {
            id: moduleSearch
            objectName: "module-search"
            Layout.fillWidth: true
            Layout.leftMargin: 4; Layout.rightMargin: 4
            Layout.topMargin: -6
            implicitHeight: 28
            visible: root.searchable
            leftPadding: 28
            placeholderText: "Search modules and controls"
            placeholderTextColor: root.theme.muted
            color: root.theme.ink
            font: root.theme.textFont
            selectByMouse: true
            // Escape clears the search and gives the keys back; Enter or ↓ keep it and go to the results.
            Keys.onEscapePressed: { text = ""; focus = false }
            NavTarget { id: searchNav; navId: "search"; kind: "search"; label: "search"; listed: false; input: moduleSearch; onActivate: moduleSearch.forceActiveFocus() }
            Accessible.name: "Search modules and controls"
            background: Rectangle {
                radius: 5
                color: root.theme.well
                border.color: moduleSearch.activeFocus || searchNav.current ? root.theme.accent : root.theme.line
                Canvas {
                    x: 9; y: (parent.height - 12) / 2
                    width: 12; height: 12
                    property color stroke: moduleSearch.activeFocus ? root.theme.accent : root.theme.muted
                    onStrokeChanged: requestPaint()
                    onPaint: {
                        const c = getContext("2d")
                        c.clearRect(0, 0, width, height)
                        c.strokeStyle = stroke; c.lineWidth = 1.3
                        c.beginPath(); c.arc(5, 5, 3.8, 0, Math.PI * 2); c.stroke()
                        c.beginPath(); c.moveTo(8, 8); c.lineTo(11, 11); c.stroke()
                    }
                }
            }
            ToolButton {
                visible: moduleSearch.text !== ""
                anchors.right: parent.right; anchors.verticalCenter: parent.verticalCenter
                width: 26; height: 26; padding: 0
                onClicked: moduleSearch.text = ""
                Accessible.name: "Clear search"
                contentItem: Text { text: "×"; color: parent.hovered ? root.theme.accent : root.theme.muted; font: root.theme.settingsFont
                                    horizontalAlignment: Text.AlignHCenter; verticalAlignment: Text.AlignVCenter }
                background: Item {}
            }
        }
        // While building the interface, switch between the designed controls and the
        // parameters that still wait for one.
        RowLayout {
            Layout.fillWidth: true
            Layout.leftMargin: 18
            Layout.rightMargin: 18
            spacing: 0
            visible: root.backend.developerMode && root.selectedPanel === 0
            Repeater {
                model: ["Curated", "Open"]
                Button {
                    id: viewButton
                    required property string modelData
                    required property int index
                    objectName: "filter-view-" + index
                    Layout.fillWidth: true
                    implicitHeight: 24
                    padding: 0
                    onClicked: root.filterView = index
                    contentItem: Text {
                        text: viewButton.modelData
                        color: root.filterView === viewButton.index ? root.theme.accent : root.theme.muted
                        font.family: root.theme.textFont.family
                        font.pixelSize: root.theme.textFont.pixelSize
                        horizontalAlignment: Text.AlignHCenter
                        verticalAlignment: Text.AlignVCenter
                    }
                    background: Rectangle {
                        color: root.filterView === viewButton.index
                               ? root.theme.surface : "transparent"
                        border.color: root.filterView === viewButton.index ? root.theme.line : "transparent"
                        radius: 4
                    }
                }
            }
        }
        FiltersPanel {
            id: filtersPanel
            onInteractionChanged: active => root.backend.setInteractive(active)
            onHalationRequested: root.backend.applyHalation()
            activeControl: root.activeControl
            onControlReset: id => root.backend.resetControl(id)
            Layout.fillWidth: true
            Layout.fillHeight: true
            visible: root.selectedPanel === 0 && root.filterView === 0
            theme: root.theme
            controls: root.backend.controls
            values: root.backend.controlValues
            editable: !root.backend.styleBusy && root.backend.preview !== ""
            onControlSelected: id => root.controlSelected(id)
            onControlEdited: (id, value) => root.backend.setControl(id, value)
            catalogModel: moduleCatalog
            states: moduleCatalog.states
            overrides: parameterQueue.overrides
            term: root.term
            onParameterChangesRequested: (operation, instance, changes) => root.changeParameters(operation, instance, changes)
        }
        Repeater {
            model: [
                { index: 5, component: tonePane },
                { index: 6, component: colorPane },
                { index: 7, component: detailPane },
                { index: 8, component: effectsPane }
            ]
            // Built on first visit, then kept so scroll position and expansion survive.
            Loader {
                required property var modelData
                property bool visited: false
                readonly property bool current: root.selectedPanel === modelData.index
                onCurrentChanged: if (current) visited = true
                Component.onCompleted: if (current) visited = true
                Layout.fillWidth: true
                Layout.fillHeight: true
                visible: current
                active: visited
                sourceComponent: modelData.component
            }
        }
        StylesPanel {
            id: stylesPanel
            onPreviewRequested: (id, active) => root.backend.hoverStyle(id, active)
            onSaveRequested: root.styleSaveRequested()
            onExportRequested: id => root.styleExportRequested(id)
            onDeleteRequested: (id, name) => root.styleDeleteRequested(id, name)
            Layout.fillWidth: true
            Layout.fillHeight: true
            visible: root.selectedPanel === 1
            theme: root.theme
            styles: root.backend.styles
            stylesReady: root.backend.stylesReady
            photoReady: root.backend.preview !== ""
            busy: root.backend.styleBusy
            appliedStyle: root.backend.activeStyle
            applyingStyle: root.backend.applyingStyle
            errorMessage: root.backend.styleError
            onApplyRequested: id => root.backend.applyStyle(id)
            cameraDefaults: root.backend.cameraDefaults
            camera: { const c = String(root.backend.metadata.camera || "").replace(/\(null\)/g, "").trim(); return c }
            catalogModel: moduleCatalog
            states: moduleCatalog.states
            overrides: parameterQueue.overrides
            term: root.term
            activeControl: root.activeControl
            onChangesRequested: (operation, instance, changes) => root.changeParameters(operation, instance, changes)
            onEnableRequested: (operation, instance, enabled) => root.enableModule(operation, instance, enabled)
            onModuleResetRequested: (operation, instance, module) => root.resetModule(operation, instance, module)
            onModuleInteractionChanged: active => root.backend.setInteractive(active)
            onControlSelected: id => root.controlSelected(id)
        }
        ModulesPanel {
            id: modulesPanel
            visible: root.selectedPanel === 0 && root.filterView === 1
            Layout.fillWidth: true
            Layout.fillHeight: true
            theme: root.theme
            catalog: root.backend.moduleCatalog
            display: root.backend.displayData
            editable: !root.backend.styleBusy && root.backend.preview !== ""
            onInteractionChanged: active => root.backend.setInteractive(active)
            onParameterEdited: (operation, instance, field, value) => root.backend.setParameter(operation, instance, field, value)
        }
        GeometryPanel {
            id: geometryPanel
            onInteractionChanged: active => root.backend.setInteractive(active)
            imageAspect: root.backend.metadata.width / Math.max(1,root.backend.metadata.height)
            visible: root.selectedPanel === 2
            Layout.fillWidth: true; Layout.fillHeight: true
            theme: root.theme; controls: root.backend.controls; values: root.backend.controlValues
            editable: !root.backend.styleBusy && root.backend.preview !== ""
            onEdited: (id, value) => root.backend.setControl(id, value)
            onCropApplied: values => root.backend.setControls(values)
            catalogModel: moduleCatalog
            states: moduleCatalog.states
            overrides: parameterQueue.overrides
            term: root.term
            activeControl: root.activeControl
            onChangesRequested: (operation, instance, changes) => root.changeParameters(operation, instance, changes)
            onEnableRequested: (operation, instance, enabled) => root.enableModule(operation, instance, enabled)
            onModuleResetRequested: (operation, instance, module) => root.resetModule(operation, instance, module)
            onModuleInteractionChanged: active => root.backend.setInteractive(active)
            onControlSelected: id => root.controlSelected(id)
        }
        HistoryPanel {
            visible: root.selectedPanel === 3
            Layout.fillWidth: true
            Layout.fillHeight: true
            theme: root.theme
            entries: root.backend.history
            cameraDefaults: root.backend.cameraDefaults
            busy: root.backend.styleBusy
            onStepRequested: step => root.backend.selectHistory(step)
            ready: root.backend.preview !== ""
        }
        MetadataPanel {
            visible: root.selectedPanel === 4
            Layout.fillWidth: true; Layout.fillHeight: true
            theme: root.theme; metadata: root.backend.metadata
        }
    }
    Component {
        id: tonePane
        TonePanel {
            theme: root.theme
            catalogModel: moduleCatalog
            states: moduleCatalog.states
            overrides: parameterQueue.overrides
            editable: !root.backend.styleBusy && root.backend.preview !== ""
            term: root.term
            activeControl: root.activeControl
            onChangesRequested: (operation, instance, changes) => root.changeParameters(operation, instance, changes)
            onEnableRequested: (operation, instance, enabled) => root.enableModule(operation, instance, enabled)
            onResetRequested: (operation, instance, module) => root.resetModule(operation, instance, module)
            onInteractionChanged: active => root.backend.setInteractive(active)
            onControlSelected: id => root.controlSelected(id)
        }
    }
    Component {
        id: colorPane
        ColorPanel {
            theme: root.theme
            catalogModel: moduleCatalog
            states: moduleCatalog.states
            overrides: parameterQueue.overrides
            editable: !root.backend.styleBusy && root.backend.preview !== ""
            term: root.term
            activeControl: root.activeControl
            onChangesRequested: (operation, instance, changes) => root.changeParameters(operation, instance, changes)
            onEnableRequested: (operation, instance, enabled) => root.enableModule(operation, instance, enabled)
            onResetRequested: (operation, instance, module) => root.resetModule(operation, instance, module)
            onInteractionChanged: active => root.backend.setInteractive(active)
            onControlSelected: id => root.controlSelected(id)
        }
    }
    Component {
        id: detailPane
        DetailPanel {
            theme: root.theme
            catalogModel: moduleCatalog
            states: moduleCatalog.states
            overrides: parameterQueue.overrides
            editable: !root.backend.styleBusy && root.backend.preview !== ""
            term: root.term
            activeControl: root.activeControl
            onChangesRequested: (operation, instance, changes) => root.changeParameters(operation, instance, changes)
            onEnableRequested: (operation, instance, enabled) => root.enableModule(operation, instance, enabled)
            onResetRequested: (operation, instance, module) => root.resetModule(operation, instance, module)
            onInteractionChanged: active => root.backend.setInteractive(active)
            onControlSelected: id => root.controlSelected(id)
        }
    }
    Component {
        id: effectsPane
        EffectsPanel {
            theme: root.theme
            catalogModel: moduleCatalog
            states: moduleCatalog.states
            overrides: parameterQueue.overrides
            editable: !root.backend.styleBusy && root.backend.preview !== ""
            term: root.term
            activeControl: root.activeControl
            onChangesRequested: (operation, instance, changes) => root.changeParameters(operation, instance, changes)
            onEnableRequested: (operation, instance, enabled) => root.enableModule(operation, instance, enabled)
            onResetRequested: (operation, instance, module) => root.resetModule(operation, instance, module)
            onInteractionChanged: active => root.backend.setInteractive(active)
            onControlSelected: id => root.controlSelected(id)
        }
    }
}
