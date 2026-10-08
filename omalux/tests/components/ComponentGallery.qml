import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import "../../ui/components"

// Every module widget with sample data, at sidebar width, for looking at and trying out.
//   /usr/lib/qt6/bin/qml omalux/tests/components/ComponentGallery.qml
// "-- --picker" opens the colour picker popup. Edits only change this page's sample data.
ApplicationWindow {
    id: window
    width: 3 * 352 + 4 * 16
    height: 1010
    visible: true
    color: editorTheme.background
    title: "Omalux component gallery"
    EditorTheme { id: editorTheme }
    palette.window: editorTheme.background
    palette.windowText: editorTheme.ink
    palette.text: editorTheme.ink
    palette.base: editorTheme.well
    palette.button: editorTheme.surface
    palette.buttonText: editorTheme.ink
    palette.highlight: editorTheme.accent

    readonly property bool openPicker: Qt.application.arguments.indexOf("--picker") >= 0

    component Caption: Text {
        color: editorTheme.muted
        font: editorTheme.moduleHeadingFont
        Layout.topMargin: 6
    }
    component Block: Rectangle {
        default property alias content: inner.data
        Layout.fillWidth: true
        implicitHeight: inner.implicitHeight + 20
        color: editorTheme.surface
        ColumnLayout {
            id: inner
            x: 14; y: 10
            width: parent.width - 28
            spacing: 8
        }
    }

    // Sample state, edited by the widgets.
    property var toneNodes: [{ x: 0, y: 0 }, { x: 0.22, y: 0.16 }, { x: 0.52, y: 0.58 }, { x: 0.8, y: 0.9 }, { x: 1, y: 1 }]
    property var rgbNodes: [[{ x: 0, y: 0 }, { x: 0.35, y: 0.42 }, { x: 1, y: 1 }],
                            [{ x: 0, y: 0 }, { x: 1, y: 1 }],
                            [{ x: 0, y: 0.05 }, { x: 0.6, y: 0.55 }, { x: 1, y: 0.95 }]]
    property int rgbChannel: 0
    property var zoneNodes: [{ x: 0, y: 0.5 }, { x: 0.125, y: 0.62 }, { x: 0.3, y: 0.5 }, { x: 0.45, y: 0.36 },
                             { x: 0.62, y: 0.5 }, { x: 0.78, y: 0.66 }, { x: 0.9, y: 0.5 }]
    property int zoneChannel: 2
    property var toneEq: [0, 0.2, 0.55, 0.4, 0.1, -0.3, -0.7, -0.5, -0.1]
    property var swatch: [0.86, 0.42, 0.18]
    property int tab: 0
    property real mode: 1
    property real preserve: 1
    property var histogram: {
        const h = []
        for (let i = 0; i < 128; ++i) {
            const x = i / 127
            h.push(Math.exp(-Math.pow((x - 0.32) / 0.13, 2)) + 0.6 * Math.exp(-Math.pow((x - 0.7) / 0.08, 2)) + 0.05)
        }
        return h
    }
    property var sampleNodes: [{ x: 0, y: 0.08 }, { x: 0.18, y: 0.1 }, { x: 0.4, y: 0.62 }, { x: 0.55, y: 0.6 }, { x: 0.85, y: 0.92 }, { x: 1, y: 0.9 }]

    RowLayout {
        id: columns
        x: 16; y: 16
        spacing: 16

        // Column 1: curves.
        ColumnLayout {
            Layout.preferredWidth: 352
            Layout.alignment: Qt.AlignTop
            spacing: 10
            Caption { text: "ModuleTabs" }
            Block {
                ModuleTabs {
                    Layout.fillWidth: true
                    theme: editorTheme
                    tabs: ["master", "4 ways", "masks"]
                    currentIndex: window.tab
                    onTabSelected: i => window.tab = i
                }
            }
            Caption { text: "ChannelChooser + CurveEditor · rgb curve" }
            Block {
                ChannelChooser {
                    theme: editorTheme
                    options: [{ label: "R", value: 0, color: "#e06c75" }, { label: "G", value: 1, color: "#98c379" },
                              { label: "B", value: 2, color: "#61afef" }]
                    current: window.rgbChannel
                    onChosen: v => window.rgbChannel = v
                }
                CurveEditor {
                    Layout.fillWidth: true
                    theme: editorTheme
                    curveLabel: "curve"
                    interpolation: "monotone"
                    nodes: window.rgbNodes[window.rgbChannel]
                    curveColor: ["#e06c75", "#98c379", "#61afef"][window.rgbChannel]
                    histogram: window.histogram
                    background: "gradient"
                    gradientColors: ["#000000", ["#e06c75", "#98c379", "#61afef"][window.rgbChannel]]
                    activeIndex: 1
                    onNodesEdited: n => { const all = window.rgbNodes.slice(); all[window.rgbChannel] = n; window.rgbNodes = all }
                }
            }
            Caption { text: "CurveEditor · color zones (periodic hue)" }
            Block {
                ChannelChooser {
                    theme: editorTheme
                    options: [{ label: "lightness", value: 0 }, { label: "chroma", value: 1 }, { label: "hue", value: 2 }]
                    current: window.zoneChannel
                    onChosen: v => window.zoneChannel = v
                }
                CurveEditor {
                    Layout.fillWidth: true
                    theme: editorTheme
                    curveLabel: "hue"
                    periodic: true
                    interpolation: "catmull"
                    background: "gradient-hue"
                    aspectRatio: 0.6
                    nodes: window.zoneNodes
                    onNodesEdited: n => window.zoneNodes = n
                }
            }
        }

        // Column 2: tone curve, graphs, notice.
        ColumnLayout {
            Layout.preferredWidth: 352
            Layout.alignment: Qt.AlignTop
            spacing: 10
            Caption { text: "CurveEditor · tone curve (log scale 64)" }
            Block {
                CurveEditor {
                    Layout.fillWidth: true
                    theme: editorTheme
                    curveLabel: "L"
                    splineVersion: 1
                    interpolation: "cubic"
                    xLog: 64; yLog: 64
                    background: "gradient-luma"
                    histogram: window.histogram
                    nodes: window.toneNodes
                    aspectRatio: 0.8
                    hoverIndex: 2
                    onNodesEdited: n => window.toneNodes = n
                }
            }
            Caption { text: "GraphView · tone equalizer (editable)" }
            Block {
                GraphView {
                    Layout.fillWidth: true
                    theme: editorTheme
                    title: "simple"
                    xs: [-8, -7, -6, -5, -4, -3, -2, -1, 0]
                    ys: window.toneEq
                    yMin: -2; yMax: 2
                    editable: true
                    activeIndex: 6
                    labels: ["-8", "-7", "-6", "-5", "-4", "-3", "-2", "-1", "0 EV"]
                    formatValue: v => (v >= 0 ? "+" : "") + v.toFixed(2) + " EV"
                    onValueEdited: (i, v) => { const a = window.toneEq.slice(); a[i] = v; window.toneEq = a }
                }
            }
            Caption { text: "GraphView · contrast equalizer (read-only)" }
            Block {
                GraphView {
                    Layout.fillWidth: true
                    theme: editorTheme
                    title: "luma"
                    xs: [0, 1, 2, 3, 4, 5, 6]
                    ys: [0.5, 0.58, 0.66, 0.7, 0.62, 0.55, 0.5]
                    aspectRatio: 0.35
                    axisLabels: ["coarse", "fine"]
                }
            }
            Caption { text: "ModuleNotice" }
            Block {
                ModuleNotice {
                    Layout.fillWidth: true
                    theme: editorTheme
                    text: "drawn on the image — not available yet"
                }
                ModuleNotice {
                    Layout.fillWidth: true
                    theme: editorTheme
                    text: "masks and blending are kept from the image history but cannot be edited here"
                }
            }
        }

        // Column 3: colour, enums, interpolation comparison.
        ColumnLayout {
            Layout.preferredWidth: 352
            Layout.alignment: Qt.AlignTop
            spacing: 10
            Caption { text: "ColorSwatch" }
            Block {
                ColorSwatch {
                    id: frameColor
                    Layout.fillWidth: true
                    theme: editorTheme
                    label: "frame line color"
                    color: window.swatch
                    onColorEdited: c => window.swatch = c
                }
                Item { Layout.preferredHeight: window.openPicker ? 214 : 0 }
                ColorSwatch {
                    Layout.fillWidth: true
                    theme: editorTheme
                    label: "border color"
                    color: { "r": 0.95, "g": 0.94, "b": 0.9 }
                }
            }
            Caption { text: "ControlChoice · ControlSwitch" }
            Block {
                ControlChoice {
                    Layout.fillWidth: true
                    theme: editorTheme
                    label: "preserve colors"
                    options: [{ value: 0, label: "none" }, { value: 1, label: "luminance" }, { value: 2, label: "max RGB" },
                              { value: 3, label: "average RGB" }, { value: 4, label: "sum RGB" }, { value: 5, label: "norm RGB" },
                              { value: 6, label: "basic power" }]
                    value: window.preserve
                    editable: true
                    onEdited: v => window.preserve = v
                }
                ControlChoice {
                    Layout.fillWidth: true
                    theme: editorTheme
                    label: "color space"
                    options: [{ value: 0, label: "Lab, linked" }, { value: 1, label: "Lab, independent" }, { value: 2, label: "RGB" }]
                    value: window.mode
                    editable: true
                    onEdited: v => window.mode = v
                }
                ControlSwitch {
                    Layout.fillWidth: true
                    theme: editorTheme
                    label: "preserve hue"
                    value: 1
                    editable: true
                }
            }
            Caption { text: "Interpolation: cubic · catmull · monotone · linear" }
            Block {
                GridLayout {
                    Layout.fillWidth: true
                    columns: 2
                    columnSpacing: 10
                    rowSpacing: 8
                    Repeater {
                        model: ["cubic", "catmull", "monotone", "linear"]
                        CurveEditor {
                            required property string modelData
                            Layout.fillWidth: true
                            Layout.preferredWidth: 1
                            theme: editorTheme
                            curveLabel: modelData
                            interpolation: modelData
                            nodes: window.sampleNodes
                            onNodesEdited: n => window.sampleNodes = n
                        }
                    }
                }
            }
        }
    }

    Timer {
        running: window.openPicker
        interval: 300
        onTriggered: frameColor.openPicker()
    }
}
