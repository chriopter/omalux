import QtQuick
import QtQuick.Controls
import QtQuick.Dialogs
import QtQuick.Layouts

Item {
    id: root
    required property var theme
    property string presetId: ""
    property string presetName: ""
    readonly property bool busy: openDialog.visible || exportDialog.visible || exportOptions.visible || savePresetDialog.visible || deleteDialog.visible || bundleDialog.visible
    signal openRequested(url file)
    signal exportRequested(url file, int quality)
    signal presetSaveRequested(string name)
    signal presetDeleteRequested(string id)
    signal presetExportRequested(string id, url directory)
    function openImage() { openDialog.open() }
    function exportImage() { exportOptions.open() }
    function savePreset() { nameInput.text="";savePresetDialog.open() }
    function deletePreset(id, name) { presetId=id;presetName=name;deleteDialog.open() }
    function exportPreset(id) { presetId=id;bundleDialog.open() }
    FileDialog {
        id: openDialog
        title: "Open photograph"
        nameFilters: ["Photographs (*.jpg *.jpeg *.png *.tif *.tiff *.dng *.cr2 *.cr3 *.nef *.arw *.raf *.rw2 *.orf *.pef)", "All files (*)"]
        onAccepted: root.openRequested(selectedFile)
    }
    Dialog {
        id: exportOptions
        title: "Export photograph"
        anchors.centerIn: Overlay.overlay
        modal: true
        standardButtons: Dialog.Ok | Dialog.Cancel
        onAccepted: exportDialog.open()
        ColumnLayout {
            Text { text: "Full-resolution JPEG or PNG · sRGB"; color: root.theme.ink }
            Text { text: "JPEG quality: " + Math.round(quality.value); color: root.theme.ink }
            Slider { id: quality; from: 1; to: 100; value: 90; stepSize: 1; Layout.preferredWidth: 300 }
        }
    }
    FileDialog {
        id: exportDialog
        title: "Export photograph"
        fileMode: FileDialog.SaveFile
        nameFilters: ["JPEG (*.jpg)", "PNG (*.png)"]
        defaultSuffix: selectedNameFilter.index === 1 ? "png" : "jpg"
        onAccepted: root.exportRequested(selectedFile, Math.round(quality.value))
    }
    Dialog {
        id: savePresetDialog
        title: "Save preset"
        anchors.centerIn: Overlay.overlay
        modal: true
        standardButtons: Dialog.Save | Dialog.Cancel
        onAccepted: if (nameInput.text.trim()) root.presetSaveRequested(nameInput.text.trim())
        TextField { id: nameInput; width: 300; placeholderText: "Preset name"; onAccepted: savePresetDialog.accept() }
        onOpened: nameInput.forceActiveFocus()
    }
    Dialog {
        id: deleteDialog
        title: "Delete preset?"
        anchors.centerIn: Overlay.overlay
        modal: true
        standardButtons: Dialog.Yes | Dialog.No
        contentItem: Text { text: root.presetName; color: root.theme.ink }
        onAccepted: root.presetDeleteRequested(root.presetId)
    }
    FolderDialog {
        id: bundleDialog
        title: "Export preset bundle into this folder"
        onAccepted: root.presetExportRequested(root.presetId, selectedFolder)
    }
}
