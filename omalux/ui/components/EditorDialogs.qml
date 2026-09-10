import QtQuick
import QtQuick.Controls
import QtQuick.Dialogs
import QtQuick.Layouts

Item {
    id: root
    required property var theme
    property string styleId: ""
    property string styleName: ""
    readonly property bool busy: openDialog.visible || exportDialog.visible || exportOptions.visible || saveStyleDialog.visible || deleteDialog.visible || bundleDialog.visible
    signal openRequested(url file)
    signal exportRequested(url file, int quality)
    signal styleSaveRequested(string name)
    signal styleDeleteRequested(string id)
    signal styleExportRequested(string id, url directory)
    function openImage() { openDialog.open() }
    function exportImage() { exportOptions.open() }
    function saveStyle() { nameInput.text="";saveStyleDialog.open() }
    function deleteStyle(id, name) { styleId=id;styleName=name;deleteDialog.open() }
    function exportStyle(id) { styleId=id;bundleDialog.open() }
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
        id: saveStyleDialog
        title: "Save style"
        anchors.centerIn: Overlay.overlay
        modal: true
        standardButtons: Dialog.Save | Dialog.Cancel
        onAccepted: if (nameInput.text.trim()) root.styleSaveRequested(nameInput.text.trim())
        TextField { id: nameInput; width: 300; placeholderText: "Style name"; onAccepted: saveStyleDialog.accept() }
        onOpened: nameInput.forceActiveFocus()
    }
    Dialog {
        id: deleteDialog
        title: "Delete style?"
        anchors.centerIn: Overlay.overlay
        modal: true
        standardButtons: Dialog.Yes | Dialog.No
        contentItem: Text { text: root.styleName; color: root.theme.ink }
        onAccepted: root.styleDeleteRequested(root.styleId)
    }
    FolderDialog {
        id: bundleDialog
        title: "Export style bundle into this folder"
        onAccepted: root.styleExportRequested(root.styleId, selectedFolder)
    }
}
