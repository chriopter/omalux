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
    // Enter closes it like Escape; the arrows and Page keys scroll.
    onOpened: scroll.forceActiveFocus()
    ScrollView {
        id: scroll
        anchors.fill: parent
        contentWidth: availableWidth
        clip: true
        Keys.onReturnPressed: root.close()
        Keys.onEnterPressed: root.close()
        Keys.onPressed: event => {
            const flick = scroll.contentItem
            const limit = Math.max(0, flick.contentHeight - flick.height)
            let y = flick.contentY
            if (event.key === Qt.Key_Down || event.key === Qt.Key_J) y += 40
            else if (event.key === Qt.Key_Up || event.key === Qt.Key_K) y -= 40
            else if (event.key === Qt.Key_PageDown || event.key === Qt.Key_Space) y += flick.height - 40
            else if (event.key === Qt.Key_PageUp) y -= flick.height - 40
            else if (event.key === Qt.Key_Home) y = 0
            else if (event.key === Qt.Key_End) y = limit
            else if (event.key === Qt.Key_Question || event.key === Qt.Key_F1) { root.close(); event.accepted = true; return }
            else return
            flick.contentY = Math.max(0, Math.min(limit, y))
            event.accepted = true
        }
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
