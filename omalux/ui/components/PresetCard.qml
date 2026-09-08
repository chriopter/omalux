import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

Rectangle {
    id: root
    required property var theme
    required property bool photoReady
    required property bool busy
    required property string appliedStyle
    required property string applyingPreset
    required property bool expanded
    signal exportRequested()
    signal deleteRequested()
    signal applyRequested
    signal detailsToggleRequested
    required property var preset
    width: parent.width
    implicitHeight: cardContent.implicitHeight + 8
    color: "transparent"
    border.color: detailsToggle.checked ? root.theme.line : "transparent"
    radius: 4
    Column {
        id: cardContent
        anchors.top: parent.top
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.margins: 4
        spacing: 10
        RowLayout {
            width: parent.width
            spacing: 6
            Button {
                id: applyPresetButton
                objectName: "preset-apply-" + root.preset.id
                Layout.fillWidth: true
                implicitHeight: 64
                padding: 0
                enabled: !root.busy && root.photoReady && root.preset.error === ""
                onClicked: root.applyRequested()
                Accessible.name: "Apply " + root.preset.name
                ToolTip.visible: hovered
                ToolTip.text: root.preset.error || "Apply " + root.preset.name
                background: Rectangle {
                    color: parent.hovered || parent.down ? "#313244" : "transparent"
                    border.color: parent.activeFocus ? root.theme.accent : "transparent"
                    radius: 3
                }
                contentItem: RowLayout {
                    spacing: 12
                    Rectangle {
                        Layout.preferredWidth: 96
                        Layout.preferredHeight: 64
                        color: "#11111b"
                        Image {
                            id: presetPreview
                            objectName: "presetPreview-" + root.preset.id
                            anchors.fill: parent
                            source: root.preset.previewUrl
                            fillMode: Image.PreserveAspectFit
                            asynchronous: true
                            smooth: true
                            sourceSize.width: 192
                            sourceSize.height: 128
                        }
                        Text {
                            anchors.centerIn: parent
                            visible: presetPreview.status !== Image.Ready
                            text: "No preview"
                            font: root.theme.textFont
                            color: root.theme.muted
                        }
                    }
                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: 5
                        Text {
                            Layout.fillWidth: true
                            text: root.preset.name
                            color: root.appliedStyle === root.preset.name ? root.theme.accent : root.theme.ink
                            font.family: root.theme.textFont.family
                            font.pixelSize: 11
                            font.bold: root.appliedStyle === root.preset.name
                            wrapMode: Text.Wrap
                            maximumLineCount: 2
                            elide: Text.ElideRight
                        }
                        Text {
                            Layout.fillWidth: true
                            font: root.theme.textFont
                            color: root.theme.muted
                            wrapMode: Text.WordWrap
                            text: root.preset.error !== "" ? "Unavailable" : root.busy && root.applyingPreset === root.preset.id ? "Applying…" : root.appliedStyle === root.preset.name ? "Last applied" : "Apply preset"
                        }
                    }
                }
            }
            ToolButton {
                text: "⋮"; implicitWidth: 24
                onClicked: presetMenu.open()
                Accessible.name: root.preset.name + " actions"
                Menu {
                    id: presetMenu
                    MenuItem { text: "Export bundle…"; onTriggered: root.exportRequested() }
                    MenuItem { text: "Delete…"; enabled: root.preset.id.startsWith("my-presets/"); onTriggered: root.deleteRequested() }
                }
            }
            Button {
                id: detailsToggle
                objectName: "preset-toggle-" + root.preset.id
                Layout.preferredWidth: 24
                checkable: true
                text: checked ? "▾" : "▸"
                checked: root.expanded
                onToggled: root.detailsToggleRequested()
                Accessible.name: root.preset.name + " details"
                ToolTip.visible: hovered
                ToolTip.text: checked ? "Hide settings" : "Show applied settings"
                background: Rectangle {
                    color: parent.hovered ? "#313244" : "transparent"
                    border.color: parent.activeFocus ? root.theme.accent : "transparent"
                    radius: 3
                }
                contentItem: Text {
                    text: parent.text
                    color: root.theme.muted
                    font: root.theme.textFont
                    horizontalAlignment: Text.AlignHCenter
                    verticalAlignment: Text.AlignVCenter
                }
            }
        }
        Column {
            visible: detailsToggle.checked
            width: parent.width
            spacing: 12
            Text {
                width: parent.width
                text: root.preset.description
                visible: text !== ""
                wrapMode: Text.WordWrap
                font: root.theme.textFont
                color: root.theme.ink
            }
            Text {
                width: parent.width
                text: root.preset.error
                visible: text !== ""
                wrapMode: Text.WrapAnywhere
                font: root.theme.textFont
                color: "#f9d58b"
            }
            Repeater {
                model: root.preset.modules
                delegate: Column {
                    required property var modelData
                    width: parent.width
                    spacing: 7
                    Rectangle {
                        width: parent.width
                        height: 1
                        color: root.theme.line
                    }
                    Text {
                        width: parent.width
                        wrapMode: Text.WordWrap
                        text: modelData.name + (modelData.instance ? " · " + modelData.instance : "") + (modelData.enabled ? " · on" : " · off")
                        color: root.theme.accent
                        font: root.theme.textFont
                    }
                    Repeater {
                        model: modelData.settings
                        delegate: RowLayout {
                            required property var modelData
                            width: parent.width
                            spacing: 10
                            Text {
                                text: modelData.label
                                Layout.fillWidth: true
                                Layout.preferredWidth: 140
                                Layout.alignment: Qt.AlignTop
                                wrapMode: Text.WordWrap
                                font: root.theme.textFont
                                color: root.theme.muted
                            }
                            Text {
                                text: modelData.display
                                Layout.preferredWidth: 90
                                Layout.maximumWidth: parent.width * 0.45
                                Layout.alignment: Qt.AlignTop
                                wrapMode: Text.WrapAnywhere
                                horizontalAlignment: Text.AlignRight
                                font: root.theme.textFont
                                color: root.theme.ink
                            }
                        }
                    }
                    Text {
                        width: parent.width
                        text: modelData.blending || ""
                        color: root.theme.muted
                        font: root.theme.textFont
                        wrapMode: Text.WordWrap
                    }
                }
            }
        }
    }
}
