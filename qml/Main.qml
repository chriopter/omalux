pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls
import QtQuick.Dialogs
import QtQuick.Layouts
import QtQuick.Window
import io.omacom.grainroom 1.0

ApplicationWindow {
    id: window
    width: 1280
    height: 820
    minimumWidth: 800
    minimumHeight: 560
    visible: true
    title: backend.fileName.length > 0 ? backend.fileName + " — Grainroom" : "Grainroom"
    color: pageColor

    readonly property color pageColor: "#101010"
    readonly property color surfaceColor: "#171717"
    readonly property color raisedColor: "#202020"
    readonly property color inkColor: "#eeeeee"
    readonly property color mutedColor: "#8f9191"
    readonly property color lineColor: "#363636"
    readonly property color accentColor: "#5584aa"
    readonly property string monoFont: "iA Writer Mono S"

    property real zoom: 1.0
    property int selectedPanel: 1
    property int selectedParameter: 0
    property bool grainAdvancedExpanded: false
    property bool grainEnabled: true
    property bool shortcutsVisible: false

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
        if (index === 0)
            return grainControl
        if (index === 1)
            return grainSizeControl
        return midtonesControl
    }

    function selectParameter(index) {
        if (index > 0)
            grainAdvancedExpanded = true
        selectedParameter = (index + 3) % 3
    }

    function moveParameter(direction) {
        if (!grainAdvancedExpanded) {
            selectedParameter = 0
            return
        }
        selectParameter(selectedParameter + direction)
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

    function selectPanel(index) {
        selectedPanel = (index + 3) % 3
    }

    function movePanel(direction) {
        selectPanel(selectedPanel + direction)
    }

    component TuiButton: Button {
        id: control
        property bool primary: false

        activeFocusOnTab: false
        leftPadding: 11
        rightPadding: 11
        topPadding: 6
        bottomPadding: 6

        contentItem: Text {
            text: control.text
            color: control.enabled
                ? (control.primary ? window.pageColor : window.inkColor)
                : window.mutedColor
            horizontalAlignment: Text.AlignHCenter
            verticalAlignment: Text.AlignVCenter
            font.family: window.monoFont
            font.pixelSize: 12
            font.bold: control.primary
        }

        background: Rectangle {
            implicitHeight: 30
            implicitWidth: 56
            radius: 0
            color: control.primary
                ? (control.down ? Qt.darker(window.inkColor, 1.25) : window.inkColor)
                : control.down
                    ? "#303030"
                    : control.hovered ? window.raisedColor : "transparent"
            border.width: 1
            border.color: control.primary
                ? window.inkColor
                : control.hovered ? window.mutedColor : window.lineColor
        }
    }

    component ParameterControl: Item {
        id: parameter
        required property int parameterIndex
        required property string label
        required property real from
        required property real to
        required property real initialValue
        property real stepSize: 1
        property real coarseStep: stepSize * 10
        property string suffix: ""
        property alias value: slider.value
        readonly property bool selected: window.selectedParameter === parameterIndex

        implicitHeight: 98
        Accessible.role: Accessible.Slider
        Accessible.name: label
        Accessible.description: Math.round(value) + suffix

        function nudge(direction, coarse) {
            var amount = coarse ? coarseStep : stepSize
            slider.value = Math.max(from, Math.min(to, slider.value + direction * amount))
        }

        function resetValue() {
            slider.value = initialValue
        }

        Rectangle {
            anchors.fill: parent
            color: parameter.selected ? Qt.rgba(0.33, 0.52, 0.67, 0.10) : "transparent"
            border.width: 1
            border.color: parameter.selected ? window.accentColor : window.lineColor
        }

        ColumnLayout {
            anchors.fill: parent
            anchors.leftMargin: 13
            anchors.rightMargin: 13
            anchors.topMargin: 11
            anchors.bottomMargin: 10
            spacing: 8

            RowLayout {
                Layout.fillWidth: true

                Text {
                    text: (parameter.parameterIndex + 1) + "  " + parameter.label.toUpperCase()
                    color: parameter.selected ? window.inkColor : window.mutedColor
                    font.family: window.monoFont
                    font.pixelSize: 12
                    font.bold: parameter.selected
                }

                Item { Layout.fillWidth: true }

                Text {
                    text: Math.round(parameter.value) + parameter.suffix
                    color: parameter.selected ? window.accentColor : window.inkColor
                    font.family: window.monoFont
                    font.pixelSize: 12
                    font.bold: true
                }
            }

            Slider {
                id: slider
                Layout.fillWidth: true
                Layout.preferredHeight: 24
                from: parameter.from
                to: parameter.to
                value: parameter.initialValue
                stepSize: parameter.stepSize
                snapMode: Slider.SnapAlways
                enabled: sourceImage.status === Image.Ready
                activeFocusOnTab: false
                onPressedChanged: if (pressed)
                    window.selectParameter(parameter.parameterIndex)

                background: Rectangle {
                    x: slider.leftPadding
                    y: slider.topPadding + slider.availableHeight / 2 - height / 2
                    width: slider.availableWidth
                    height: 2
                    color: window.lineColor

                    Rectangle {
                        width: slider.visualPosition * parent.width
                        height: parent.height
                        color: slider.enabled ? window.accentColor : window.mutedColor
                    }
                }

                handle: Rectangle {
                    x: slider.leftPadding + slider.visualPosition * (slider.availableWidth - width)
                    y: slider.topPadding + slider.availableHeight / 2 - height / 2
                    width: parameter.selected ? 12 : 10
                    height: width
                    radius: 0
                    color: slider.enabled ? window.inkColor : window.mutedColor
                    border.width: parameter.selected ? 2 : 1
                    border.color: parameter.selected ? window.accentColor : window.pageColor
                }
            }

            Text {
                Layout.fillWidth: true
                text: parameter.selected ? "H/L ADJUST  ·  SHIFT FAST  ·  R RESET" : ""
                color: window.mutedColor
                font.family: window.monoFont
                font.pixelSize: 9
            }
        }

        HoverHandler {
            onHoveredChanged: if (hovered)
                window.selectParameter(parameter.parameterIndex)
        }
    }

    component MockParameterControl: Item {
        id: mockParameter
        required property string label
        property real from: 0
        property real to: 100
        property real initialValue: 50
        property string suffix: ""

        implicitHeight: 68
        Accessible.role: Accessible.Slider
        Accessible.name: label + " mock control"

        Rectangle {
            anchors.fill: parent
            color: "transparent"
            border.width: 1
            border.color: window.lineColor
        }

        ColumnLayout {
            anchors.fill: parent
            anchors.leftMargin: 12
            anchors.rightMargin: 12
            anchors.topMargin: 8
            anchors.bottomMargin: 7
            spacing: 4

            RowLayout {
                Layout.fillWidth: true

                Text {
                    text: mockParameter.label.toUpperCase()
                    color: window.mutedColor
                    font.family: window.monoFont
                    font.pixelSize: 10
                }

                Text {
                    text: "MOCK"
                    color: window.accentColor
                    font.family: window.monoFont
                    font.pixelSize: 8
                }

                Item { Layout.fillWidth: true }

                Text {
                    text: Math.round(mockSlider.value) + mockParameter.suffix
                    color: window.inkColor
                    font.family: window.monoFont
                    font.pixelSize: 10
                }
            }

            Slider {
                id: mockSlider
                Layout.fillWidth: true
                Layout.preferredHeight: 20
                from: mockParameter.from
                to: mockParameter.to
                value: mockParameter.initialValue
                enabled: sourceImage.status === Image.Ready
                activeFocusOnTab: false

                background: Rectangle {
                    x: mockSlider.leftPadding
                    y: mockSlider.topPadding + mockSlider.availableHeight / 2 - height / 2
                    width: mockSlider.availableWidth
                    height: 2
                    color: window.lineColor

                    Rectangle {
                        width: mockSlider.visualPosition * parent.width
                        height: parent.height
                        color: window.mutedColor
                    }
                }

                handle: Rectangle {
                    x: mockSlider.leftPadding
                       + mockSlider.visualPosition * (mockSlider.availableWidth - width)
                    y: mockSlider.topPadding + mockSlider.availableHeight / 2 - height / 2
                    width: 8
                    height: 8
                    radius: 0
                    color: window.mutedColor
                }
            }
        }
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

    Shortcut {
        sequence: StandardKey.Open
        onActivated: openDialog.open()
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
    }

    Item {
        id: keyboardLayer
        anchors.fill: parent
        focus: true
        Keys.priority: Keys.BeforeItem

        Component.onCompleted: forceActiveFocus()

        Keys.onPressed: function(event) {
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
            } else if (window.selectedPanel === 1
                       && (event.key === Qt.Key_Down || keyText === "j")) {
                window.moveParameter(1)
            } else if (window.selectedPanel === 1
                       && (event.key === Qt.Key_Up || keyText === "k")) {
                window.moveParameter(-1)
            } else if (window.selectedPanel === 1
                       && (event.key === Qt.Key_Left || keyText === "h")) {
                window.adjustParameter(-1, coarse)
            } else if (window.selectedPanel === 1
                       && (event.key === Qt.Key_Right || keyText === "l")) {
                window.adjustParameter(1, coarse)
            } else if (keyText === "g") {
                window.selectPanel(1)
                window.selectParameter(0)
            } else if (keyText === "s") {
                window.selectPanel(1)
                window.selectParameter(1)
            } else if (keyText === "m") {
                window.selectPanel(1)
                window.selectParameter(2)
            } else if (keyText === "a" && window.selectedPanel === 1) {
                window.toggleGrainAdvanced()
            } else if (keyText === "r" && window.selectedPanel === 1) {
                window.resetParameter()
            } else if (keyText === "b") {
                window.grainEnabled = !window.grainEnabled
            } else if (keyText === "o") {
                openDialog.open()
            } else if (keyText === "0") {
                window.fitPhoto()
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
                Layout.preferredHeight: 48
                color: window.pageColor

                Rectangle {
                    anchors.left: parent.left
                    anchors.right: parent.right
                    anchors.bottom: parent.bottom
                    height: 1
                    color: window.lineColor
                }

                RowLayout {
                    anchors.fill: parent
                    anchors.leftMargin: 16
                    anchors.rightMargin: 12
                    spacing: 9

                    Text {
                        text: "GRAINROOM"
                        color: window.inkColor
                        font.family: window.monoFont
                        font.pixelSize: 13
                        font.bold: true
                        font.letterSpacing: 1.5
                    }

                    Rectangle {
                        Layout.preferredWidth: 1
                        Layout.preferredHeight: 20
                        color: window.lineColor
                    }

                    Text {
                        Layout.fillWidth: true
                        text: backend.fileName.length > 0 ? backend.fileName : "NO IMAGE"
                        color: window.mutedColor
                        elide: Text.ElideMiddle
                        font.family: window.monoFont
                        font.pixelSize: 11
                    }

                    BusyIndicator {
                        running: backend.loading
                        visible: running
                        implicitWidth: 22
                        implicitHeight: 22
                    }

                    Text {
                        text: backend.status.toUpperCase()
                        color: window.mutedColor
                        font.family: window.monoFont
                        font.pixelSize: 10
                    }

                    TuiButton {
                        text: "−"
                        enabled: sourceImage.status === Image.Ready && window.zoom > 0.25
                        onClicked: window.zoomOut()
                        ToolTip.visible: hovered
                        ToolTip.text: "Zoom out · − / Ctrl−"
                    }

                    TuiButton {
                        text: Math.round(window.zoom * 100) + "%"
                        enabled: sourceImage.status === Image.Ready
                        onClicked: window.fitPhoto()
                        ToolTip.visible: hovered
                        ToolTip.text: "Fit photograph · 0 / Ctrl+0"
                    }

                    TuiButton {
                        text: "+"
                        enabled: sourceImage.status === Image.Ready && window.zoom < 8.0
                        onClicked: window.zoomIn()
                        ToolTip.visible: hovered
                        ToolTip.text: "Zoom in · + / Ctrl+"
                    }

                    TuiButton {
                        text: "[O] OPEN"
                        primary: true
                        onClicked: openDialog.open()
                    }

                    TuiButton {
                        text: "?"
                        onClicked: window.shortcutsVisible = true
                        ToolTip.visible: hovered
                        ToolTip.text: "Keyboard reference · ? / F1"
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
                            anchors.horizontalCenter: parent.horizontalCenter
                            text: "[ O ]  OPEN PHOTOGRAPH"
                            primary: true
                            onClicked: openDialog.open()
                        }
                    }

                    Flickable {
                        id: photoFlick
                        anchors.fill: parent
                        anchors.margins: 20
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
                                asynchronous: true
                                cache: false
                                visible: false
                            }

                            ShaderEffect {
                                id: grainEffect
                                anchors.fill: parent
                                visible: sourceImage.status === Image.Ready
                                blending: false

                                property var source: sourceImage
                                property real grainAmount: window.grainEnabled
                                    ? grainControl.value / 100.0 : 0.0
                                property vector2d imageSize: Qt.vector2d(
                                    sourceImage.sourceSize.width,
                                    sourceImage.sourceSize.height)
                                property real grainCoarseness: grainSizeControl.value
                                property real midtonesBias: midtonesControl.value / 100.0
                                property real grainSeed: window.stableSeed(backend.fileName)

                                fragmentShader: "shaders/grain.frag.qsb"
                            }

                            Rectangle {
                                anchors.fill: parent
                                color: "transparent"
                                border.width: 1
                                border.color: window.lineColor
                                visible: sourceImage.status === Image.Ready
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
                    Layout.preferredWidth: 316
                    Layout.fillHeight: true
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

                        RowLayout {
                            Layout.fillWidth: true
                            spacing: 6

                            TuiButton {
                                Layout.preferredWidth: 42
                                text: "⌗"
                                primary: window.selectedPanel === 0
                                onClicked: window.selectPanel(0)
                                Accessible.name: "Crop · 1"
                                ToolTip.visible: hovered
                                ToolTip.text: "Crop · 1"
                            }

                            TuiButton {
                                Layout.preferredWidth: 42
                                text: "▒"
                                primary: window.selectedPanel === 1
                                onClicked: window.selectPanel(1)
                                Accessible.name: "Grain shader · 2"
                                ToolTip.visible: hovered
                                ToolTip.text: "Grain shader · 2"
                            }

                            TuiButton {
                                Layout.preferredWidth: 42
                                text: "ⓘ"
                                primary: window.selectedPanel === 2
                                onClicked: window.selectPanel(2)
                                Accessible.name: "Metadata · 3"
                                ToolTip.visible: hovered
                                ToolTip.text: "Metadata · 3"
                            }

                            Item { Layout.fillWidth: true }
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

                            Item {
                                ColumnLayout {
                                    anchors.fill: parent
                                    spacing: 12

                                    Text {
                                        text: "01 / CROP"
                                        color: window.inkColor
                                        font.family: window.monoFont
                                        font.pixelSize: 13
                                        font.bold: true
                                        font.letterSpacing: 1
                                    }

                                    Rectangle {
                                        Layout.fillWidth: true
                                        Layout.preferredHeight: 220
                                        color: window.surfaceColor
                                        border.width: 1
                                        border.color: window.lineColor

                                        Column {
                                            anchors.centerIn: parent
                                            spacing: 9

                                            Text {
                                                anchors.horizontalCenter: parent.horizontalCenter
                                                text: "┌──────────────┐\n│              │\n│  CROP TOOL   │\n│              │\n└──────────────┘"
                                                color: window.mutedColor
                                                font.family: window.monoFont
                                                font.pixelSize: 12
                                                horizontalAlignment: Text.AlignHCenter
                                            }

                                            Text {
                                                anchors.horizontalCenter: parent.horizontalCenter
                                                text: "PLACEHOLDER / NEXT ITERATION"
                                                color: window.accentColor
                                                font.family: window.monoFont
                                                font.pixelSize: 10
                                                font.bold: true
                                            }
                                        }
                                    }

                                    Text {
                                        Layout.fillWidth: true
                                        text: "PLANNED\n\n• FREE CROP\n• ASPECT RATIOS\n• ROTATE / STRAIGHTEN"
                                        color: window.mutedColor
                                        font.family: window.monoFont
                                        font.pixelSize: 11
                                        lineHeight: 1.45
                                    }

                                    Item { Layout.fillHeight: true }
                                }
                            }

                            Item {
                                ColumnLayout {
                                    anchors.fill: parent
                                    spacing: 10

                                    RowLayout {
                                        Layout.fillWidth: true

                                        Text {
                                            text: "02 / GRAIN SHADER"
                                            color: window.inkColor
                                            font.family: window.monoFont
                                            font.pixelSize: 13
                                            font.bold: true
                                            font.letterSpacing: 1
                                        }

                                        Item { Layout.fillWidth: true }

                                        Text {
                                            text: window.grainEnabled ? "● LIVE" : "○ BYPASS"
                                            color: window.grainEnabled
                                                ? window.accentColor : window.mutedColor
                                            font.family: window.monoFont
                                            font.pixelSize: 10
                                        }
                                    }

                                    Text {
                                        Layout.fillWidth: true
                                        text: "MAIN CONTROL  ·  [A] ADVANCED"
                                        color: window.mutedColor
                                        font.family: window.monoFont
                                        font.pixelSize: 10
                                    }

                                    ScrollView {
                                        id: grainScroll
                                        Layout.fillWidth: true
                                        Layout.fillHeight: true
                                        clip: true
                                        contentWidth: availableWidth
                                        ScrollBar.horizontal.policy: ScrollBar.AlwaysOff
                                        ScrollBar.vertical.policy: ScrollBar.AsNeeded

                                        ColumnLayout {
                                            width: grainScroll.availableWidth
                                            spacing: 8

                                            ParameterControl {
                                                id: grainControl
                                                Layout.fillWidth: true
                                                parameterIndex: 0
                                                label: "Grain"
                                                from: 0
                                                to: 100
                                                initialValue: 24
                                                stepSize: 1
                                                coarseStep: 10
                                            }

                                            RowLayout {
                                                Layout.fillWidth: true
                                                spacing: 8

                                                TuiButton {
                                                    Layout.preferredWidth: 34
                                                    text: window.grainAdvancedExpanded ? "⌄" : "›"
                                                    onClicked: window.toggleGrainAdvanced()
                                                    Accessible.name: window.grainAdvancedExpanded
                                                        ? "Hide grain subparameters"
                                                        : "Show grain subparameters"
                                                    ToolTip.visible: hovered
                                                    ToolTip.text: Accessible.name
                                                }

                                                Text {
                                                    text: "SIZE / MIDTONES"
                                                    color: window.grainAdvancedExpanded
                                                        ? window.inkColor : window.mutedColor
                                                    font.family: window.monoFont
                                                    font.pixelSize: 10
                                                    font.bold: window.grainAdvancedExpanded
                                                }

                                                Item { Layout.fillWidth: true }

                                                Text {
                                                    text: "2 SUBPARAMETERS"
                                                    color: window.mutedColor
                                                    font.family: window.monoFont
                                                    font.pixelSize: 8
                                                }
                                            }

                                            ColumnLayout {
                                                Layout.fillWidth: true
                                                Layout.leftMargin: 14
                                                spacing: 8
                                                visible: window.grainAdvancedExpanded

                                                ParameterControl {
                                                    id: grainSizeControl
                                                    Layout.fillWidth: true
                                                    parameterIndex: 1
                                                    label: "Size"
                                                    from: 20
                                                    to: 6400
                                                    initialValue: 1600
                                                    stepSize: 100
                                                    coarseStep: 500
                                                    suffix: " ISO"
                                                }

                                                ParameterControl {
                                                    id: midtonesControl
                                                    Layout.fillWidth: true
                                                    parameterIndex: 2
                                                    label: "Midtones"
                                                    from: 0
                                                    to: 100
                                                    initialValue: 100
                                                    stepSize: 1
                                                    coarseStep: 10
                                                }
                                            }

                                            Rectangle {
                                                Layout.fillWidth: true
                                                Layout.topMargin: 6
                                                Layout.preferredHeight: 1
                                                color: window.lineColor
                                            }

                                            Text {
                                                Layout.fillWidth: true
                                                text: "FUTURE CONTROLS / UI MOCK"
                                                color: window.accentColor
                                                font.family: window.monoFont
                                                font.pixelSize: 9
                                                font.bold: true
                                            }

                                            MockParameterControl {
                                                Layout.fillWidth: true
                                                label: "Exposure"
                                                from: -100
                                                to: 100
                                                initialValue: 0
                                            }

                                            MockParameterControl {
                                                Layout.fillWidth: true
                                                label: "Contrast"
                                                initialValue: 50
                                            }

                                            MockParameterControl {
                                                Layout.fillWidth: true
                                                label: "Highlights"
                                                initialValue: 62
                                            }

                                            MockParameterControl {
                                                Layout.fillWidth: true
                                                label: "Shadows"
                                                initialValue: 38
                                            }

                                            MockParameterControl {
                                                Layout.fillWidth: true
                                                label: "Vignette"
                                                initialValue: 18
                                            }

                                            Text {
                                                Layout.fillWidth: true
                                                Layout.topMargin: 4
                                                text: "THREE-OCTAVE FILM GRAIN\nDARKTABLE / RAWTHERAPEE MODEL\nGPU PREVIEW"
                                                color: window.mutedColor
                                                font.family: window.monoFont
                                                font.pixelSize: 10
                                                lineHeight: 1.45
                                            }
                                        }
                                    }
                                }
                            }

                            Item {
                                ColumnLayout {
                                    anchors.fill: parent
                                    spacing: 12

                                    Text {
                                        text: "03 / METADATA"
                                        color: window.inkColor
                                        font.family: window.monoFont
                                        font.pixelSize: 13
                                        font.bold: true
                                        font.letterSpacing: 1
                                    }

                                    Text {
                                        Layout.fillWidth: true
                                        text: backend.fileName.length > 0
                                            ? backend.fileName.toUpperCase()
                                            : "NO PHOTOGRAPH"
                                        color: window.accentColor
                                        elide: Text.ElideMiddle
                                        font.family: window.monoFont
                                        font.pixelSize: 11
                                        font.bold: true
                                    }

                                    Rectangle {
                                        Layout.fillWidth: true
                                        Layout.preferredHeight: 1
                                        color: window.lineColor
                                    }

                                    ScrollView {
                                        Layout.fillWidth: true
                                        Layout.fillHeight: true
                                        clip: true

                                        Text {
                                            width: parent.width
                                            text: backend.metadataText
                                            color: backend.fileName.length > 0
                                                ? window.inkColor : window.mutedColor
                                            font.family: window.monoFont
                                            font.pixelSize: 11
                                            lineHeight: 1.65
                                            wrapMode: Text.WrapAnywhere
                                        }
                                    }

                                    Rectangle {
                                        Layout.fillWidth: true
                                        Layout.preferredHeight: 1
                                        color: window.lineColor
                                    }

                                    Text {
                                        Layout.fillWidth: true
                                        text: "EXIF / LIBRAW\nREAD-ONLY"
                                        color: window.mutedColor
                                        font.family: window.monoFont
                                        font.pixelSize: 10
                                        lineHeight: 1.45
                                    }
                                }
                            }
                        }
                    }
                }
            }

            Rectangle {
                Layout.fillWidth: true
                Layout.preferredHeight: 28
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
                        text: "[1/2/3] PANELS"
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
                        text: "[B] " + (window.grainEnabled ? "BYPASS" : "ENABLE")
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
                        text: "[0] FIT"
                        color: window.inkColor
                        font.family: window.monoFont
                        font.pixelSize: 10
                    }

                    Item { Layout.fillWidth: true }

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
                            text: "KEYBOARD / GRAINROOM"
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
                        text: "1 / 2 / 3   CROP / GRAIN / METADATA\nTAB / [ ]   CHANGE PANEL\n\nA           GRAIN SUBPARAMETERS\nJ / K       SELECT GRAIN PARAMETER\n↓ / ↑       SELECT GRAIN PARAMETER\nH / L       ADJUST VALUE\n← / →       ADJUST VALUE\nSHIFT+H/L   ADJUST FAST\nG / S / M   GRAIN / SIZE / MIDTONES\nR           RESET SELECTED VALUE\n\nB           TOGGLE GRAIN BYPASS\n− / +       ZOOM OUT / IN\n0           FIT PHOTOGRAPH\nO           OPEN PHOTOGRAPH\n\nCTRL+O      OPEN PHOTOGRAPH\nCTRL+−/+    ZOOM OUT / IN\nCTRL+0      FIT PHOTOGRAPH\n? / F1      THIS REFERENCE"
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
