import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import "../components"

ColumnLayout {
    id: root
    required property var theme
    required property var entries
    required property bool ready
    required property bool busy
    signal stepRequested(int step)
    spacing: 12

    RowLayout {
        Layout.fillWidth: true
        Text { text: "HISTORY"; color: root.theme.ink; font.bold: true; font.letterSpacing: 2 }
        Item { Layout.fillWidth: true }
        Text { text: Math.max(0, root.entries.length - 1); color: root.theme.muted; font: root.theme.textFont }
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
