import QtQuick
import QtQuick.Controls
import QtTest
import "../../ui/components"
import "../../ui/panels"

// The shell around the panes: toolbar, photo viewport, status bar, Info pane and the export
// dialog, each on its own with stand-in data.
//   XDG_CONFIG_HOME=$(mktemp -d) QT_QPA_PLATFORM=offscreen \
//     /usr/lib/qt6/bin/qmltestrunner -input omalux/tests/components/tst_shell.qml
Item {
    id: top
    width: 1000; height: 760

    EditorTheme { id: theme }
    property var log: []

    EditorToolbar {
        id: toolbar
        width: parent.width
        theme: theme
        logoSource: ""
        filename: "a-photograph-with-a-very-long-file-name-2026-10-09.dng"
        zoom: viewport.zoom
        minimumZoom: viewport.minimumZoom
        maximumZoom: viewport.maximumZoom
        onOpenRequested: top.log.push(["open"])
        onSaveRequested: top.log.push(["save"])
        onZoomRequested: factor => { top.log.push(["zoom", factor]); viewport.zoomBy(factor) }
        onFitRequested: { top.log.push(["fit"]); viewport.fit() }
    }
    PhotoViewport {
        id: viewport
        y: 48; width: 640; height: 480
        theme: theme
        preview: ""
        previewAspectRatio: 1.5
        textureTransform: Qt.vector4d(1, 1, 0, 0)
        status: "Loading image…"
        onOpenRequested: top.log.push(["open"])
    }
    EditorStatusBar {
        id: statusBar
        y: 540; width: 420
        theme: theme
        hints: "[↑/↓] SELECT   [←/→] exposure   [⏎] EXPAND   [R] RESET VALUE   [TAB] PANES   [?] HELP"
        status: "Ready · OpenCL auto · 12 ms"
    }
    MetadataPanel {
        id: info
        x: 660; y: 48; width: 330; height: 480
        theme: theme
        metadata: ({})
    }
    EditorDialogs {
        id: dialogs
        theme: theme
        photoPath: "/photos/2026/beach day.cr2"
    }

    TestCase {
        name: "shell"
        when: windowShown

        function find(item, name) {
            if (!item) return null
            if (item.objectName === name) return item
            const children = item.children
            for (let i = 0; i < children.length; ++i) {
                const found = find(children[i], name)
                if (found) return found
            }
            return null
        }
        function init() { top.log = []; viewport.fit() }

        function test_toolbar_states() {
            const zoomOut = find(toolbar, "toolbar-zoom-out"), zoomIn = find(toolbar, "toolbar-zoom-in")
            const fit = find(toolbar, "toolbar-fit"), exportButton = find(toolbar, "toolbar-export")
            compare(fit.text, "FIT")
            verify(!zoomOut.enabled, "nothing to zoom out of at fit")
            mouseClick(zoomOut)
            compare(top.log.length, 0, "a disabled button does nothing")
            mouseClick(zoomIn)
            compare(JSON.stringify(top.log), JSON.stringify([["zoom", 1.25]]))
            verify(zoomOut.enabled)
            compare(fit.text, "1.3×")
            for (let i = 0; i < 20; ++i) mouseClick(zoomIn)
            compare(viewport.zoom, viewport.maximumZoom)
            verify(!zoomIn.enabled, "nothing to zoom into at the limit")
            mouseClick(fit)
            compare(viewport.zoom, 1)
            // Hover raises the fill and keeps the ink: the accent marks the chosen tab and focus.
            mouseMove(exportButton, exportButton.width / 2, exportButton.height / 2)
            tryVerify(() => exportButton.hovered)
            verify(Qt.colorEqual(exportButton.contentItem.color, theme.ink), "hovered label stays ink")
            verify(Qt.colorEqual(exportButton.background.color, theme.hover))
            toolbar.photoReady = false
            verify(!exportButton.enabled, "nothing to export without a photograph")
            verify(find(toolbar, "toolbar-open").enabled)
            toolbar.photoReady = true
            // A click leaves no focus on the toolbar; the keys stay with the editor.
            mouseClick(exportButton)
            verify(!exportButton.activeFocus)
            const name = find(toolbar, "toolbar-filename")
            verify(name.truncated && name.x + name.width <= toolbar.width, "a long name is cut, not clipped")
        }

        function test_zoom_keeps_the_point() {
            const flick = find(viewport, "photo-flick"), photo = find(viewport, "photo-image")
            const under = (x, y) => [(flick.contentX + x - photo.x) / photo.width, (flick.contentY + y - photo.y) / photo.height]
            // Keys and toolbar: around the centre of the viewport.
            viewport.zoomBy(2)
            compare(viewport.zoom, 2)
            let now = under(viewport.width / 2, viewport.height / 2)
            fuzzyCompare(now[0], .5, .002); fuzzyCompare(now[1], .5, .002)
            viewport.zoomBy(2)
            now = under(viewport.width / 2, viewport.height / 2)
            fuzzyCompare(now[0], .5, .002); fuzzyCompare(now[1], .5, .002)
            // The wheel: around the pointer.
            const x = 420, y = 150
            const before = under(x, y)
            mouseWheel(viewport, x, y, 0, 120)
            verify(viewport.zoom > 4)
            now = under(x, y)
            fuzzyCompare(now[0], before[0], .003); fuzzyCompare(now[1], before[1], .003)
            mouseWheel(viewport, x, y, 0, -120)
            now = under(x, y)
            fuzzyCompare(now[0], before[0], .003); fuzzyCompare(now[1], before[1], .003)
            // Never past the photograph, never below fit.
            for (let i = 0; i < 40; ++i) mouseWheel(viewport, 5, 5, 0, -120)
            compare(viewport.zoom, 1)
            compare(flick.contentX, 0); compare(flick.contentY, 0)
        }

        function test_empty_state() {
            const empty = find(viewport, "photo-empty"), open = find(viewport, "photo-empty-open")
            verify(empty.visible, "no preview yet")
            compare(find(viewport, "photo-empty-status").text, "Loading image…")
            verify(!open.visible, "no way out is offered while the engine starts")
            viewport.photoMissing = true
            verify(open.visible)
            tryVerify(() => open.width > 0 && open.y > 0)
            mouseClick(open)
            compare(JSON.stringify(top.log), JSON.stringify([["open"]]))
            viewport.photoMissing = false
        }

        function test_status_never_leaves_the_bar() {
            const text = find(statusBar, "statusText"), hints = find(statusBar, "keyHints")
            verify(!text.truncated)
            statusBar.status = "Saved /home/someone/Pictures/2026/10/a rather long folder name/another/beach-volleyball-omalux.jpg"
            verify(text.truncated, "a long path is cut in the middle")
            verify(text.x + text.width <= statusBar.width, "and stays inside the window")
            verify(hints.x + hints.width <= text.x, "without running into the hints")
            statusBar.status = "Ready"
        }

        function test_info_formats() {
            compare(info.exposureText(1 / 640), "1/640 s")
            compare(info.exposureText(0.0015625000232830644), "1/640 s")
            compare(info.exposureText(0.5), "1/2 s")
            compare(info.exposureText(2), "2 s")
            compare(info.exposureText(2.5), "2.5 s")
            compare(info.exposureText(0), "")
            compare(info.sizeText(24646050), "23.5 MB")
            compare(info.sizeText(900), "900 B")
            const raw = info.groups({ path: "/photos/2026/R01.cr2", bytes: 24646050, raw: true, width: 5568, height: 3708,
                                      camera: "Canon EOS 70D", lens: "Canon EF-S 18-135mm f/3.5-5.6 IS STM", ISO: 100,
                                      aperture: 5.599999904632568, "exposure seconds": 0.0015625000232830644,
                                      "focal length mm": 24, "crop factor": 1.6, datetime: "2017:01:07 11:00:17",
                                      "exposure bias": 0, "exposure bias known": true, latitude: 0, longitude: 0 })
            const rows = {}
            for (const group of raw) for (const row of group.rows) rows[row[0]] = row[1]
            compare(raw.map(g => g.title).join(","), "File,Capture", "0° N 0° E is no location")
            compare(rows.name, "R01.cr2"); compare(rows.folder, "/photos/2026"); compare(rows.type, "CR2 · raw")
            compare(rows.pixels, "5568 × 3708 · 20.6 MP")
            compare(rows.date, "2017-01-07 11:00:17")
            compare(rows.exposure, "1/640 s"); compare(rows.aperture, "f/5.6"); compare(rows.ISO, "100")
            compare(rows["focal length"], "24 mm · 38 mm at 35 mm")
            compare(rows["exposure bias"], "0.0 EV")
            // A file without camera data: only what the file itself says, and a note.
            info.metadata = { path: "/photos/beach.jpg", bytes: 2048, width: 1536, height: 1024, camera: "(null) (null)",
                              lens: "", ISO: 0, aperture: 0, "exposure seconds": 0, "focal length mm": 0 }
            compare(info.shown.map(g => g.title).join(","), "File")
            verify(find(info, "info-no-capture").visible)
            verify(find(info, "info-pixels") !== null)
            compare(find(info, "info-pixels").value, "1536 × 1024 · 1.6 MP")
            info.metadata = { path: "/photos/here.dng", latitude: 51.9, longitude: -4.47, elevation: 12, ISO: 200 }
            compare(info.shown.map(g => g.title).join(","), "File,Capture,Location")
            compare(find(info, "info-longitude").value, "4.47000° W")
            info.metadata = {}
            compare(info.shown.length, 0)
        }

        function test_export_options() {
            dialogs.exportImage()
            const options = find(top.Window.window.contentItem, "export-options") || dialogs.children[0]
            tryVerify(() => dialogs.busy)
            const jpeg = find(Overlay.overlay, "export-format-jpg"), png = find(Overlay.overlay, "export-format-png")
            const quality = find(Overlay.overlay, "export-quality")
            verify(jpeg && png && quality, "the dialog offers both formats and the quality")
            verify(jpeg.checked && quality.enabled)
            keyClick(Qt.Key_P)
            verify(png.checked && !jpeg.checked, "P picks PNG")
            verify(!quality.enabled, "PNG has no quality")
            compare(find(Overlay.overlay, "export-quality-value").text, "—")
            mouseClick(jpeg)
            verify(jpeg.checked && quality.enabled, "one click picks JPEG")
            keyClick(Qt.Key_Escape)
            tryVerify(() => !dialogs.busy, 1000, "Escape closes the dialog")
            compare(dialogs.photoFolder, "/photos/2026")
            compare(dialogs.photoBase, "beach day")
            compare(dialogs.fileUrl("/photos/2026/beach day.cr2"), "file:///photos/2026/beach%20day.cr2")
        }
    }
}
