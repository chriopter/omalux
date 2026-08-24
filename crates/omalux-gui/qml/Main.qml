pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls
import QtCore
import QtQuick.Dialogs
import QtQuick.Layouts
import QtQuick.Window
import io.omacom.omalux 1.0

ApplicationWindow {
    id: window
    width: 1280
    height: 820
    minimumWidth: 800
    minimumHeight: 560
    // Headless mode still uses Qt's offscreen platform for the QML event loop.
    visible: true
    title: backend.fileName.length > 0 ? backend.fileName + " — Omalux" : "Omalux"
    color: pageColor

    readonly property color pageColor: backend.themeBackground
    readonly property color inkColor: backend.themeForeground
    readonly property color accentColor: backend.themeAccent
    readonly property color selectionColor: backend.themeSelection
    readonly property color surfaceColor: mixColors(pageColor, inkColor, 0.035)
    readonly property color raisedColor: mixColors(pageColor, inkColor, 0.08)
    readonly property color mutedColor: mixColors(pageColor, inkColor, 0.55)
    readonly property color lineColor: mixColors(pageColor, inkColor, 0.18)
    readonly property string monoFont: "iA Writer Mono S"
    readonly property real sidebarWidth: 316

    property real zoom: 1.0
    property int selectedPanel: 0
    property int selectedParameter: 0
    property bool grainAdvancedExpanded: false
    property bool shortcutsVisible: false
    property bool exportMenuVisible: false
    property string pendingExportFormat: ""
    property int exportQuality: 90
    property bool photoFullscreen: false
    property int visibilityBeforePhotoFullscreen: Window.Windowed
    readonly property var commandLineArguments: Qt.application.arguments
    readonly property bool cliHeadless:
        commandLineArguments.indexOf("--headless") >= 0
    readonly property string cliInput: argumentValue("--input", "")
    readonly property string cliOutput: argumentValue("--output", "")
    readonly property string cliRequestedFormat:
        argumentValue("--format", inferredCliFormat(cliOutput)).toUpperCase()
    property bool cliExportStarted: false

    function mixColors(base, tint, amount) {
        return Qt.rgba(base.r + (tint.r - base.r) * amount,
                       base.g + (tint.g - base.g) * amount,
                       base.b + (tint.b - base.b) * amount,
                       1.0)
    }

    function argumentValue(name, fallback) {
        var position = commandLineArguments.indexOf(name)
        return position >= 0 && position + 1 < commandLineArguments.length
            ? commandLineArguments[position + 1] : fallback
    }

    function numericArgument(name, fallback) {
        var value = Number(argumentValue(name, fallback.toString()))
        return isFinite(value) ? value : fallback
    }

    function inferredCliFormat(path) {
        var suffix = path.substring(path.lastIndexOf(".") + 1).toUpperCase()
        if (suffix === "JPG" || suffix === "JPEG")
            return "JPEG"
        if (suffix === "HEIC" || suffix === "HEIF")
            return "HEIC"
        return "ORIGINAL"
    }

    function localFileUrl(path) {
        return backend.urlForLocalPath(path)
    }

    function normalizedCliFormat() {
        if (cliRequestedFormat === "JPG")
            return "JPEG"
        if (cliRequestedFormat === "HEIF")
            return "HEIC"
        return cliRequestedFormat
    }

    function failCli(message) {
        console.error("omalux: " + message)
        Qt.callLater(function() { Qt.exit(1) })
    }

    function startCli() {
        if (cliInput.length === 0) {
            if (cliHeadless)
                failCli("--headless requires --input")
            return
        }
        if (cliHeadless && cliOutput.length === 0) {
            failCli("--headless requires --output")
            return
        }
        var format = normalizedCliFormat()
        if (["ORIGINAL", "JPEG", "HEIC"].indexOf(format) < 0) {
            failCli("unsupported --format: " + cliRequestedFormat)
            return
        }

        exportQuality = Math.max(1, Math.min(100,
            Math.round(numericArgument("--quality", 90))))
        grainPanel.grainValue = Math.max(0, Math.min(100,
            numericArgument("--grain", 24)))
        grainPanel.grainSizeValue = Math.max(20, Math.min(6400,
            numericArgument("--grain-size", 4000)))
        grainPanel.midtonesValue = Math.max(0, Math.min(100,
            numericArgument("--midtones", 100)))
        backend.setParameter("effects.grain.amount", grainPanel.grainValue)
        backend.setParameter("effects.grain.size_iso", grainPanel.grainSizeValue)
        backend.setParameter("effects.grain.midtone_response", grainPanel.midtonesValue)

        console.log("omalux: opening " + cliInput)
        backend.openPhoto(localFileUrl(cliInput))
    }

    function continueCliExport() {
        if (cliOutput.length === 0 || cliExportStarted
                || sourceImage.status !== Image.Ready)
            return
        cliExportStarted = true
        var format = normalizedCliFormat()
        var destination = localFileUrl(cliOutput)
        console.log("omalux: exporting " + format + " at "
                    + exportQuality + "% to " + cliOutput)
        if (format === "ORIGINAL")
            backend.saveOriginal(destination)
        else
            backend.exportPhoto(destination, format, exportQuality)
    }

    Component.onCompleted: {
        backend.startThemeWatcher()
        startCli()
    }

    function enterPhotoFullscreen() {
        if (sourceImage.status !== Image.Ready || photoFullscreen)
            return
        visibilityBeforePhotoFullscreen = window.visibility
        zoom = 1.0
        photoFlick.contentX = 0
        photoFlick.contentY = 0
        photoFullscreen = true
        window.showFullScreen()
    }

    function exitPhotoFullscreen() {
        if (!photoFullscreen)
            return
        photoFullscreen = false
        if (visibilityBeforePhotoFullscreen === Window.Maximized)
            window.showMaximized()
        else if (visibilityBeforePhotoFullscreen === Window.FullScreen)
            window.showFullScreen()
        else
            window.showNormal()
        zoom = 1.0
        photoFlick.contentX = 0
        photoFlick.contentY = 0
        keyboardLayer.forceActiveFocus()
    }

    function setZoomAt(value, focalX, focalY) {
        var nextZoom = Math.max(0.25, Math.min(8.0, value))
        if (Math.abs(nextZoom - zoom) < 0.001)
            return

        var oldZoom = zoom
        var imageX = photoFlick.contentX + focalX - photoSurface.x
        var imageY = photoFlick.contentY + focalY - photoSurface.y
        zoom = nextZoom

        Qt.callLater(function() {
            var ratio = nextZoom / oldZoom
            photoFlick.contentX = photoSurface.x + imageX * ratio - focalX
            photoFlick.contentY = photoSurface.y + imageY * ratio - focalY
            photoFlick.returnToBounds()
        })
    }

    function setZoom(value) {
        setZoomAt(value, photoFlick.width / 2, photoFlick.height / 2)
    }

    function zoomIn() {
        setZoom(zoom * 1.25)
    }

    function zoomOut() {
        setZoom(zoom / 1.25)
    }

    function fitPhoto() {
        zoom = 1.0
        Qt.callLater(function() {
            photoFlick.contentX = 0
            photoFlick.contentY = 0
            photoFlick.returnToBounds()
        })
    }

    function stableSeed(text) {
        var hash = 5381
        for (var index = text.length - 1; index >= 0; --index)
            hash = ((hash * 33) ^ text.charCodeAt(index)) >>> 0
        return (hash % 997) + 1
    }

    function parameterAt(index) {
        return grainPanel.parameterAt(index)
    }

    function selectParameter(index) {
        if (index === 19 || index === 20)
            grainAdvancedExpanded = true
        selectedParameter = (index + grainPanel.parameterCount)
            % grainPanel.parameterCount
        Qt.callLater(function() {
            var control = parameterAt(selectedParameter)
            grainPanel.ensureVisible(control)
        })
    }

    function moveParameter(direction) {
        var order = grainPanel.navigationOrder()
        var position = order.indexOf(selectedParameter)
        if (position < 0)
            position = 0
        selectParameter(order[(position + direction + order.length) % order.length])
    }

    function adjustParameter(direction, coarse) {
        parameterAt(selectedParameter).nudge(direction, coarse)
    }

    function resetParameter() {
        parameterAt(selectedParameter).resetValue()
    }

    function toggleGrainAdvanced() {
        grainAdvancedExpanded = !grainAdvancedExpanded
        if (!grainAdvancedExpanded)
            selectedParameter = 0
    }

    function moveActiveParameter(direction) {
        if (selectedPanel === 0)
            moveParameter(direction)
    }

    function adjustActiveParameter(direction, coarse) {
        if (selectedPanel === 0)
            adjustParameter(direction, coarse)
    }

    function resetActiveParameter() {
        if (selectedPanel === 0)
            resetParameter()
    }

    function selectPanel(index) {
        selectedPanel = (index + 3) % 3
    }

    function movePanel(direction) {
        selectPanel(selectedPanel + direction)
    }

    function exportBaseName() {
        var name = backend.fileName.length > 0 ? backend.fileName : "photograph"
        var dot = name.lastIndexOf(".")
        return dot > 0 ? name.substring(0, dot) : name
    }

    function exportResolution() {
        var width = Math.round(sourceImage.sourceSize.width)
        var height = Math.round(sourceImage.sourceSize.height)
        return width > 0 && height > 0 ? width + " × " + height : "—"
    }

    function humanFileSize(bytes) {
        if (bytes < 1024)
            return Math.round(bytes) + " B"
        if (bytes < 1024 * 1024)
            return (bytes / 1024).toFixed(1) + " KB"
        return (bytes / (1024 * 1024)).toFixed(1) + " MB"
    }

    function estimatedExportSize(format) {
        if (format === "ORIGINAL")
            return backend.originalFileSize
        var pixels = sourceImage.sourceSize.width * sourceImage.sourceSize.height
        if (pixels < 1)
            return "—"
        var qualityScale = 0.35 + Math.pow(exportQuality / 100, 2) * 0.8
        var bytesPerPixel = format === "JPEG" ? 0.42 : 0.24
        var grainPenalty = 1 + grainPanel.grainValue / 100 * 0.35
        return "~" + humanFileSize(
            pixels * bytesPerPixel * qualityScale * grainPenalty)
    }

    function exportDetails(format) {
        return exportResolution() + "  ·  " + estimatedExportSize(format)
    }

    function chooseExportFormat(format) {
        exportMenuVisible = false
        pendingExportFormat = format

        if (format === "ORIGINAL") {
            var originalSuffix = backend.originalFormat.toLowerCase()
            exportDialog.title = "Save original " + backend.originalFormat
            exportDialog.defaultSuffix = originalSuffix
            exportDialog.nameFilters = [backend.originalFormat + " (*." + originalSuffix + ")"]
            exportDialog.selectedFile = exportDialog.currentFolder + "/"
                    + exportBaseName() + "-copy." + originalSuffix
        } else if (format === "JPEG") {
            exportDialog.title = "Export JPEG"
            exportDialog.defaultSuffix = "jpg"
            exportDialog.nameFilters = ["JPEG image (*.jpg *.jpeg)"]
            exportDialog.selectedFile = exportDialog.currentFolder + "/"
                    + exportBaseName() + "-omalux.jpg"
        } else {
            exportDialog.title = "Export HEIC"
            exportDialog.defaultSuffix = "heic"
            exportDialog.nameFilters = ["HEIC image (*.heic)"]
            exportDialog.selectedFile = exportDialog.currentFolder + "/"
                    + exportBaseName() + "-omalux.heic"
        }
        exportDialog.open()
    }

    PhotoBackend {
        id: backend
    }

    FileDialog {
        id: openDialog
        title: "Open photograph"
        fileMode: FileDialog.OpenFile
        nameFilters: [
            "Photographs (*.jpg *.jpeg *.png *.bmp *.dng *.cr2 *.cr3 *.nef *.nrw *.arw *.raf *.rw2 *.orf *.pef)",
            "All files (*)"
        ]
        onAccepted: backend.openPhoto(selectedFile)
    }

    FileDialog {
        id: exportDialog
        fileMode: FileDialog.SaveFile
        acceptLabel: "Save"
        currentFolder: StandardPaths.standardLocations(StandardPaths.PicturesLocation)[0]
        onAccepted: {
            if (window.pendingExportFormat === "ORIGINAL")
                backend.saveOriginal(selectedFile)
            else
                backend.exportPhoto(selectedFile, window.pendingExportFormat,
                                    window.exportQuality)
        }
    }

    Shortcut {
        sequences: [StandardKey.Open]
        onActivated: openDialog.open()
    }

    Shortcut {
        sequences: [StandardKey.Save]
        enabled: sourceImage.status === Image.Ready
        onActivated: window.exportMenuVisible = true
    }

    Shortcut {
        sequences: ["Ctrl++", "Ctrl+="]
        enabled: sourceImage.status === Image.Ready
        onActivated: window.zoomIn()
    }

    Shortcut {
        sequence: "Ctrl+-"
        enabled: sourceImage.status === Image.Ready
        onActivated: window.zoomOut()
    }

    Shortcut {
        sequence: "Ctrl+0"
        enabled: sourceImage.status === Image.Ready
        onActivated: window.fitPhoto()
    }

    Shortcut {
        sequences: ["Ctrl+?", "F1"]
        onActivated: window.shortcutsVisible = !window.shortcutsVisible
    }

    Connections {
        target: backend
        function onPreviewUrlChanged() { window.fitPhoto() }
        function onStatusChanged() {
            if (window.cliInput.length === 0)
                return
            var status = backend.status
            console.log("omalux: " + status)
            if (window.cliExportStarted
                    && status.indexOf("Published with durability warning:") === 0) {
                console.warn("omalux: export complete with durability warning")
                Qt.exit(0)
            } else if (window.cliExportStarted && status.indexOf("Saved ") === 0) {
                console.log("omalux: export complete")
                Qt.exit(0)
            } else if (status.indexOf("Could not") === 0
                       || status.indexOf("Development failed") === 0
                       || status.indexOf("Unsupported") === 0
                       || status.indexOf("Only local") === 0
                       || status.indexOf("Open a photograph") === 0
                       || status.indexOf("does not exist") >= 0) {
                window.failCli(status)
            }
        }
    }

    Timer {
        interval: 180000
        running: window.cliHeadless
        repeat: false
        onTriggered: window.failCli("headless operation timed out")
    }

    Item {
        id: keyboardLayer
        anchors.fill: parent
        focus: true
        Keys.priority: Keys.BeforeItem

        Component.onCompleted: forceActiveFocus()

        Keys.onPressed: function(event) {
            if (window.photoFullscreen) {
                window.exitPhotoFullscreen()
                event.accepted = true
                return
            }

            if (window.exportMenuVisible) {
                if (event.key === Qt.Key_Escape) {
                    window.exportMenuVisible = false
                    event.accepted = true
                } else if (event.key === Qt.Key_1) {
                    window.chooseExportFormat("ORIGINAL")
                    event.accepted = true
                } else if (event.key === Qt.Key_2) {
                    window.chooseExportFormat("JPEG")
                    event.accepted = true
                } else if (event.key === Qt.Key_3) {
                    window.chooseExportFormat("HEIC")
                    event.accepted = true
                } else if (event.key === Qt.Key_Left) {
                    window.exportQuality = Math.max(
                        1, window.exportQuality
                           - ((event.modifiers & Qt.ShiftModifier) ? 5 : 1))
                    event.accepted = true
                } else if (event.key === Qt.Key_Right) {
                    window.exportQuality = Math.min(
                        100, window.exportQuality
                             + ((event.modifiers & Qt.ShiftModifier) ? 5 : 1))
                    event.accepted = true
                }
                return
            }

            if (event.modifiers & (Qt.ControlModifier | Qt.AltModifier | Qt.MetaModifier))
                return

            if (window.shortcutsVisible) {
                if (event.key === Qt.Key_Escape || event.text === "?") {
                    window.shortcutsVisible = false
                    event.accepted = true
                }
                return
            }

            var coarse = (event.modifiers & Qt.ShiftModifier) !== 0
            var keyText = event.text ? event.text.toLowerCase() : ""
            if (event.key === Qt.Key_Tab || event.key === Qt.Key_Backtab) {
                window.movePanel(coarse || event.key === Qt.Key_Backtab ? -1 : 1)
            } else if (keyText === "1") {
                window.selectPanel(0)
            } else if (keyText === "2") {
                window.selectPanel(1)
            } else if (keyText === "3") {
                window.selectPanel(2)
            } else if (keyText === "[") {
                window.movePanel(-1)
            } else if (keyText === "]") {
                window.movePanel(1)
            } else if (window.selectedPanel === 0
                       && (event.key === Qt.Key_Down || keyText === "j")) {
                window.moveActiveParameter(1)
            } else if (window.selectedPanel === 0
                       && (event.key === Qt.Key_Up || keyText === "k")) {
                window.moveActiveParameter(-1)
            } else if (window.selectedPanel === 0
                       && (event.key === Qt.Key_Left || keyText === "h")) {
                window.adjustActiveParameter(-1, coarse)
            } else if (window.selectedPanel === 0
                       && (event.key === Qt.Key_Right || keyText === "l")) {
                window.adjustActiveParameter(1, coarse)
            } else if (keyText === "g") {
                window.selectPanel(0)
                window.selectParameter(18)
            } else if (keyText === "s") {
                window.selectPanel(0)
                window.selectParameter(19)
            } else if (keyText === "m") {
                window.selectPanel(0)
                window.selectParameter(20)
            } else if (keyText === "a" && window.selectedPanel === 0) {
                window.toggleGrainAdvanced()
            } else if (keyText === "r" && window.selectedPanel === 0) {
                window.resetActiveParameter()
            } else if (keyText === "o") {
                openDialog.open()
            } else if (keyText === "0") {
                window.fitPhoto()
            } else if (keyText === "f") {
                window.enterPhotoFullscreen()
            } else if (keyText === "+" || keyText === "=") {
                window.zoomIn()
            } else if (keyText === "-") {
                window.zoomOut()
            } else if (keyText === "?") {
                window.shortcutsVisible = true
            } else {
                return
            }
            event.accepted = true
        }

        ColumnLayout {
            anchors.fill: parent
            spacing: 0

            Rectangle {
                Layout.fillWidth: true
                Layout.preferredHeight: window.photoFullscreen ? 0 : 48
                visible: !window.photoFullscreen
                color: window.pageColor

                Rectangle {
                    anchors.left: parent.left
                    anchors.right: parent.right
                    anchors.bottom: parent.bottom
                    height: 1
                    color: window.lineColor
                }

                Row {
                    id: headerActions
                    anchors.horizontalCenter: parent.horizontalCenter
                    anchors.verticalCenter: parent.verticalCenter
                    spacing: 9

                    TuiButton {
                        theme: window
                        text: "[O] OPEN"
                        primary: true
                        onClicked: openDialog.open()
                    }

                    TuiButton {
                        theme: window
                        text: "[S] SAVE"
                        enabled: sourceImage.status === Image.Ready
                        onClicked: window.exportMenuVisible = true
                    }

                    Row {
                        height: 30
                        spacing: 8

                        Text {
                            anchors.verticalCenter: parent.verticalCenter
                            text: "ZOOM"
                            color: window.mutedColor
                            font.family: window.monoFont
                            font.pixelSize: 10
                            font.bold: true
                        }

                        Slider {
                            id: zoomSlider
                            anchors.verticalCenter: parent.verticalCenter
                            width: 118
                            height: 20
                            from: -2
                            to: 3
                            value: Math.log(window.zoom) / Math.log(2)
                            stepSize: 0.05
                            enabled: sourceImage.status === Image.Ready
                            activeFocusOnTab: false
                            onMoved: window.setZoom(Math.pow(2, value))

                            background: Rectangle {
                                x: zoomSlider.leftPadding
                                y: zoomSlider.topPadding
                                   + zoomSlider.availableHeight / 2 - height / 2
                                width: zoomSlider.availableWidth
                                height: 2
                                color: window.lineColor

                                Rectangle {
                                    width: zoomSlider.visualPosition * parent.width
                                    height: parent.height
                                    color: window.mutedColor
                                }
                            }

                            handle: Rectangle {
                                x: zoomSlider.leftPadding
                                   + zoomSlider.visualPosition
                                   * (zoomSlider.availableWidth - width)
                                y: zoomSlider.topPadding
                                   + zoomSlider.availableHeight / 2 - height / 2
                                width: 8
                                height: 8
                                color: window.mutedColor
                            }
                        }

                        Text {
                            anchors.verticalCenter: parent.verticalCenter
                            width: 38
                            text: Math.round(window.zoom * 100) + "%"
                            color: window.inkColor
                            font.family: window.monoFont
                            font.pixelSize: 11
                            horizontalAlignment: Text.AlignRight
                        }
                    }
                }

            }

            RowLayout {
                Layout.fillWidth: true
                Layout.fillHeight: true
                spacing: 0

                Item {
                    id: viewport
                    Layout.fillWidth: true
                    Layout.fillHeight: true

                    Rectangle {
                        anchors.fill: parent
                        color: "#0b0b0b"
                    }

                    Column {
                        anchors.centerIn: parent
                        visible: backend.previewUrl.toString().length === 0 && !backend.loading
                        spacing: 10

                        Text {
                            anchors.horizontalCenter: parent.horizontalCenter
                            text: "NO PHOTOGRAPH LOADED"
                            color: window.inkColor
                            font.family: window.monoFont
                            font.pixelSize: 14
                            font.bold: true
                        }

                        Text {
                            anchors.horizontalCenter: parent.horizontalCenter
                            text: "JPEG / PNG / CAMERA RAW"
                            color: window.mutedColor
                            font.family: window.monoFont
                            font.pixelSize: 11
                        }

                        TuiButton {
                            theme: window
                            anchors.horizontalCenter: parent.horizontalCenter
                            text: "[ O ]  OPEN PHOTOGRAPH"
                            primary: true
                            onClicked: openDialog.open()
                        }
                    }

                    Flickable {
                        id: photoFlick
                        anchors.fill: parent
                        anchors.margins: window.photoFullscreen ? 0 : 20
                        clip: true
                        boundsBehavior: Flickable.StopAtBounds
                        contentWidth: Math.max(width, photoSurface.width)
                        contentHeight: Math.max(height, photoSurface.height)
                        interactive: window.zoom > 1.0

                        readonly property real fitScale: sourceImage.sourceSize.width > 0
                            && sourceImage.sourceSize.height > 0
                            ? Math.min(width / sourceImage.sourceSize.width,
                                       height / sourceImage.sourceSize.height)
                            : 1.0

                        Item {
                            id: photoSurface
                            width: Math.max(1, sourceImage.sourceSize.width * photoFlick.fitScale * window.zoom)
                            height: Math.max(1, sourceImage.sourceSize.height * photoFlick.fitScale * window.zoom)
                            x: photoFlick.contentWidth > width ? (photoFlick.contentWidth - width) / 2 : 0
                            y: photoFlick.contentHeight > height ? (photoFlick.contentHeight - height) / 2 : 0

                            Image {
                                id: sourceImage
                                anchors.fill: parent
                                source: backend.previewUrl
                                autoTransform: true
                                asynchronous: true
                                cache: false
                                visible: status === Image.Ready
                                onStatusChanged: {
                                    if (status === Image.Ready)
                                        window.continueCliExport()
                                    else if (status === Image.Error
                                             && window.cliInput.length > 0)
                                        window.failCli("Qt could not load the developed image")
                                }
                            }

                            Rectangle {
                                anchors.fill: parent
                                color: "transparent"
                                border.width: 1
                                border.color: window.lineColor
                                visible: sourceImage.status === Image.Ready
                                    && !window.photoFullscreen
                            }
                        }

                        WheelHandler {
                            target: null
                            enabled: sourceImage.status === Image.Ready
                            acceptedDevices: PointerDevice.Mouse
                            onWheel: function(event) {
                                var delta = event.angleDelta.y !== 0
                                        ? event.angleDelta.y
                                        : event.pixelDelta.y * 3
                                if (delta !== 0) {
                                    var factor = Math.pow(1.0018, delta)
                                    window.setZoomAt(window.zoom * factor,
                                                     point.position.x,
                                                     point.position.y)
                                }
                                event.accepted = true
                            }
                        }

                        PinchHandler {
                            target: null
                            enabled: sourceImage.status === Image.Ready
                            acceptedDevices: PointerDevice.TouchPad | PointerDevice.TouchScreen
                            minimumPointCount: 2
                            maximumPointCount: 2
                            rotationAxis.enabled: false

                            onScaleChanged: function(delta) {
                                window.setZoomAt(window.zoom * delta,
                                                 centroid.position.x,
                                                 centroid.position.y)
                            }

                            onTranslationChanged: function(delta) {
                                photoFlick.contentX -= delta.x
                                photoFlick.contentY -= delta.y
                            }

                            onActiveChanged: if (!active)
                                photoFlick.returnToBounds()
                        }
                    }
                }

                Rectangle {
                    Layout.preferredWidth: window.photoFullscreen
                        ? 0 : window.sidebarWidth
                    Layout.fillHeight: true
                    visible: !window.photoFullscreen
                    color: window.pageColor

                    Rectangle {
                        anchors.left: parent.left
                        width: 1
                        height: parent.height
                        color: window.lineColor
                    }

                    ColumnLayout {
                        anchors.fill: parent
                        anchors.margins: 12
                        spacing: 12

                        Rectangle {
                            Layout.fillWidth: true
                            Layout.preferredHeight: 69
                            color: window.lineColor
                            border.width: 1
                            border.color: window.lineColor

                            GridLayout {
                                anchors.fill: parent
                                anchors.margins: 1
                                columns: 3
                                rows: 1
                                rowSpacing: 1
                                columnSpacing: 1

                                ToolTabButton {
                                    theme: window
                                    Layout.fillWidth: true
                                    Layout.fillHeight: true
                                    iconSource: "qrc:/icons/edit.svg"
                                    label: "Edit"
                                    selected: window.selectedPanel === 0
                                    onClicked: window.selectPanel(0)
                                    Accessible.name: "Edit · 1"
                                }

                                ToolTabButton {
                                    theme: window
                                    Layout.fillWidth: true
                                    Layout.fillHeight: true
                                    iconSource: "qrc:/icons/presets.svg"
                                    label: "Presets"
                                    selected: window.selectedPanel === 1
                                    onClicked: window.selectPanel(1)
                                    Accessible.name: "Presets · 2"
                                }

                                ToolTabButton {
                                    theme: window
                                    Layout.fillWidth: true
                                    Layout.fillHeight: true
                                    iconSource: "qrc:/icons/info.svg"
                                    label: "Meta"
                                    selected: window.selectedPanel === 2
                                    onClicked: window.selectPanel(2)
                                    Accessible.name: "Metadata · 3"
                                }
                            }
                        }

                        Rectangle {
                            Layout.fillWidth: true
                            Layout.preferredHeight: 1
                            color: window.lineColor
                        }

                        StackLayout {
                            Layout.fillWidth: true
                            Layout.fillHeight: true
                            currentIndex: window.selectedPanel

                            GrainPanel {
                                id: grainPanel
                                theme: window
                                photoReady: sourceImage.status === Image.Ready
                                selectedParameter: window.selectedParameter
                                advancedExpanded: window.grainAdvancedExpanded
                                settingsJson: backend.settingsJson
                                supportedParametersJson: backend.supportedParametersJson
                                onSelectionRequested: index => window.selectParameter(index)
                                onAdvancedToggleRequested: window.toggleGrainAdvanced()
                                onParameterCommitted: (id, value) =>
                                    backend.setParameter(id, value)
                            }

                            PresetsPanel {
                                theme: window
                                photoReady: sourceImage.status === Image.Ready
                                catalogJson: backend.presetCatalogJson
                                selectedPresetId: backend.selectedPresetId
                                onPresetRequested: id => backend.selectPreset(id)
                            }

                            MetadataPanel {
                                theme: window
                                fileName: backend.fileName
                                metadataText: backend.metadataText
                            }
                        }
                    }
                }
            }

            Rectangle {
                Layout.fillWidth: true
                Layout.preferredHeight: window.photoFullscreen ? 0 : 28
                visible: !window.photoFullscreen
                color: window.surfaceColor

                Rectangle {
                    anchors.top: parent.top
                    width: parent.width
                    height: 1
                    color: window.lineColor
                }

                RowLayout {
                    anchors.fill: parent
                    anchors.leftMargin: 12
                    anchors.rightMargin: 12
                    spacing: 18

                    Text {
                        text: "[1–3] PANELS"
                        color: window.accentColor
                        font.family: window.monoFont
                        font.pixelSize: 10
                        font.bold: true
                    }

                    Text {
                        text: "[O] OPEN"
                        color: window.inkColor
                        font.family: window.monoFont
                        font.pixelSize: 10
                    }

                    Text {
                        text: "[S] SAVE"
                        color: window.inkColor
                        font.family: window.monoFont
                        font.pixelSize: 10
                    }

                    Text {
                        text: "[−/+] ZOOM"
                        color: window.inkColor
                        font.family: window.monoFont
                        font.pixelSize: 10
                    }

                    Text {
                        text: "[0] FIT  [F] FULL"
                        color: window.inkColor
                        font.family: window.monoFont
                        font.pixelSize: 10
                    }

                    Item { Layout.fillWidth: true }

                    Text {
                        visible: window.selectedPanel === 0 && window.width >= 1050
                        text: "[↑/↓] SELECT  [←/→] ADJUST  [⇧] FAST  [R] RESET"
                        color: window.mutedColor
                        font.family: window.monoFont
                        font.pixelSize: 10
                    }

                    Text {
                        text: "[?] KEYS"
                        color: window.accentColor
                        font.family: window.monoFont
                        font.pixelSize: 10
                        font.bold: true
                    }
                }
            }
        }

        Rectangle {
            anchors.fill: parent
            visible: window.exportMenuVisible
            z: 110
            color: Qt.rgba(0, 0, 0, 0.76)

            MouseArea {
                anchors.fill: parent
                onClicked: window.exportMenuVisible = false
            }

            Rectangle {
                anchors.centerIn: parent
                width: Math.min(430, parent.width - 48)
                height: 360
                color: window.pageColor
                border.width: 1
                border.color: window.accentColor

                ColumnLayout {
                    anchors.fill: parent
                    anchors.margins: 20
                    spacing: 10

                    RowLayout {
                        Layout.fillWidth: true

                        Text {
                            text: "SAVE / FORMAT"
                            color: window.inkColor
                            font.family: window.monoFont
                            font.pixelSize: 14
                            font.bold: true
                        }

                        Item { Layout.fillWidth: true }

                        Text {
                            text: "ESC"
                            color: window.accentColor
                            font.family: window.monoFont
                            font.pixelSize: 10
                        }
                    }

                    Text {
                        Layout.fillWidth: true
                        text: "ORIGINAL COPIES THE SOURCE. JPEG AND HEIC INCLUDE THE CURRENT GRAIN."
                        color: window.mutedColor
                        wrapMode: Text.WordWrap
                        font.family: window.monoFont
                        font.pixelSize: 9
                    }

                    Rectangle {
                        Layout.fillWidth: true
                        Layout.preferredHeight: 1
                        color: window.lineColor
                    }

                    TuiButton {
                        theme: window
                        Layout.fillWidth: true
                        text: "[1] ORIGINAL · " + backend.originalFormat
                            + "     " + window.exportDetails("ORIGINAL")
                        onClicked: window.chooseExportFormat("ORIGINAL")
                    }

                    TuiButton {
                        theme: window
                        Layout.fillWidth: true
                        text: "[2] JPEG · .JPG     " + window.exportDetails("JPEG")
                        onClicked: window.chooseExportFormat("JPEG")
                    }

                    TuiButton {
                        theme: window
                        Layout.fillWidth: true
                        text: "[3] HEIC · .HEIC     " + window.exportDetails("HEIC")
                        onClicked: window.chooseExportFormat("HEIC")
                    }

                    Rectangle {
                        Layout.fillWidth: true
                        Layout.preferredHeight: 1
                        color: window.lineColor
                    }

                    RowLayout {
                        Layout.fillWidth: true

                        Text {
                            text: "QUALITY  ·  JPEG / HEIC"
                            color: window.mutedColor
                            font.family: window.monoFont
                            font.pixelSize: 10
                            font.bold: true
                        }

                        Item { Layout.fillWidth: true }

                        Text {
                            text: window.exportQuality + "%"
                            color: window.accentColor
                            font.family: window.monoFont
                            font.pixelSize: 11
                            font.bold: true
                        }
                    }

                    Slider {
                        id: exportQualitySlider
                        Layout.fillWidth: true
                        Layout.preferredHeight: 20
                        from: 1
                        to: 100
                        value: window.exportQuality
                        stepSize: 1
                        snapMode: Slider.SnapAlways
                        activeFocusOnTab: false
                        onMoved: window.exportQuality = Math.round(value)

                        background: Rectangle {
                            x: exportQualitySlider.leftPadding
                            y: exportQualitySlider.topPadding
                               + exportQualitySlider.availableHeight / 2 - height / 2
                            width: exportQualitySlider.availableWidth
                            height: 2
                            color: window.lineColor

                            Rectangle {
                                width: exportQualitySlider.visualPosition * parent.width
                                height: parent.height
                                color: window.accentColor
                            }
                        }

                        handle: Rectangle {
                            x: exportQualitySlider.leftPadding
                               + exportQualitySlider.visualPosition
                               * (exportQualitySlider.availableWidth - width)
                            y: exportQualitySlider.topPadding
                               + exportQualitySlider.availableHeight / 2 - height / 2
                            width: 8
                            height: 8
                            color: window.accentColor
                        }
                    }

                    Text {
                        Layout.fillWidth: true
                        text: "←/→ ADJUST  ·  SHIFT FAST  ·  SIZES ARE ESTIMATES"
                        color: window.mutedColor
                        font.family: window.monoFont
                        font.pixelSize: 9
                    }

                    Item { Layout.fillHeight: true }
                }
            }
        }

        Rectangle {
            anchors.fill: parent
            visible: window.shortcutsVisible
            z: 100
            color: Qt.rgba(0, 0, 0, 0.76)

            TapHandler {
                onTapped: window.shortcutsVisible = false
            }

            Rectangle {
                anchors.centerIn: parent
                width: Math.min(560, parent.width - 48)
                height: Math.min(500, parent.height - 48)
                color: window.pageColor
                border.width: 1
                border.color: window.accentColor

                ColumnLayout {
                    anchors.fill: parent
                    anchors.margins: 24
                    spacing: 14

                    RowLayout {
                        Layout.fillWidth: true

                        Text {
                            text: "KEYBOARD / OMALUX"
                            color: window.inkColor
                            font.family: window.monoFont
                            font.pixelSize: 15
                            font.bold: true
                        }

                        Item { Layout.fillWidth: true }

                        Text {
                            text: "ESC"
                            color: window.accentColor
                            font.family: window.monoFont
                            font.pixelSize: 11
                        }
                    }

                    Rectangle {
                        Layout.fillWidth: true
                        Layout.preferredHeight: 1
                        color: window.lineColor
                    }

                    Text {
                        Layout.fillWidth: true
                        text: "1–3         EDIT / PRESETS / META\nTAB / [ ]   CHANGE PANEL\n\nA           GRAIN SUBPARAMETERS\nJ / K       SELECT PARAMETER\n↓ / ↑       SELECT PARAMETER\nH / L       ADJUST VALUE\n← / →       ADJUST VALUE\nSHIFT+H/L   ADJUST FAST\nG / S / M   GRAIN / SIZE / MIDTONES\nR           RESET SELECTED VALUE\n\n− / +       ZOOM OUT / IN\n0           FIT PHOTOGRAPH\nF           PHOTO FULLSCREEN\nO           OPEN PHOTOGRAPH\n\nCTRL+O      OPEN PHOTOGRAPH\nCTRL+S      SAVE / EXPORT\nCTRL+−/+    ZOOM OUT / IN\nCTRL+0      FIT PHOTOGRAPH\n? / F1      THIS REFERENCE"
                        color: window.inkColor
                        font.family: window.monoFont
                        font.pixelSize: 12
                        lineHeight: 1.45
                    }

                    Item { Layout.fillHeight: true }

                    Text {
                        Layout.alignment: Qt.AlignRight
                        text: "PRESS ESC OR ? TO CLOSE"
                        color: window.mutedColor
                        font.family: window.monoFont
                        font.pixelSize: 10
                    }
                }
            }
        }
    }
}
