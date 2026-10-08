import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

// The keyboard reference (?): every row of the binding table, grouped by section.
Dialog {
    id: root
    required property var theme
    required property var shortcuts     // EditorShortcuts
    title: "Keyboard reference"
    modal: true
    standardButtons: Dialog.Close
    ScrollView {
        id: scroll
        anchors.fill: parent
        contentWidth: availableWidth
        Column {
            width: scroll.availableWidth
            spacing: 4
            Repeater {
                model: root.shortcuts.sections
                Column {
                    id: section
                    required property string modelData
                    width: parent.width
                    spacing: 3
                    Text {
                        text: section.modelData
                        color: root.theme.accent
                        font: root.theme.moduleHeadingFont
                        topPadding: 10
                        bottomPadding: 2
                    }
                    Repeater {
                        model: root.shortcuts.bindings.filter(b => b.section === section.modelData)
                        RowLayout {
                            required property var modelData
                            width: section.width
                            spacing: 12
                            Text {
                                Layout.preferredWidth: 190
                                Layout.alignment: Qt.AlignTop
                                text: modelData.keys.length > 5 ? root.shortcuts.display(modelData.keys[0]) + " … " + root.shortcuts.display(modelData.keys[modelData.keys.length - 1])
                                                                : root.shortcuts.displayKeys(modelData)
                                color: root.theme.ink
                                font: root.theme.textFont
                                wrapMode: Text.WordWrap
                            }
                            Text {
                                Layout.fillWidth: true
                                text: modelData.label
                                color: root.theme.muted
                                font: root.theme.textFont
                                wrapMode: Text.WordWrap
                            }
                        }
                    }
                }
            }
        }
    }
}
