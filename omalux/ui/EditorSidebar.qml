import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import "panels"

Rectangle {
    id: root
    required property var theme
    required property var backend
    property int selectedPanel: 0
    readonly property bool textEditing: selectedPanel === 1 && presetsPanel.textEditing
    signal controlSelected(string id)

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
                        icon: "☷",
                        name: "Filters"
                    },
                    {
                        icon: "▧",
                        name: "Presets"
                    },
                    {
                        icon: "⌗",
                        name: "Crop & Rotate"
                    },
                    {
                        icon: "↶",
                        name: "History"
                    },
                    {
                        icon: "ⓘ",
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
                    enabled: index < 2
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
                    contentItem: Text {
                        text: tab.modelData.icon
                        horizontalAlignment: Text.AlignHCenter
                        verticalAlignment: Text.AlignVCenter
                        color: root.selectedPanel === tab.index ? root.theme.accent : root.theme.muted
                        opacity: tab.enabled ? 1 : 0.4
                    }
                }
            }
        }
        Rectangle {
            Layout.fillWidth: true
            height: 1
            color: root.theme.line
        }
        FiltersPanel {
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
    }
}
