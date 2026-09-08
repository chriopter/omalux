import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import org.omalux

Dialog {
    id: dialog
    required property var theme
    required property var backend
    property string renameId: ""
    property bool updating: false
    modal: true
    anchors.centerIn: parent
    width: Math.min(440, parent.width - 40)
    title: renameId ? "Rename preset" : "Save preset"
    closePolicy: backend.savingPreset ? Popup.NoAutoClose : Popup.CloseOnEscape

    function prompt(name, id, update) {
        renameId = id || ""
        updating = update || false
        nameField.text = name || ""
        backend.presetError = ""
        open()
        nameField.forceActiveFocus()
    }
    function save(mode) {
        if (renameId) {
            if (backend.renamePreset(renameId, nameField.text)) close()
        } else {
            backend.savePreset(nameField.text, mode)
        }
    }
    Connections {
        target: dialog.backend
        function onPresetSaved(id) { dialog.close() }
    }
    contentItem: ColumnLayout {
        spacing: 12
        TextField {
            id: nameField
            Layout.fillWidth: true
            placeholderText: "Preset name"
            maximumLength: 160
            enabled: !dialog.backend.savingPreset
            onAccepted: if (text.trim()) dialog.save(dialog.updating ? "replace" : "new")
        }
        Label {
            Layout.fillWidth: true
            text: dialog.backend.savingPreset ? "Rendering beach preview…" : dialog.backend.presetError
            visible: text.length > 0
            wrapMode: Text.Wrap
        }
        RowLayout {
            enabled: !dialog.backend.savingPreset
            TuiButton {
                theme: dialog.theme
                text: dialog.renameId ? "RENAME" : dialog.updating ? "UPDATE" : "SAVE"
                enabled: nameField.text.trim().length > 0
                onClicked: dialog.save(dialog.updating ? "replace" : "new")
            }
            TuiButton {
                theme: dialog.theme
                text: "CANCEL"
                onClicked: dialog.close()
            }
        }
        RowLayout {
            visible: !dialog.renameId && dialog.backend.presetError.indexOf("Name already exists") === 0
            enabled: !dialog.backend.savingPreset
            TuiButton { theme: dialog.theme; text: "REPLACE"; onClicked: dialog.save("replace") }
            TuiButton { theme: dialog.theme; text: "SAVE COPY"; onClicked: dialog.save("copy") }
        }
    }
}
