import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

// denoise (profiled): mode, color mode and the wavelet curve of the chosen Y0U0V0 channel
// (denoiseprofile.c), in the same rows as the sliders above it: choice rows, channel chips and
// a recessed graph with darktable's coarse–fine axis.
ColumnLayout {
    id: root
    required property var theme
    required property var values
    required property bool editable
    signal edited(string id, real value)
    property int channel: 4
    property string group: ""     // keyboard group (the module)
    spacing: 6

    ControlChoice {
        // ControlChoice keeps 8 px inside its row; here the column already has the row margins.
        Layout.fillWidth: true
        Layout.leftMargin: -8
        Layout.rightMargin: -8
        theme: root.theme
        label: "mode"
        labelFont: root.theme.settingsFont
        labelColor: root.theme.ink
        options: ["non-local means", "wavelets", "compute variance", "non-local means auto", "wavelets auto"]
                 .map((label, value) => ({ value: value, label: label }))
        value: root.values.denoise_mode || 0
        editable: root.editable
        onEdited: v => root.edited("denoise_mode", v)
        navTarget.navId: "denoise_mode"
        navTarget.group: root.group
        navTarget.resettable: false
    }
    ControlChoice {
        // ControlChoice keeps 8 px inside its row; here the column already has the row margins.
        Layout.fillWidth: true
        Layout.leftMargin: -8
        Layout.rightMargin: -8
        theme: root.theme
        label: "color mode"
        labelFont: root.theme.settingsFont
        labelColor: root.theme.ink
        options: [{ value: 0, label: "RGB" }, { value: 1, label: "Y0U0V0" }]
        value: root.values.denoise_color_mode || 0
        editable: root.editable
        onEdited: v => root.edited("denoise_color_mode", v)
        navTarget.navId: "denoise_color_mode"
        navTarget.group: root.group
        navTarget.resettable: false
    }
    ModuleNotice {
        Layout.fillWidth: true
        visible: root.values.denoise_color_mode !== 1
        theme: root.theme
        text: "select Y0U0V0 to edit luminance and chroma separately"
    }
    ColumnLayout {
        Layout.fillWidth: true
        spacing: 6
        visible: root.values.denoise_color_mode === 1 && (root.values.denoise_mode === 1 || root.values.denoise_mode === 4)
        RowLayout {
            Layout.fillWidth: true
            ChannelChooser {
                theme: root.theme
                options: [{ label: "Y0", value: 4 }, { label: "U0V0", value: 5 }]
                current: root.channel
                onChosen: v => root.channel = v
                navTarget.navId: "denoise_channel"
                navTarget.group: root.group
                ToolTip.visible: hover.hovered
                ToolTip.delay: 500
                ToolTip.text: root.channel === 4 ? "luminance" : "chroma"
                HoverHandler { id: hover }
            }
            Item { Layout.fillWidth: true }
            ToolButton {
                id: resetButton
                padding: 0
                hoverEnabled: true
                enabled: root.editable
                onClicked: { for (let b = 0; b < 7; ++b) root.edited("denoise_" + root.channel + "_" + b, .5) }
                Accessible.name: "Reset " + (root.channel === 4 ? "Y0" : "U0V0") + " curve"
                ToolTip.visible: hovered
                ToolTip.delay: 500
                ToolTip.text: "reset the " + (root.channel === 4 ? "Y0" : "U0V0") + " curve"
                contentItem: Text {
                    text: "reset"
                    color: resetButton.hovered ? root.theme.accent : root.theme.muted
                    font: root.theme.textFont
                }
                background: Item {}
            }
        }
        Rectangle {
            Layout.fillWidth: true
            implicitHeight: 130
            radius: 4
            color: root.theme.well
            border.color: root.theme.line
            Canvas {
                id: graph
                anchors.fill: parent; anchors.margins: 8
                property var values: root.values
                property int channel: root.channel
                onValuesChanged: requestPaint()
                onChannelChanged: requestPaint()
                onPaint: {
                    const c = getContext("2d"); c.clearRect(0, 0, width, height)
                    c.strokeStyle = root.theme.line; c.lineWidth = 1
                    for (let i = 1; i < 4; ++i) { c.beginPath(); c.moveTo(0, height * i / 4); c.lineTo(width, height * i / 4); c.stroke() }
                    c.strokeStyle = root.channel === 4 ? root.theme.ink : "#dc9960"; c.lineWidth = 2; c.beginPath()
                    for (let i = 0; i < 7; ++i) {
                        const x = i / 6 * width, y = (1 - root.values["denoise_" + root.channel + "_" + i]) * height
                        if (i === 0) c.moveTo(x, y); else c.lineTo(x, y)
                    }
                    c.stroke()
                    c.fillStyle = root.theme.accent
                    for (let i = 0; i < 7; ++i) {
                        const x = i / 6 * width, y = (1 - root.values["denoise_" + root.channel + "_" + i]) * height
                        c.beginPath(); c.arc(x, y, 3.5, 0, 2 * Math.PI); c.fill()
                    }
                }
                MouseArea {
                    anchors.fill: parent; enabled: root.editable; preventStealing: true
                    property int band: 0
                    function edit(y) { root.edited("denoise_" + root.channel + "_" + band, Math.max(0, Math.min(1, 1 - y / height))) }
                    onPressed: mouse => { band = Math.max(0, Math.min(6, Math.round(mouse.x / width * 6))); edit(mouse.y) }
                    onPositionChanged: mouse => { if (pressed) edit(mouse.y) }
                    Accessible.name: root.channel === 4 ? "Y0 wavelet control points" : "U0V0 wavelet control points"
                }
            }
        }
        RowLayout {
            Layout.fillWidth: true
            Text { text: "coarse"; color: root.theme.muted; font: root.theme.textFont }
            Item { Layout.fillWidth: true }
            Text { text: "fine"; color: root.theme.muted; font: root.theme.textFont }
        }
    }
}
