import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import QtCore

import "../components"

SidebarScrollView {
    id: root
    objectName: "stylesPanel"
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
    // What darktable set up for this camera (backend.cameraDefaults); the Camera pane shows the
    // Omalux camera presets among it, and below them every camera preset (backend.cameraPresets)
    // to put on the photograph by hand.
    property var cameraDefaults: []
    property var allCameraPresets: []
    signal cameraPresetRequested(string name, bool on)
    property string camera: ""
    property bool cameraOpen: true
    // The Styles area has three panes, all this component: "looks" (the library grid), "modules"
    // (the darktable modules that shape a look) and "camera" (what was applied for this camera).
    property string view: "looks"
    readonly property bool looksView: view === "looks"
    // The look modules as one flat list of cards.
    readonly property var lookModules: {
        if (!catalogModel) return []
        const modules = []
        for (const g of catalogModel.groupsForTab("styles")) for (const m of g.modules) modules.push(m)
        return modules.length ? [{ id: "looks", label: "Look modules", fixed: true, quiet: false, modules: modules }] : []
    }
    readonly property var cameraPresets: (cameraDefaults || []).filter(e => e.group === "Camera presets")
    // A look may name the film profile it belongs with (style.json "film": the film's id or
    // name). While that look is applied and no film is on, the Looks pane offers the film; it
    // is never put on by itself.
    readonly property var lookFilm: {
        const style = (root.styles || []).find(s => s.name === root.appliedStyle)
        if (!style || !style.film) return null
        const films = (root.allCameraPresets || []).filter(p => !!p.film)
        if (films.some(p => p.applied)) return null
        return films.find(p => p.available && (p.id === style.film || p.title === style.film || p.name === style.film)) || null
    }
    // The looks as a thumbnail grid: favourites (and the basic looks) on top, then one
    // collapsible group per family of the catalogue (catalog/styles/<family>/…, series by their
    // sub-folder; named by the folders, or by the `family` a look carries from a family.json in
    // the catalogue, as "DHH" for dhh/), one family open at a time, each showing its first `limit` looks until "show
    // all". A filter (this pane's field, or the sidebar search) opens every family with a match.
    // Favourites and the open family are remembered in the app settings.
    Settings {
        id: preferences
        category: "Styles"
        property string favourites: "[]"
        property string openFamily: "film"
    }
    property var favourites: []
    property string expandedStyleGroup: "film"
    property bool restoring: true
    Component.onCompleted: {
        try { favourites = JSON.parse(preferences.favourites) } catch (e) {}
        expandedStyleGroup = preferences.openFamily
        restoring = false
    }
    onFavouritesChanged: if (!restoring) preferences.favourites = JSON.stringify(favourites)
    onExpandedStyleGroupChanged: if (!restoring) preferences.openFamily = expandedStyleGroup
    function isFavourite(id) { return favourites.indexOf(id) >= 0 }
    function toggleFavourite(id) {
        favourites = isFavourite(id) ? favourites.filter(f => f !== id) : favourites.concat([id])
    }
    readonly property int limit: 12
    property var showAll: ({})
    function toggleShowAll(id) { const next = Object.assign({}, showAll); next[id] = !next[id]; showAll = next }
    property string styleQuery: ""
    readonly property string query: (styleQuery.trim() || (visible && looksView ? term : "")).toLowerCase()
    // The look whose settings are shown, and the section it was opened in ("key|id").
    property string details: ""
    function toggleStyleDetails(section, id) { details = details === section + "|" + id ? "" : section + "|" + id }

    function styleGroup(id) {
        let parts = id.split("/")
        return parts.length > 2 ? parts.slice(0, -2).join("/") : ""
    }
    function groupName(id) {
        return id.split("/").map(p => p.replace(/-/g, " ")).map(p => p.charAt(0).toUpperCase() + p.slice(1)).join(" · ")
    }
    // Under its family a look drops the family's words its name starts with: "Late Summer
    // Contrast" reads "Contrast" under "Series · Late summer".
    // A family's display name: declared in the catalogue (style.family), else from its folders.
    readonly property var familyNames: {
        const names = {}
        for (const style of root.styles) if (style.family) names[styleGroup(style.id)] = style.family
        return names
    }
    function familyName(id) { return familyNames[id] || groupName(id) }
    function shortName(style, section) {
        if (section === "top") return ""
        const last = familyName(section).split(" · ").pop().toLowerCase()
        const name = style.name
        return name.toLowerCase().startsWith(last + " ") && name.length > last.length + 1 ? name.slice(last.length + 1) : ""
    }
    function groupOpen(id) { return query !== "" || expandedStyleGroup === id }
    function toggleGroup(id) { expandedStyleGroup = expandedStyleGroup === id ? "" : id }
    function showDetails(id) {
        styleQuery = ""
        const family = styleGroup(id)
        if (family !== "") expandedStyleGroup = family
        details = (family === "" ? "top" : family) + "|" + id
    }
    function matches(style, q) {
        return !q || (style.name + " " + style.description + " " + familyName(styleGroup(style.id))).toLowerCase().indexOf(q) >= 0
    }
    // Looks matching the sidebar search (its dot on the Styles tab).
    function matchCount(q) { return q ? root.styles.filter(s => matches(s, q)).length : 0 }
    readonly property var visibleStyles: root.styles.filter(s => matches(s, query))
    function byName(a, b) {
        return a.id === "neutral/style.dtstyle" ? -1 : b.id === "neutral/style.dtstyle" ? 1 : a.name.localeCompare(b.name)
    }
    // The families [{ id, name, styles, collapsible }], independent of the favourites, so marking
    // one does not rebuild the grids (nor reload their thumbnails).
    readonly property var families: {
        const families = []
        for (const style of visibleStyles) {
            const key = styleGroup(style.id)
            if (key === "") continue
            let family = families.find(g => g.id === key)
            if (!family) families.push(family = { id: key, name: familyName(key), order: style.familyOrder || 0, styles: [], collapsible: true })
            family.styles.push(style)
        }
        function rank(id) {
            if (id === "my-styles") return 1
            if (id === "monochrome") return 2
            if (id === "experimental") return 4
            return 3
        }
        // Between Monochrome and Experimental: by the order a family declares, then by folder.
        families.sort((a, b) => rank(a.id) - rank(b.id) || a.order - b.order || a.id.localeCompare(b.id))
        for (const family of families) family.styles.sort(byName)
        return families
    }
    // The section above the families: the favourites, then the basic looks (Neutral …).
    readonly property var topSections: {
        const basics = visibleStyles.filter(s => styleGroup(s.id) === "").sort(byName)
        const favs = query ? [] : favourites.map(id => root.styles.find(s => s.id === id)).filter(s => !!s)
        const top = favs.concat(basics.filter(s => !isFavourite(s.id) || query))
        return top.length ? [{ id: "top", name: favs.length ? "★ Favourites" : "Basics", styles: top, collapsible: false,
                               favouriteCount: favs.length }] : []
    }
    Column {
        width: root.availableWidth; padding: 10
        Column {
            width: parent.width - 20; spacing: 10
            // Camera presets darktable applied automatically, before any style.
            Rectangle {
                objectName: "camera-presets"
                width: parent.width
                height: cameraBlock.implicitHeight + 14
                radius: 4
                color: root.theme.surface
                visible: root.photoReady && root.view === "camera"
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
                        objectName: "camera-presets-none"
                        text: (root.allCameraPresets || []).length > 0
                              ? "No Omalux camera preset matched this camera automatically. Choose one below."
                              : "No Omalux camera preset matches this camera."
                        color: root.theme.muted; font: root.theme.textFont; wrapMode: Text.WordWrap
                    }
                    Repeater {
                        model: root.cameraOpen ? root.cameraPresets : []
                        RowLayout {
                            required property var modelData
                            // The module still carries this preset (not merely is on).
                            readonly property bool applied: !!modelData.enabled
                            objectName: "camera-matched-" + modelData.module
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
            Loader {
                active: root.photoReady && root.view === "camera" && (root.allCameraPresets || []).length > 0
                visible: active
                width: parent.width
                sourceComponent: CameraPresetList {
                    objectName: "camera-preset-list"
                    theme: root.theme
                    presets: root.allCameraPresets
                    ready: !root.busy
                    onPresetRequested: (name, on) => root.cameraPresetRequested(name, on)
                }
            }
            Text {
                visible: !root.photoReady && root.view === "camera"
                width: parent.width
                text: "Open a photograph to see what was set up for its camera."
                color: root.theme.muted; font: root.theme.textFont; wrapMode: Text.WordWrap
            }
            RowLayout {
                width: parent.width
                spacing: 8
                visible: root.looksView
                TextField {
                    id: styleSearch
                    objectName: "styleSearch"
                    Layout.fillWidth: true
                    implicitHeight: 30
                    leftPadding: 10
                    placeholderText: "Filter " + root.styles.length + " looks"
                    color: root.theme.ink
                    placeholderTextColor: root.theme.muted; font: root.theme.textFont
                    selectByMouse: true
                    onTextChanged: root.styleQuery = text
                    Keys.onEscapePressed: { text = ""; focus = false }
                    NavTarget { id: searchNav; navId: "style-search"; kind: "search"; label: "filter looks"; input: styleSearch; onActivate: styleSearch.forceActiveFocus() }
                    background: Rectangle { color: root.theme.well; border.color: styleSearch.activeFocus || searchNav.current ? root.theme.accent : root.theme.line; radius: 5 }
                    Accessible.name: "Filter looks"
                }
                Button {
                    id: saveButton
                    objectName: "style-save"
                    text: "Save look…"
                    implicitHeight: 30
                    font: root.theme.textFont
                    enabled: root.photoReady && !root.busy
                    onClicked: { saveNav.claim(); root.saveRequested() }
                    highlighted: saveNav.current
                    ToolTip.visible: hovered
                    ToolTip.delay: 500
                    ToolTip.text: "Save the current edit as a look"
                    NavTarget { id: saveNav; navId: "save"; label: "save current look"; enabled: saveButton.enabled; onActivate: root.saveRequested() }
                }
            }
            AbstractButton {
                id: filmOffer
                objectName: "style-film-offer"
                visible: root.looksView && !!root.lookFilm
                width: parent.width
                height: 26
                padding: 0
                hoverEnabled: true
                enabled: root.photoReady && !root.busy
                onClicked: { filmOfferNav.claim(); root.cameraPresetRequested(root.lookFilm.name, true) }
                Accessible.name: root.lookFilm ? "Put the look's film on: " + root.lookFilm.title : ""
                NavTarget {
                    id: filmOfferNav
                    navId: "style-film-offer"
                    label: "with its film"
                    enabled: filmOffer.visible && filmOffer.enabled
                    activateLabel: "APPLY FILM"
                    onActivate: if (root.lookFilm) root.cameraPresetRequested(root.lookFilm.name, true)
                }
                background: Rectangle {
                    radius: 4
                    color: filmOffer.pressed ? root.theme.active : filmOffer.hovered ? root.theme.hover : "transparent"
                    border.width: filmOfferNav.current || filmOffer.visualFocus ? 1 : 0
                    border.color: root.theme.accent
                }
                contentItem: Text {
                    leftPadding: 4
                    verticalAlignment: Text.AlignVCenter
                    elide: Text.ElideRight
                    text: root.lookFilm ? "with its film: " + root.lookFilm.title + " ›" : ""
                    color: filmOffer.hovered || filmOfferNav.current ? root.theme.accent : root.theme.muted
                    font: root.theme.textFont
                }
            }
            Text {
                width: parent.width; visible: root.looksView && (!root.stylesReady || root.visibleStyles.length === 0)
                text: !root.stylesReady ? "Loading styles…" : root.styles.length === 0 ? "No styles in styles/." : "No look matches “" + root.query + "”."
                color: root.theme.muted; font: root.theme.textFont; wrapMode: Text.WordWrap
            }
            Repeater { model: root.looksView ? root.topSections : []; delegate: sectionDelegate }
            Repeater { model: root.looksView ? root.families : []; delegate: sectionDelegate }
            Component {
                id: sectionDelegate
                Column {
                    id: section
                    required property var modelData
                    readonly property bool open: !modelData.collapsible || root.groupOpen(modelData.id)
                    readonly property bool all: root.query !== "" || !!root.showAll[modelData.id]
                    readonly property var shown: !open ? [] : all ? modelData.styles : modelData.styles.slice(0, root.limit)
                    readonly property string detailsId: root.details.startsWith(modelData.id + "|") ? root.details.slice(modelData.id.length + 1) : ""
                    readonly property var detailsStyle: detailsId ? modelData.styles.find(s => s.id === detailsId) || null : null
                    width: parent.width
                    spacing: 6
                    // Family heading: a click opens it (and closes the open one).
                    ToolButton {
                        id: heading
                        objectName: "style-group-" + section.modelData.id
                        width: parent.width
                        height: 30
                        padding: 0
                        hoverEnabled: true
                        enabled: section.modelData.collapsible
                        onClicked: { groupNav.claim(); root.toggleGroup(section.modelData.id) }
                        NavTarget {
                            id: groupNav
                            navId: "group:" + section.modelData.id
                            label: section.modelData.name
                            kind: "group"
                            group: section.modelData.id
                            enabled: section.modelData.collapsible
                            listed: section.modelData.collapsible
                            activateLabel: section.open ? "COLLAPSE" : "EXPAND"
                            onActivate: root.toggleGroup(section.modelData.id)
                            onAdjust: steps => { if ((steps > 0) !== section.open) root.toggleGroup(section.modelData.id) }
                        }
                        Accessible.name: section.modelData.name
                        Accessible.description: section.open ? "Collapse group" : "Expand group"
                        background: Rectangle { color: "transparent"; border.color: heading.visualFocus || groupNav.current ? root.theme.accent : "transparent"; radius: 3 }
                        contentItem: RowLayout {
                            spacing: 6
                            Text {
                                visible: section.modelData.collapsible
                                Layout.preferredWidth: 12
                                text: section.open ? "▾" : "▸"
                                color: root.theme.muted; font: root.theme.textFont
                            }
                            Text {
                                Layout.fillWidth: true
                                text: section.modelData.name
                                color: heading.hovered || groupNav.current ? root.theme.ink
                                     : section.open ? root.theme.ink : root.theme.muted
                                font: root.theme.settingsFont
                            }
                            Text {
                                text: section.modelData.favouriteCount !== undefined ? section.modelData.favouriteCount || "" : section.modelData.styles.length
                                color: root.theme.muted; font: root.theme.textFont
                                opacity: .75
                            }
                        }
                    }
                    Grid {
                        id: grid
                        visible: section.open
                        width: parent.width
                        columns: 3
                        columnSpacing: 6; rowSpacing: 8
                        readonly property real tileWidth: Math.floor((width - 2 * columnSpacing) / 3)
                        Repeater {
                            model: section.shown
                            delegate: StyleTile {
                                required property var modelData
                                width: grid.tileWidth
                                theme: root.theme
                                style: modelData
                                shortName: root.shortName(modelData, section.modelData.id)
                                keyPrefix: section.modelData.id === "top" && root.isFavourite(modelData.id) && root.styleGroup(modelData.id) !== "" ? "favourite:" : ""
                                photoReady: root.photoReady
                                busy: root.busy
                                appliedStyle: root.appliedStyle
                                applyingStyle: root.applyingStyle
                                favourite: root.isFavourite(modelData.id)
                                detailsOpen: section.detailsId === modelData.id
                                onPreviewRequested: active => root.previewRequested(modelData.id, active)
                                onApplyRequested: root.applyRequested(modelData.id)
                                onFavouriteToggled: root.toggleFavourite(modelData.id)
                                onDetailsToggleRequested: root.toggleStyleDetails(section.modelData.id, modelData.id)
                                onExportRequested: root.exportRequested(modelData.id)
                                onDeleteRequested: root.deleteRequested(modelData.id, modelData.name)
                                navTarget.group: section.modelData.id
                            }
                        }
                    }
                    ToolButton {
                        id: moreButton
                        objectName: "style-show-all-" + section.modelData.id
                        visible: section.open && root.query === "" && section.modelData.styles.length > root.limit
                        anchors.right: parent.right
                        padding: 0
                        hoverEnabled: true
                        onClicked: { moreNav.claim(); root.toggleShowAll(section.modelData.id) }
                        NavTarget {
                            id: moreNav
                            navId: "show-all:" + section.modelData.id
                            label: "show all"
                            group: section.modelData.id
                            enabled: moreButton.visible
                            onActivate: root.toggleShowAll(section.modelData.id)
                        }
                        Accessible.name: section.all ? "Show fewer looks" : "Show all looks of " + section.modelData.name
                        contentItem: Text {
                            text: section.all ? "show fewer ‹" : "show all " + section.modelData.styles.length + " ›"
                            color: moreButton.hovered || moreNav.current ? root.theme.accent : root.theme.muted
                            font: root.theme.textFont
                        }
                        background: Item {}
                    }
                    Loader {
                        width: parent.width
                        active: !!section.detailsStyle && section.open
                        visible: active
                        sourceComponent: StyleDetails {
                            objectName: "style-details-" + section.detailsId
                            theme: root.theme
                            style: section.detailsStyle
                            onCloseRequested: root.details = ""
                        }
                    }
                }
            }
            Loader {
                active: !!root.catalogModel && root.view === "modules"
                visible: active
                width: parent.width
                sourceComponent: ModuleList {
                    theme: root.theme
                    groups: root.lookModules
                    catalogModel: root.catalogModel
                    states: root.states
                    overrides: root.overrides
                    editable: !root.busy && root.photoReady
                    // Only while the pane is on screen (search opens every match).
                    term: root.visible ? root.term : ""
                    activeControl: root.activeControl
                    settingsKey: "styles"
                    compact: true
                    onChangesRequested: (operation, instance, changes) => root.changesRequested(operation, instance, changes)
                    onEnableRequested: (operation, instance, enabled) => root.enableRequested(operation, instance, enabled)
                    onResetRequested: (operation, instance, module) => root.moduleResetRequested(operation, instance, module)
                    onInteractionChanged: active => root.moduleInteractionChanged(active)
                    onControlSelected: id => root.controlSelected(id)
                }
            }
            Text { width: parent.width; visible: root.looksView && root.errorMessage !== ""; text: root.errorMessage; color: "#f9d58b"; font: root.theme.textFont; wrapMode: Text.WordWrap }
        }
    }
}
