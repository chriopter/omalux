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
            if (list.count !== window.catalog.length) return window.fail("incomplete list")
            if (window.checked === window.catalog.length) {
                if (window.requests !== 1 || backend.selectedPresetId !== window.catalog[0].id)
                    return window.fail("preset selection was not delivered")
                console.log("All " + window.checked + " embedded previews loaded and scrolled; selection passed")
                verification.stop()
                if (window.capturePath.length > 0) {
                    list.positionViewAtBeginning()
                    capture.start()
                } else {
                    Qt.quit()
                }
                return
            }
            let entry = window.catalog[window.checked]
            list.positionViewAtIndex(window.checked, ListView.Beginning)
            let button = list.itemAtIndex(window.checked)
            if (!button) return
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
