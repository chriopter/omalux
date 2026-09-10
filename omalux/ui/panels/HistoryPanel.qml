import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import "../components"

ColumnLayout {
    id: root
    required property var theme
    required property var entries
    required property var cameraDefaults
    required property bool ready
    required property bool busy
    signal stepRequested(int step)
    spacing: 12

    // What darktable set up for this camera before any style: colour, lens, base tone.
    readonly property var cameraGroups: [...new Set(root.cameraDefaults.map(e => e.group))]

    RowLayout {
        Layout.fillWidth: true
        Text { text: "HISTORY"; color: root.theme.ink; font.bold: true; font.letterSpacing: 2 }
        Item { Layout.fillWidth: true }
        Text { text: Math.max(0, root.entries.length - 1); color: root.theme.muted; font: root.theme.textFont }
    }
    Column {
        id: cameraSection
        objectName: "cameraDefaults"
        Layout.fillWidth: true
        visible: root.cameraDefaults.length > 0
        spacing: 6
        Rectangle {
            width: parent.width
            height: cameraColumn.implicitHeight + 20
            color: Qt.lighter(root.theme.background, 1.16)
            radius: 4
            Column {
                id: cameraColumn
                x: 10; y: 10
                width: parent.width - 20
                spacing: 8
                Text {
                    text: "Applied for this camera"
                    color: root.theme.ink
                    font.family: root.theme.textFont.family
                    font.pixelSize: root.theme.textFont.pixelSize
                    font.bold: true
                }
                Repeater {
                    model: root.cameraGroups
                    Column {
                        id: group
                        required property string modelData
                        width: cameraColumn.width
                        spacing: 2
                        Text {
                            text: group.modelData
                            color: root.theme.muted
                            font.family: root.theme.textFont.family
                            font.pixelSize: root.theme.textFont.pixelSize
                            font.letterSpacing: 1
                            bottomPadding: 1
                        }
                        Repeater {
                            model: root.cameraDefaults.filter(e => e.group === group.modelData)
                            Item {
                                required property var modelData
                                width: group.width
                                implicitHeight: Math.max(entryLabel.implicitHeight, entryValue.implicitHeight) + 3
                                Rectangle {
                                    id: dot
                                    width: 5; height: 5; radius: 2.5
                                    anchors.top: parent.top
                                    anchors.topMargin: Math.round((entryLabel.implicitHeight - height) / 2)
                                    color: modelData.enabled ? root.theme.accent : root.theme.muted
                                    opacity: modelData.enabled ? 1 : .5
                                }
                                Text {
                                    id: entryLabel
                                    anchors.left: dot.right; anchors.leftMargin: 8
                                    anchors.top: parent.top
                                    text: modelData.label
                                    color: root.theme.ink
                                    font: root.theme.textFont
                                    opacity: modelData.enabled ? 1 : .55
                                }
                                Text {
                                    id: entryValue
                                    anchors.right: parent.right
                                    anchors.top: parent.top
                                    width: Math.min(implicitWidth, group.width - entryLabel.implicitWidth - 26)
                                    horizontalAlignment: Text.AlignRight
                                    wrapMode: Text.WordWrap
                                    text: modelData.value
                                    color: modelData.enabled ? root.theme.ink : root.theme.muted
                                    font: root.theme.textFont
                                    opacity: modelData.enabled ? 1 : .55
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    Text {
        Layout.fillWidth: true
        text: "Processing steps · newest first"
        color: root.theme.muted
        font: root.theme.textFont
        wrapMode: Text.WordWrap
    }
    ListView {
        id: list
        Layout.fillWidth: true
        Layout.fillHeight: true
        clip: true
        spacing: 6
        model: root.entries
        boundsBehavior: Flickable.StopAtBounds
        flickableDirection: Flickable.VerticalFlick
        ScrollBar.vertical: ScrollBar {}
        SidebarWheelHandler { flickable: list }
        delegate: ItemDelegate {
            id: entry
            objectName: "history-step-" + modelData.step
            required property var modelData
            width: list.width
            implicitHeight: row.implicitHeight + 20
            enabled: root.ready && !root.busy
            onClicked: root.stepRequested(modelData.step)
            background: Rectangle {
                color: entry.modelData.current ? root.theme.line : entry.hovered ? "#313244" : "transparent"
                border.color: entry.activeFocus ? root.theme.accent : "transparent"
            }
            opacity: modelData.active ? 1 : .45
            Accessible.name: modelData.step + ": " + modelData.label + (modelData.enabled ? ", on" : ", off")
            RowLayout {
                id: row
                anchors.fill: parent
                anchors.margins: 10
                spacing: 10
                Text { text: modelData.step; color: root.theme.muted; font: root.theme.textFont }
                ColumnLayout {
                    Layout.fillWidth: true
                    spacing: 4
                    Text {
                        Layout.fillWidth: true
                        text: modelData.label
                        // darktable returns escaped module/instance labels.
                        textFormat: Text.StyledText
                        wrapMode: Text.WordWrap
                        color: modelData.current ? root.theme.accent : root.theme.ink
                        font: root.theme.textFont
                    }
                    Text {
                        text: (modelData.step === 0 ? "original" : modelData.enabled ? "on" : "off") + (modelData.current ? " · current" : "")
                        color: root.theme.muted
                        font: root.theme.textFont
                    }
                }
            }
        }
        Text {
            anchors.centerIn: parent
            width: parent.width
            visible: root.entries.length === 0
            text: root.ready ? "No processing steps yet." : "Loading history…"
            color: root.theme.muted
            font: root.theme.textFont
            horizontalAlignment: Text.AlignHCenter
            wrapMode: Text.WordWrap
        }
    }
}
