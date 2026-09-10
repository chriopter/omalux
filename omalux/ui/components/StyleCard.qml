import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

Rectangle {
    id: root
    required property var theme
    required property bool photoReady
    required property bool busy
    required property string appliedStyle
    required property string applyingStyle
    required property bool expanded
    signal previewRequested(bool active)
    readonly property bool previewHovered: applyStyleButton.hovered && applyStyleButton.enabled && root.visible
    onPreviewHoveredChanged: {
        if (previewHovered) hoverDelay.restart()
        else { hoverDelay.stop(); root.previewRequested(false) }
    }
    Component.onDestruction: root.previewRequested(false)
    Timer { id: hoverDelay; interval: 150; onTriggered: if (root.previewHovered) root.previewRequested(true) }
    signal exportRequested()
    signal deleteRequested()
    signal applyRequested
    signal detailsToggleRequested
    required property var style
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
                id: applyStyleButton
                objectName: "style-apply-" + root.style.id
                Layout.fillWidth: true
                implicitHeight: 64
                padding: 0
                enabled: !root.busy && root.photoReady && root.style.error === ""
                hoverEnabled: true
                onClicked: { hoverDelay.stop(); root.previewRequested(false); root.applyRequested() }
                Accessible.name: "Apply " + root.style.name
                ToolTip.visible: hovered && root.style.error !== ""
                ToolTip.text: root.style.error
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
                            id: stylePreview
                            objectName: "stylePreview-" + root.style.id
                            anchors.fill: parent
                            source: root.style.previewUrl
                            fillMode: Image.PreserveAspectFit
                            asynchronous: true
                            smooth: true
                            sourceSize.width: 192
                            sourceSize.height: 128
                        }
                        Text {
                            anchors.centerIn: parent
                            visible: stylePreview.status !== Image.Ready
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
                            text: root.style.name
                            color: root.appliedStyle === root.style.name ? root.theme.accent : root.theme.ink
                            font.family: root.theme.textFont.family
                            font.pixelSize: 11
                            font.bold: root.appliedStyle === root.style.name
                            wrapMode: Text.Wrap
                            maximumLineCount: 2
                            elide: Text.ElideRight
                        }
                        Text {
                            Layout.fillWidth: true
                            font: root.theme.textFont
                            color: root.theme.muted
                            wrapMode: Text.WordWrap
                            text: root.style.error !== "" ? "Unavailable" : root.busy && root.applyingStyle === root.style.id ? "Applying…" : root.appliedStyle === root.style.name ? "Last applied" : "Apply style"
                        }
                    }
                }
            }
            ToolButton {
                text: "⋮"; implicitWidth: 24
                onClicked: styleMenu.open()
                Accessible.name: root.style.name + " actions"
                Menu {
                    id: styleMenu
                    MenuItem { text: "Export bundle…"; onTriggered: root.exportRequested() }
                    MenuItem { text: "Delete…"; enabled: root.style.id.startsWith("my-styles/"); onTriggered: root.deleteRequested() }
                }
            }
            Button {
                id: detailsToggle
                objectName: "style-toggle-" + root.style.id
                Layout.preferredWidth: 24
                checkable: true
                text: checked ? "▾" : "▸"
                checked: root.expanded
                onToggled: root.detailsToggleRequested()
                Accessible.name: root.style.name + " details"
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
                text: root.style.description
                visible: text !== ""
                wrapMode: Text.WordWrap
                font: root.theme.textFont
                color: root.theme.ink
            }
            Text {
                width: parent.width
                text: root.style.error
                visible: text !== ""
                wrapMode: Text.WrapAnywhere
                font: root.theme.textFont
                color: "#f9d58b"
            }
            Repeater {
                model: root.style.modules
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
