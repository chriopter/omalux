import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

Item {
    id: panel
    required property var theme
    required property string fileName
    required property string metadataText

    ColumnLayout {
        anchors.fill: parent
        spacing: 12

        Text {
            text: "03 / METADATA"
            color: panel.theme.inkColor
            font.family: panel.theme.monoFont
            font.pixelSize: 13
            font.bold: true
            font.letterSpacing: 1
        }

        Text {
            Layout.fillWidth: true
            text: panel.fileName.length > 0 ? panel.fileName.toUpperCase() : "NO PHOTOGRAPH"
            color: panel.theme.accentColor
            elide: Text.ElideMiddle
            font.family: panel.theme.monoFont
            font.pixelSize: 11
            font.bold: true
        }

        Rectangle {
            Layout.fillWidth: true
            Layout.preferredHeight: 1
            color: panel.theme.lineColor
        }

        ScrollView {
            Layout.fillWidth: true
            Layout.fillHeight: true
            clip: true

            Text {
                width: parent.width
                text: panel.metadataText
                color: panel.fileName.length > 0 ? panel.theme.inkColor : panel.theme.mutedColor
                font.family: panel.theme.monoFont
                font.pixelSize: 11
                lineHeight: 1.65
                wrapMode: Text.WrapAnywhere
            }
        }

        Rectangle {
            Layout.fillWidth: true
            Layout.preferredHeight: 1
            color: panel.theme.lineColor
        }

        Text {
            Layout.fillWidth: true
            text: "EXIF / LIBRAW\nREAD-ONLY"
            color: panel.theme.mutedColor
            font.family: panel.theme.monoFont
            font.pixelSize: 10
            lineHeight: 1.45
        }
    }
}
