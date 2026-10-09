import QtQuick
import QtQuick.Controls
import QtQuick.Dialogs
import QtQuick.Layouts
import QtCore

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
    function openImage() { if (photoFolder !== "") openDialog.currentFolder = fileUrl(photoFolder); openDialog.open() }
    function exportImage() { exportOptions.open() }
    function saveStyle() { nameInput.text="";saveStyleDialog.open() }
    function deleteStyle(id, name) { styleId=id;styleName=name;deleteDialog.open() }
    function exportStyle(id) { styleId=id;bundleDialog.open() }
    // The photograph's own file: Open starts in its folder, Export proposes its name there.
    property string photoPath: ""
    readonly property string photoFolder: photoPath.lastIndexOf("/") > 0 ? photoPath.slice(0, photoPath.lastIndexOf("/")) : ""
    readonly property string photoBase: {
        const name = photoPath.slice(photoPath.lastIndexOf("/") + 1)
        return name.lastIndexOf(".") > 0 ? name.slice(0, name.lastIndexOf(".")) : name
    }
    // Enter in a dialog: the focused button if there is one (Cancel stays Cancel), otherwise
    // the confirming button.
    function confirm(dialog) {
        const focused = root.Window.window ? root.Window.window.activeFocusItem : null
        if (focused && focused.down !== undefined && typeof focused.clicked === "function") focused.clicked()
        else dialog.accept()
    }
    function fileUrl(path) { return "file://" + path.split("/").map(encodeURIComponent).join("/") }
    // The last export choice returns with the next start.
    Settings {
        id: exportSettings
        category: "Export"
        property string format: "jpg"
        property int quality: 90
    }
    FileDialog {
        id: openDialog
        title: "Open photograph"
        nameFilters: ["Photographs (*.jpg *.jpeg *.png *.tif *.tiff *.dng *.cr2 *.cr3 *.nef *.arw *.raf *.rw2 *.orf *.pef *.srw *.x3f *.3fr *.iiq *.erf *.mrw *.nrw *.heic *.heif *.avif *.webp *.jxl *.exr *.hdr *.pfm)", "All files (*)"]
        onAccepted: root.openRequested(selectedFile)
    }
    Dialog {
        id: exportOptions
        objectName: "export-options"
        title: "Export photograph"
        anchors.centerIn: Overlay.overlay
        modal: true
        standardButtons: Dialog.Ok | Dialog.Cancel
        readonly property bool jpeg: exportSettings.format !== "png"
        onAboutToShow: quality.value = exportSettings.quality
        onOpened: { const ok = standardButton(Dialog.Ok); if (ok) ok.text = "Choose file…"; exportContent.forceActiveFocus() }
        onAccepted: {
            exportSettings.quality = Math.round(quality.value)
            if (root.photoFolder !== "") {
                exportDialog.currentFolder = root.fileUrl(root.photoFolder)
                exportDialog.selectedFile = root.fileUrl(root.photoFolder + "/" + root.photoBase + "-omalux." + exportSettings.format)
            }
            exportDialog.open()
        }
        contentItem: ColumnLayout {
            id: exportContent
            spacing: 12
            // J and P pick the format wherever the focus is in the dialog; Enter confirms
            // (confirm() below).
            Shortcut { sequence: "J"; enabled: exportOptions.visible; onActivated: exportSettings.format = "jpg" }
            Shortcut { sequence: "P"; enabled: exportOptions.visible; onActivated: exportSettings.format = "png" }
            Shortcut { sequences: ["Return", "Enter"]; enabled: exportOptions.visible; onActivated: root.confirm(exportOptions) }
            RowLayout {
                spacing: 8
                Text { text: "format"; color: root.theme.muted; font: root.theme.textFont; Layout.preferredWidth: 64 }
                Repeater {
                    model: [{ id: "jpg", label: "JPEG", key: "J" }, { id: "png", label: "PNG", key: "P" }]
                    Button {
                        required property var modelData
                        objectName: "export-format-" + modelData.id
                        text: modelData.label
                        font: root.theme.textFont
                        checkable: true
                        autoExclusive: true
                        checked: exportSettings.format === modelData.id
                        onClicked: exportSettings.format = modelData.id
                        Accessible.name: modelData.label + " format"
                        ToolTip.visible: hovered && !pressed
                        ToolTip.delay: 500
                        ToolTip.text: modelData.label + "  [" + modelData.key + "]"
                    }
                }
            }
            RowLayout {
                spacing: 8
                opacity: exportOptions.jpeg ? 1 : .4
                Text { text: "quality"; color: root.theme.muted; font: root.theme.textFont; Layout.preferredWidth: 64 }
                PlainSlider {
                    id: quality
                    objectName: "export-quality"
                    enabled: exportOptions.jpeg
                    from: 1; to: 100; value: 90; stepSize: 1
                    Layout.preferredWidth: 220
                    Accessible.name: "JPEG quality"
                }
                Text {
                    objectName: "export-quality-value"
                    text: exportOptions.jpeg ? Math.round(quality.value) : "—"
                    color: root.theme.ink; font: root.theme.textFont
                    horizontalAlignment: Text.AlignRight
                    Layout.preferredWidth: 28
                }
            }
            Text {
                text: "Full resolution · sRGB · EXIF and colour profile embedded"
                color: root.theme.muted; font: root.theme.textFont
            }
        }
    }
    FileDialog {
        id: exportDialog
        title: "Export photograph"
        fileMode: FileDialog.SaveFile
        nameFilters: exportSettings.format === "png" ? ["PNG (*.png)"] : ["JPEG (*.jpg *.jpeg)"]
        defaultSuffix: exportSettings.format
        onAccepted: root.exportRequested(selectedFile, exportSettings.quality)
    }
    Dialog {
        id: saveStyleDialog
        title: "Save style"
        anchors.centerIn: Overlay.overlay
        modal: true
        standardButtons: Dialog.Save | Dialog.Cancel
        onAccepted: if (nameInput.text.trim()) root.styleSaveRequested(nameInput.text.trim())
        // implicitWidth: the dialog sizes itself to the field (a fixed width stuck out of it).
        TextField { id: nameInput; implicitWidth: 300; font: root.theme.textFont; placeholderText: "Style name"; onAccepted: saveStyleDialog.accept() }
        onOpened: nameInput.forceActiveFocus()
    }
    Dialog {
        id: deleteDialog
        title: "Delete style?"
        anchors.centerIn: Overlay.overlay
        modal: true
        standardButtons: Dialog.Yes | Dialog.No
        // A fixed width for the wrapped line: sizing the dialog to a wrapping text loops.
        contentItem: Item {
            implicitWidth: 320
            implicitHeight: deleteText.implicitHeight
            Shortcut { sequences: ["Return", "Enter"]; enabled: deleteDialog.visible; onActivated: root.confirm(deleteDialog) }
            Text {
                id: deleteText
                width: parent.width
                text: "“" + root.styleName + "” is removed from your styles."
                color: root.theme.ink; font: root.theme.textFont
                wrapMode: Text.WordWrap
            }
        }
        onOpened: contentItem.forceActiveFocus()
        onAccepted: root.styleDeleteRequested(root.styleId)
    }
    FolderDialog {
        id: bundleDialog
        title: "Export style bundle into this folder"
        onAccepted: root.styleExportRequested(root.styleId, selectedFolder)
    }
}
