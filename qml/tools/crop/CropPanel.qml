import QtQuick
import QtQuick.Layouts

Item {
    id: panel
    required property var theme

    ColumnLayout {
        anchors.fill: parent
        spacing: 12

        Text {
            text: "01 / CROP"
            color: panel.theme.inkColor
            font.family: panel.theme.monoFont
            font.pixelSize: 13
            font.bold: true
            font.letterSpacing: 1
        }

        Rectangle {
            Layout.fillWidth: true
            Layout.preferredHeight: 220
            color: panel.theme.surfaceColor
            border.width: 1
            border.color: panel.theme.lineColor

            Column {
                anchors.centerIn: parent
                spacing: 9

                Text {
                    anchors.horizontalCenter: parent.horizontalCenter
                    text: "┌──────────────┐\n│              │\n│  CROP TOOL   │\n│              │\n└──────────────┘"
                    color: panel.theme.mutedColor
                    font.family: panel.theme.monoFont
                    font.pixelSize: 12
                    horizontalAlignment: Text.AlignHCenter
                }

                Text {
                    anchors.horizontalCenter: parent.horizontalCenter
                    text: "PLACEHOLDER / NEXT ITERATION"
                    color: panel.theme.accentColor
                    font.family: panel.theme.monoFont
                    font.pixelSize: 10
                    font.bold: true
                }
            }
        }

        Text {
            Layout.fillWidth: true
            text: "PLANNED\n\n• FREE CROP\n• ASPECT RATIOS\n• ROTATE / STRAIGHTEN"
            color: panel.theme.mutedColor
            font.family: panel.theme.monoFont
            font.pixelSize: 11
            lineHeight: 1.45
        }

        Item { Layout.fillHeight: true }
    }
}
