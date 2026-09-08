import QtQuick
import QtQuick.Controls
import org.omalux 1.0
import "qrc:/qt/qml/org/omalux/qml/tools/presets"

ApplicationWindow {
    id: window
    width: 316
    height: 720
    visible: true
    color: "#151515"
    property int checked: 0
    property int requests: 0
    property bool selecting: false
    property int phase: 0
    property string capturePath: ""
    readonly property var catalog: JSON.parse(backend.presetCatalogJson).presets
    readonly property var testTheme: ({ inkColor: "#eeeeee", accentColor: "#e8b66d",
        mutedColor: "#bbbbbb", selectionColor: "#383024", surfaceColor: "#252525",
        lineColor: "#444444", monoFont: "monospace" })

    PhotoBackend { id: backend }
    PresetsPanel {
        id: panel
        anchors.fill: parent
        anchors.margins: 12
        theme: window.testTheme
        photoReady: false
        catalogJson: backend.presetCatalogJson
        selectedPresetId: backend.selectedPresetId
        onPresetRequested: id => {
            window.requests++
            backend.selectPreset(id)
        }
    }

    function find(item, name) {
        if (item.objectName === name) return item
        for (let child of item.children || []) {
            let found = find(child, name)
            if (found) return found
        }
        return null
    }
    function fail(message) {
        console.error(message)
        Qt.exit(1)
    }

    Timer {
        id: verification
        interval: 20
        running: true
        repeat: true
        onTriggered: {
            let list = window.find(panel, "presetList")
            if (!list || window.catalog.length === 0) return
            if (window.phase === 0) {
                if (panel.groups[0].id !== "monochrome" || !panel.expandedGroups.monochrome)
                    return window.fail("monochrome must start first and expanded")
                if (panel.groups[panel.groups.length - 1].id !== "experimental")
                    return window.fail("experimental must be the last group")
                let lateSummer = panel.groups.findIndex(group => group.id === "series/late-summer")
                let movie = panel.groups.findIndex(group => group.id === "series/movie")
                if (lateSummer < 0 || movie !== lateSummer + 1)
                    return window.fail("series groups must stay together")
                if (panel.groups.some(group => group.id === "basic")
                        || panel.rows[0].id !== "neutral" || !panel.rows[0].isDefault)
                    return window.fail("neutral must be the standalone first preset")
                if (list.count !== panel.groups.length + panel.groups[0].presets.length + 1)
                    return window.fail("only monochrome must start expanded")
                let monochrome = list.itemAtIndex(1)
                if (!monochrome) return
                if (!monochrome.enabled) return window.fail("groups must open without a photo")
                monochrome.clicked()
                window.phase = 1
                return
            }
            if (window.phase === 1) {
                if (list.count !== panel.groups.length + 1)
                    return window.fail("monochrome did not collapse")
                panel.toggleGroup(panel.groups[1].id)
                window.phase = 2
                return
            }
            if (window.phase === 2) {
                if (panel.expandedGroups.monochrome
                        || !panel.expandedGroups[panel.groups[1].id])
                    return window.fail("opening a group did not close the previous group")
                if (list.count !== panel.groups.length + 1
                        + panel.groups[1].presets.length)
                    return window.fail("accordion group shows the wrong presets")
                list.forceLayout()
                list.positionViewAtBeginning()
                window.phase = 3
            }
            if (window.phase === 4) {
                if (list.count !== panel.groups.length + 1) return window.fail("groups did not collapse")
                if (window.requests !== 1 || backend.selectedPresetId !== window.catalog[0].id)
                    return window.fail("group toggles changed selection")
                console.log("All " + window.checked + " previews loaded; group defaults, compact rows and selection passed")
                verification.stop()
                if (window.capturePath.length > 0) {
                    panel.toggleGroup(panel.groups[0].id)
                    list.positionViewAtBeginning()
                    capture.start()
                } else Qt.quit()
                return
            }
            if (window.checked === window.catalog.length) {
                if (window.requests !== 1 || backend.selectedPresetId !== window.catalog[0].id)
                    return window.fail("preset selection was not delivered")
                for (let group of panel.groups) {
                    if (panel.expandedGroups[group.id])
                        panel.toggleGroup(group.id)
                }
                window.phase = 4
                return
            }
            let entry = window.catalog[window.checked]
            if (entry.group !== "basic" && !panel.expandedGroups[entry.group]) {
                panel.toggleGroup(entry.group)
                return
            }
            let rowIndex = panel.rows.findIndex(row => !row.isGroup && row.id === entry.id)
            if (rowIndex < 0) return window.fail("preset missing from its group")
            list.positionViewAtIndex(rowIndex, ListView.Beginning)
            let button = list.itemAtIndex(rowIndex)
            if (!button) return
            if (button.height !== 72 || button.background !== null)
                return window.fail("preset rows must be compact and borderless")
            let preview = window.find(button, "presetPreview-" + entry.id)
            if (!preview) return window.fail("missing preview for " + entry.id)
            if (preview.status === Image.Error) return window.fail("unreadable resource: " + entry.previewUrl)
            if (preview.status !== Image.Ready) return
            if (Math.max(preview.sourceSize.width, preview.sourceSize.height) !== 288)
                return window.fail("unbounded preview dimensions")
            if (window.checked === 0 && !window.selecting) {
                if (button.enabled || window.requests !== 0 || backend.selectedPresetId !== "neutral")
                    return window.fail("browsing without a photo changed selection")
                panel.photoReady = true
                window.selecting = true
                return
            }
            if (window.checked === 0) {
                if (!button.enabled) return window.fail("selection stays disabled with a photo")
                button.clicked()
            }
            window.checked++
        }
    }
    Timer {
        id: capture
        interval: 300
        onTriggered: {
            panel.grabToImage(result => {
                if (!result.saveToFile(window.capturePath)) return window.fail("screenshot failed")
                Qt.quit()
            })
        }
    }
    Timer {
        interval: 30000
        running: true
        onTriggered: window.fail("preview test timed out")
    }
}
