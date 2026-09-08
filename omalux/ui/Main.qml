import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

ApplicationWindow {
    id: window
    width: 1280; height: 820
    minimumWidth: 780; minimumHeight: 520
    visible: true
    title: editor.filename + " — Omalux"
    color: "#1e1e2e"
    readonly property color ink: "#cdd6f4"
    readonly property color muted: "#7f849c"
    readonly property color line: "#45475a"
    readonly property color accent: "#89b4fa"
    font.family: "JetBrains Mono"
    font.pixelSize: 11

    component GhostButton: Rectangle {
        property string label
        implicitWidth: label.length * 7 + 22; implicitHeight: 30
        color: "transparent"; border.color: window.line; opacity: 0.45
        Text { anchors.centerIn: parent; text: parent.label; color: window.ink; font: window.font }
    }
    component Placeholder: Column {
        property string label
        width: parent.width; spacing: 14; opacity: 0.38
        RowLayout {
            width: parent.width
            Text { text: label; color: window.ink; font: window.font; Layout.fillWidth: true }
            Text { text: "—"; color: window.muted; font: window.font }
        }
        Rectangle { width: parent.width; height: 2; color: window.muted
            Rectangle { anchors.centerIn: parent; width: 7; height: 7; color: window.muted }
        }
    }
    ColumnLayout {
        anchors.fill: parent; spacing: 0
        Rectangle {
            Layout.fillWidth: true; Layout.preferredHeight: 48; color: "#1e1e2e"
            RowLayout {
                anchors.fill: parent; anchors.leftMargin: 18; anchors.rightMargin: 18; spacing: 12
                Image { id: logo; source: assetsRoot + "logo/omalux-logo-light.svg"; Layout.preferredWidth: 92; Layout.preferredHeight: 26; fillMode: Image.PreserveAspectFit
                }
                Item { Layout.fillWidth: true }
                GhostButton { label: "[O] OPEN" }
                GhostButton { label: "[S] SAVE" }
                Text { text: "ZOOM"; color: window.muted; font: window.font }
                Rectangle { Layout.preferredWidth: 90; height: 2; color: window.line }
                Text { text: "FIT"; color: window.ink; font: window.font }
                Item { Layout.fillWidth: true }
                Text { text: editor.filename; color: window.muted; font: window.font; elide: Text.ElideMiddle; Layout.maximumWidth: 170 }
            }
            Rectangle { anchors.bottom: parent.bottom; width: parent.width; height: 1; color: window.line }
        }
        RowLayout {
            Layout.fillWidth: true; Layout.fillHeight: true; spacing: 0
            Rectangle {
                Layout.fillWidth: true; Layout.fillHeight: true; color: "#0b0b0d"
                Image { id: photo; anchors.fill: parent; anchors.margins: 20; source: editor.preview; fillMode: Image.PreserveAspectFit; cache: false; asynchronous: false }
                Text { anchors.centerIn: parent; visible: editor.preview === ""; text: editor.status; color: window.muted; font: window.font; width: parent.width - 40; wrapMode: Text.WordWrap; horizontalAlignment: Text.AlignHCenter }
            }
            Rectangle {
                Layout.preferredWidth: 312; Layout.fillHeight: true; color: "#1e1e2e"
                Rectangle { width: 1; height: parent.height; color: window.line }
                ColumnLayout {
                    anchors.fill: parent; anchors.margins: 12; spacing: 14
                    RowLayout {
                        Layout.fillWidth: true; spacing: 0
                        Repeater { model: ["☷", "▧", "⌗", "↶", "ⓘ"]
                            Rectangle { required property string modelData; required property int index
                                Layout.fillWidth: true; height: 26; color: index === 0 ? "#45475a" : "transparent"; border.color: window.line
                                Text { anchors.centerIn: parent; text: modelData; color: index === 0 ? window.accent : window.muted; opacity: index === 0 ? 1 : 0.4 }
                                Rectangle { visible: index === 0; anchors.bottom: parent.bottom; width: parent.width; height: 2; color: window.accent }
                            }
                        }
                    }
                    Rectangle { Layout.fillWidth: true; height: 1; color: window.line }
                    ScrollView {
                        Layout.fillWidth: true; Layout.fillHeight: true; clip: true
                        contentWidth: availableWidth
                        Column {
                            width: parent.width; spacing: 28; padding: 10
                            Text { text: "FILTERS"; color: window.ink; font.bold: true; font.letterSpacing: 2 }
                            Text { text: "01 / BASICS"; color: window.accent; font.bold: true; font.pixelSize: 15; font.letterSpacing: 1 }
                            Placeholder { width: parent.width - 20; label: "EXPOSURE" }
                            Column {
                                width: parent.width - 20; spacing: 8
                                RowLayout {
                                    width: parent.width
                                    Text { text: "BRIGHTNESS"; color: window.accent; font: window.font; Layout.fillWidth: true }
                                    Text { text: Math.round(editor.brightness); color: window.accent; font: window.font }
                                }
                                Slider {
                                    id: brightness; width: parent.width; from: -100; to: 100; stepSize: 1; value: editor.brightness
                                    onMoved: editor.brightness = value
                                    background: Rectangle { x: brightness.leftPadding; y: brightness.topPadding + brightness.availableHeight / 2; width: brightness.availableWidth; height: 2; color: window.muted }
                                    handle: Rectangle { x: brightness.leftPadding + brightness.visualPosition * (brightness.availableWidth - width); y: brightness.topPadding + brightness.availableHeight / 2 - height / 2; width: 8; height: 8; color: brightness.pressed ? "#ffffff" : window.accent }
                                    Accessible.name: "Brightness"
                                }
                            }
                            Placeholder { width: parent.width - 20; label: "CONTRAST" }
                            Placeholder { width: parent.width - 20; label: "CLARITY" }
                            Placeholder { width: parent.width - 20; label: "HIGHLIGHTS" }
                            Placeholder { width: parent.width - 20; label: "SHADOWS" }
                            Placeholder { width: parent.width - 20; label: "WHITES" }
                            Placeholder { width: parent.width - 20; label: "BLACKS" }
                            Text { text: "02 / COLOR"; color: window.accent; font.bold: true; font.pixelSize: 15 }
                            Placeholder { width: parent.width - 20; label: "SATURATION" }
                        }
                    }
                    GhostButton { label: "SAVE AS PRESET…"; Layout.fillWidth: true }
                }
            }
        }
        Rectangle {
            Layout.fillWidth: true; Layout.preferredHeight: 28; color: "#242438"
            RowLayout { anchors.fill: parent; anchors.leftMargin: 12; anchors.rightMargin: 12
                Text { text: "[←/→] BRIGHTNESS   [R] RESET"; color: window.muted; font: window.font }
                Item { Layout.fillWidth: true }
                Text { text: editor.status; color: window.ink; font: window.font }
            }
        }
    }
    Shortcut { sequence: "R"; onActivated: editor.brightness = 0 }
    Shortcut { sequence: "Left"; onActivated: editor.brightness -= 1 }
    Shortcut { sequence: "Right"; onActivated: editor.brightness += 1 }
}
