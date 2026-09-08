import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

Column {
    id: root
    required property var theme
    required property var section
    required property var values
    required property bool editable
    required property string activeControl
    required property bool expanded
    signal interactionChanged(bool active)
    signal expansionRequested()
    signal controlSelected(string id)
    signal controlEdited(string id, real value)
    signal controlReset(string id)
    signal halationRequested()
    readonly property string enableControl: section.controls[0].module + "_enabled"
    readonly property bool moduleEnabled: values[enableControl] > .5
    readonly property bool hasDetails: section.controls.some(c => !section.primary.includes(c.id))
                                       || section.name === "denoise (profiled)" || section.name === "diffuse or sharpen"
    spacing: 4
    topPadding: headerVisible && section.primary.length !== 1 ? 12 : 0
    bottomPadding: headerVisible ? 8 : 0

    function findControl(id) {
        for (let i = 0; i < rows.count; ++i) {
            const item = rows.itemAt(i)
            if (item.control.id === id && item.visible) return item
        }
        return null
    }
    readonly property bool headerVisible: expanded || section.primary.length !== 1
    Item {
        id: moduleHeader
        visible: root.headerVisible
        x: -14
        width: parent.width + 14
        implicitHeight: Math.max(30, headerContent.implicitHeight + 10)
        // The background spans the module without participating in Column layout.
        Rectangle {
            z: -1
            width: parent.width
            height: root.height - moduleHeader.y
            color: Qt.lighter(root.theme.background, 1.16)
        }
        RowLayout {
            id: headerContent
            anchors.fill: parent
            anchors.leftMargin: 8; anchors.rightMargin: 28
            anchors.topMargin: 5; anchors.bottomMargin: 5
            spacing: 6
            ToolButton {
                id: heading
                objectName: "module-toggle-" + root.section.module
                Layout.fillWidth: true
                padding: 0
                enabled: root.editable
                onClicked: root.controlEdited(root.enableControl, root.moduleEnabled ? 0 : 1)
                Accessible.name: "Enable " + root.section.name
                Accessible.checkable: true; Accessible.checked: root.moduleEnabled
                contentItem: RowLayout {
                    spacing: 7
                    Rectangle { opacity: root.moduleEnabled ? 1 : 0; width: 7; height: 7; radius: 3.5; color: root.theme.ink }
                    ModuleIcon {
                        moduleKey: root.section.module
                        opacity: root.moduleEnabled ? 1 : .6
                    }
                    Text {
                        Layout.fillWidth: true
                        text: root.section.name
                        color: heading.hovered || heading.activeFocus ? root.theme.accent : (root.moduleEnabled ? root.theme.ink : root.theme.muted)
                        font: root.theme.moduleHeadingFont; wrapMode: Text.WordWrap
                    }
                }
                background: Rectangle { color: "transparent"; border.color: heading.activeFocus ? root.theme.accent : "transparent" }
            }
        }
        ToolButton {
            objectName: "module-details-" + root.section.module
            visible: root.hasDetails
            anchors.right: parent.right
            y: 1
            width: 24; height: 22; padding: 0
            text: root.expanded ? "⌄" : "›"
            onClicked: root.expansionRequested()
            Accessible.name: "Details for " + root.section.name
            background: Rectangle { color: "transparent" }

        }
    }
    Repeater {
        id: rows
        model: root.section.controls
        delegate: ControlSlider {
            required property var modelData
            objectName: "filter-control-" + modelData.id + (root.section.module === "colisa" && !root.section.shortcut && modelData.id !== "contrast" ? "-module" : "")
            readonly property bool secondary: !root.section.primary.includes(modelData.id)
            x: 0
            width: parent.width - x
            opacity: moduleEnabled ? 1 : .7
            visible: root.expanded || root.section.primary.includes(modelData.id)
            compact: true
            moduleToggleAvailable: !root.headerVisible
            moduleIconKey: !root.headerVisible ? root.section.module : ""
            qualifyLabel: !root.headerVisible
            displayLabel: modelData.id === "vibrance" ? "vibrance"
                : !root.headerVisible && root.section.shortTitle ? root.section.name
                : root.headerVisible && modelData.section.includes(" · ")
                  ? modelData.section.split(" · ").slice(1).join(" · ") + " · " + modelData.label : ""
            darkVignette: modelData.id === "vignette" && !root.expanded
            moduleName: modelData.section
            moduleEnabled: root.values[modelData.module + "_enabled"] > .5
            onModuleToggleRequested: root.controlEdited(modelData.module + "_enabled", moduleEnabled ? 0 : 1)
            theme: root.theme; control: modelData; value: root.values[modelData.id]
            editable: root.editable; selected: root.activeControl === modelData.id
            detailsAvailable: !root.headerVisible && root.hasDetails && root.section.primary[0] === modelData.id
            detailsExpanded: root.expanded
            onDetailsRequested: root.expansionRequested()
            onSelectedRequested: root.controlSelected(modelData.id)
            onInteractionChanged: active => root.interactionChanged(active)
            onEdited: value => root.controlEdited(modelData.id, value)
            onResetRequested: root.controlReset(modelData.id)
        }
    }
    Button {
        visible: root.expanded && root.section.name === "diffuse or sharpen"
        text: "Halation recipe (experimental)"
        enabled: root.editable
        onClicked: root.halationRequested()
    }
    DenoiseCurve {
        visible: root.expanded && root.section.name === "denoise (profiled)"
        opacity: root.moduleEnabled ? 1 : .45
        x: 12; width: parent.width - 12
        theme: root.theme; values: root.values; editable: root.editable
        onEdited: (id, value) => root.controlEdited(id, value)
    }
}
