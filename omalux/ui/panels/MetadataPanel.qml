import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import "../components"

SidebarScrollView {
    id: root
    required property var theme
    required property var metadata
    Column {
        width: root.availableWidth - 20; x: 10; spacing: 18
        Text { text: "METADATA"; color: root.theme.ink; font.bold: true }
        Repeater {
            model: Object.keys(root.metadata)
            Column {
                required property string modelData
                width: parent.width; spacing: 5
                Text { text: modelData; font: root.theme.textFont; color: root.theme.muted }
                Text { width: parent.width; text: String(root.metadata[modelData] || "—"); wrapMode: Text.WrapAnywhere; font: root.theme.textFont; color: root.theme.ink }
            }
        }
    }
}
