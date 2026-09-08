import QtQuick
import org.omalux

Main {
    id: window
    property int phase: 0
    property string previousUrl: ""
    property string savedId: ""
    property real oldX: 0
    property real oldY: 0
    property string capturePath: CAPTURE_PATH
    function find(item, name) {
        if (item.objectName === name) return item
        for (const child of item.children || []) {
            const found = find(child, name)
            if (found) return found
        }
        return null
    }
    function fail(message) { phase = -1; console.error(message); Qt.exit(1) }
    Timer {
        interval: 100
        running: true
        repeat: true
        onTriggered: {
            const backend = window.presetBackend
            const viewport = window.find(window.contentItem, "photoViewport")
            if (window.phase === 0) {
                const tabs = window.find(window.contentItem, "toolTabs")
                const buttons = tabs.children.filter(child => child.label !== undefined)
                if (buttons.length !== 5 || buttons.some(button => button.y !== buttons[0].y || button.height < 20))
                    return window.fail("Tool tabs must fit in a single usable row")
                window.openPhoto(backend.urlForLocalPath(INPUT_PATH))
                window.phase = 1
            } else if (window.phase === 1 && backend.previewUrl.toString() && !backend.loading && viewport.imageWidth > 1) {
                window.setZoom(2)
                window.oldX = viewport.contentX
                window.oldY = viewport.contentY
                window.previousUrl = backend.previewUrl.toString()
                backend.setParameter("basics.exposure_ev", 0.4)
                window.phase = 2
            } else if (window.phase === 2 && !backend.loading && backend.previewUrl.toString() !== window.previousUrl) {
                if (window.zoom !== 2 || Math.abs(viewport.contentX - window.oldX) > 1 || Math.abs(viewport.contentY - window.oldY) > 1)
                    return window.fail("Editing moved the viewport")
                const geometry = JSON.parse(backend.settingsJson).geometry
                geometry.quarter_turns_clockwise = 1
                geometry.crop = { x: 0.125, y: 0.125, width: 0.75, height: 0.75 }
                backend.setGeometry(JSON.stringify(geometry))
                backend.savePreset("Workflow look", "new")
                window.phase = 3
            } else if (window.phase === 3 && !backend.savingPreset) {
                if (backend.presetError) return window.fail(backend.presetError)
                const personal = JSON.parse(backend.presetCatalogJson).presets.filter(p => p.user)
                if (personal.length !== 1) return window.fail("Saved preset missing")
                window.savedId = personal[0].id
                backend.setParameter("basics.exposure_ev", 0)
                backend.selectPreset(window.savedId)
                const settings = JSON.parse(backend.settingsJson)
                if (Math.abs(settings.basics.exposure_ev - 0.4) > 0.001 || settings.geometry.quarter_turns_clockwise !== 1)
                    return window.fail("Personal look did not preserve photo geometry")
                if (!backend.renamePreset(window.savedId, "Renamed look")) return window.fail("Rename failed")
                backend.savePreset("Renamed look", "new")
                window.phase = 4
            } else if (window.phase === 4 && !backend.savingPreset) {
                if (!backend.presetError) return window.fail("Duplicate name should require a decision")
                if (window.capturePath) {
                    window.find(window.contentItem, "editorSurface").grabToImage(result => result.saveToFile(window.capturePath))
                }
                backend.savePreset("Renamed look", "copy")
                window.phase = 5
            } else if (window.phase === 5 && !backend.savingPreset) {
                const personal = JSON.parse(backend.presetCatalogJson).presets.filter(p => p.user)
                if (personal.length !== 2) return window.fail("Save copy failed")
                backend.setParameter("basics.exposure_ev", 0.7)
                backend.savePreset("Renamed look", "replace")
                window.phase = 6
            } else if (window.phase === 6 && !backend.savingPreset) {
                const personal = JSON.parse(backend.presetCatalogJson).presets.filter(p => p.user)
                if (personal.length !== 2 || backend.presetError) return window.fail("Replace failed")
                backend.selectPreset(window.savedId)
                if (Math.abs(JSON.parse(backend.settingsJson).basics.exposure_ev - 0.7) > 0.001)
                    return window.fail("Updated look did not reload")
                backend.exportPreset(window.savedId, backend.urlForLocalPath(EXPORT_PATH))
                for (const preset of personal) {
                    if (!backend.deletePreset(preset.id)) return window.fail("Delete failed")
                }
                window.phase = 7
                console.log("Crop, rotation, stable edit viewport and personal preset lifecycle passed")
                Qt.exit(0)
            }
        }
    }
}
