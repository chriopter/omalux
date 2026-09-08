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
    onSelectedPanelChanged: { if (selectedPanel !== 2) geometryPanel.cancel(); if (selectedPanel === 2) controlSelected("rotation"); else if (selectedPanel === 0 && activeControl === "rotation") controlSelected("brightness") }
    property int selectedPanel: 0
    readonly property bool textEditing: selectedPanel === 1 && presetsPanel.textEditing
    signal presetSaveRequested()
    signal presetExportRequested(string id)
    signal presetDeleteRequested(string id, string name)
    signal controlSelected(string id)

    function navigateControl(direction) { filtersPanel.navigate(direction) }
    function revealControl(id) { selectedPanel = 0; controlSelected(id); filtersPanel.reveal(id) }
    function toggleGrainDetails() { let next=Object.assign({}, filtersPanel.expandedDetails); next.grain=!next.grain; filtersPanel.expandedDetails=next }
    function showPresetDetails(id) {
        selectedPanel = 1;
        presetsPanel.showDetails(id);
    }

    implicitWidth: 312
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
                        icon: "presets.svg",
                        name: "Presets"
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
        PresetsPanel {
            id: presetsPanel
            onSaveRequested: root.presetSaveRequested()
            onExportRequested: id => root.presetExportRequested(id)
            onDeleteRequested: (id, name) => root.presetDeleteRequested(id, name)
            Layout.fillWidth: true
            Layout.fillHeight: true
            visible: root.selectedPanel === 1
            theme: root.theme
            presets: root.backend.presets
            presetsReady: root.backend.presetsReady
            photoReady: root.backend.preview !== ""
            busy: root.backend.styleBusy
            appliedStyle: root.backend.activeStyle
            applyingPreset: root.backend.applyingPreset
            errorMessage: root.backend.presetError
            onApplyRequested: id => root.backend.applyPreset(id)
        }
        GeometryPanel {
            id: geometryPanel
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
