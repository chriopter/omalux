import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

// The settings a look applies, shown under its family's grid: description, any error, and per
// darktable module its displayed settings and blending.
Rectangle {
    id: root
    required property var theme
    required property var style
    signal closeRequested()
    implicitHeight: content.implicitHeight + 16
    radius: 4
    color: "transparent"
    border.color: root.theme.line
    Column {
        id: content
        x: 8; y: 8
        width: parent.width - 16
        spacing: 10
        RowLayout {
            width: parent.width
            Text {
                Layout.fillWidth: true
                text: root.style.name
                color: root.theme.ink; font: root.theme.moduleHeadingFont
                elide: Text.ElideRight
            }
            ToolButton {
                objectName: "style-details-close"
                implicitWidth: 22; implicitHeight: 22; padding: 0
                onClicked: root.closeRequested()
                Accessible.name: "Hide settings of " + root.style.name
                contentItem: Text { text: "×"; color: parent.hovered ? root.theme.ink : root.theme.muted; font: root.theme.settingsFont
                                    horizontalAlignment: Text.AlignHCenter; verticalAlignment: Text.AlignVCenter }
                background: Item {}
            }
        }
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
                Rectangle { width: parent.width; height: 1; color: root.theme.line }
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
                            // Long values (curve nodes) are cut after a few lines.
                            maximumLineCount: 3
                            elide: Text.ElideRight
                            horizontalAlignment: Text.AlignRight
                            font: root.theme.textFont
                            color: root.theme.ink
                        }
                    }
                }
                Text {
                    width: parent.width
                    visible: text !== ""
                    text: modelData.blending || ""
                    color: root.theme.muted
                    font: root.theme.textFont
                    wrapMode: Text.WordWrap
                }
            }
        }
    }
}
