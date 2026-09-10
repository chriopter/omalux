import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import "panels"

Rectangle {
    id: root
    required property var theme
    required property var backend
    required property string activeControl
    required property url iconsRoot
    property alias geometry: geometryPanel
    onSelectedPanelChanged: { if (selectedPanel !== 2) geometryPanel.cancel(); if (selectedPanel === 2) controlSelected("rotation"); else if (selectedPanel === 0 && activeControl === "rotation") controlSelected("exposure") }
    property int selectedPanel: 0
    readonly property bool textEditing: selectedPanel === 1 && stylesPanel.textEditing
    signal styleSaveRequested()
    signal styleExportRequested(string id)
    signal styleDeleteRequested(string id, string name)
    signal controlSelected(string id)

    function navigateControl(direction) { filtersPanel.navigate(direction) }
    function revealControl(id) { selectedPanel = 0; controlSelected(id); filtersPanel.reveal(id) }
    function toggleGrainDetails() { filtersPanel.toggleGrainDetails() }
    function showStyleDetails(id) {
        selectedPanel = 1;
        stylesPanel.showDetails(id);
    }

    implicitWidth: 352
    color: theme.background
    Rectangle {
        width: 1
        height: parent.height
        color: root.theme.line
    }
    ColumnLayout {
        anchors.fill: parent
        anchors.margins: 12
        spacing: 14
        RowLayout {
            Layout.fillWidth: true
            spacing: 0
            Repeater {
                model: [
                    {
                        icon: "edit.svg",
                        name: "Filters"
                    },
                    {
                        icon: "styles.svg",
                        name: "Styles"
                    },
                    {
                        icon: "crop.svg",
                        name: "Crop & Rotate"
                    },
                    {
                        icon: "history.svg",
                        name: "History"
                    },
                    {
                        icon: "info.svg",
                        name: "Info"
                    }
                ]
                Button {
                    id: tab
                    required property var modelData
                    required property int index
                    objectName: "sidebar-tab-" + index
                    Layout.fillWidth: true
                    implicitHeight: 26
                    padding: 0
                    onClicked: root.selectedPanel = index
                    Accessible.name: modelData.name
                    Accessible.role: Accessible.PageTab
                    Accessible.selected: root.selectedPanel === index
                    ToolTip.visible: hovered
                    ToolTip.text: modelData.name
                    background: Rectangle {
                        color: root.selectedPanel === tab.index ? root.theme.line : "transparent"
                        border.color: tab.activeFocus ? root.theme.accent : root.theme.line
                        Rectangle {
                            visible: root.selectedPanel === tab.index
                            anchors.bottom: parent.bottom
                            width: parent.width
                            height: 2
                            color: root.theme.accent
                        }
                    }
                    display: AbstractButton.IconOnly
                    icon.source: root.iconsRoot + modelData.icon
                    icon.width: 16; icon.height: 16
                    icon.color: root.selectedPanel === index ? root.theme.accent : root.theme.muted

                }
            }
        }
        Rectangle {
            Layout.fillWidth: true
            height: 1
            color: root.theme.line
        }
        FiltersPanel {
            id: filtersPanel
            onInteractionChanged: active => root.backend.setInteractive(active)
            onHalationRequested: root.backend.applyHalation()
            activeControl: root.activeControl
            onControlReset: id => root.backend.resetControl(id)
            Layout.fillWidth: true
            Layout.fillHeight: true
            visible: root.selectedPanel === 0
            theme: root.theme
            controls: root.backend.controls
            values: root.backend.controlValues
            editable: !root.backend.styleBusy && root.backend.preview !== ""
            onControlSelected: id => root.controlSelected(id)
            onControlEdited: (id, value) => root.backend.setControl(id, value)
        }
        StylesPanel {
            id: stylesPanel
            onPreviewRequested: (id, active) => root.backend.hoverStyle(id, active)
            onSaveRequested: root.styleSaveRequested()
            onExportRequested: id => root.styleExportRequested(id)
            onDeleteRequested: (id, name) => root.styleDeleteRequested(id, name)
            Layout.fillWidth: true
            Layout.fillHeight: true
            visible: root.selectedPanel === 1
            theme: root.theme
            styles: root.backend.styles
            stylesReady: root.backend.stylesReady
            photoReady: root.backend.preview !== ""
            busy: root.backend.styleBusy
            appliedStyle: root.backend.activeStyle
            applyingStyle: root.backend.applyingStyle
            errorMessage: root.backend.styleError
            onApplyRequested: id => root.backend.applyStyle(id)
        }
        GeometryPanel {
            id: geometryPanel
            onInteractionChanged: active => root.backend.setInteractive(active)
            imageAspect: root.backend.metadata.width / Math.max(1,root.backend.metadata.height)
            visible: root.selectedPanel === 2
            Layout.fillWidth: true; Layout.fillHeight: true
            theme: root.theme; controls: root.backend.controls; values: root.backend.controlValues
            editable: !root.backend.styleBusy && root.backend.preview !== ""
            onEdited: (id, value) => root.backend.setControl(id, value)
            onCropApplied: values => root.backend.setControls(values)
        }
        HistoryPanel {
            visible: root.selectedPanel === 3
            Layout.fillWidth: true
            Layout.fillHeight: true
            theme: root.theme
            entries: root.backend.history
            cameraDefaults: root.backend.cameraDefaults
            busy: root.backend.styleBusy
            onStepRequested: step => root.backend.selectHistory(step)
            ready: root.backend.preview !== ""
        }
        MetadataPanel {
            visible: root.selectedPanel === 4
            Layout.fillWidth: true; Layout.fillHeight: true
            theme: root.theme; metadata: root.backend.metadata
        }
    }
}
