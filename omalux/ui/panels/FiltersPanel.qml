import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import "../components"

SidebarScrollView {
    id: root
    objectName: "filtersScroll"
    required property var theme
    required property var controls
    required property var values
    required property bool editable
    required property string activeControl
    property var expandedDetails: ({})
    signal halationRequested()
    signal controlSelected(string id)
    signal controlEdited(string id, real value)
    signal controlReset(string id)
    readonly property var sections: {
        let result = []
        for (let c of controls.filter(c => c.group !== "System" && c.group !== "Geometry" && c.group !== "Curve")) {
            let section = result.find(s => s.name === c.section && s.group === c.group)
            if (!section) { section = { name: c.section, group: c.group, controls: [] }; result.push(section) }
            section.controls.push(c)
        }
        const order=["Basics","Color","Effects","Denoise"]
        result.sort((a,b) => order.indexOf(a.group)-order.indexOf(b.group))
        return result
    }
    function reveal(id) {
        const c = controls.find(c => c.id === id)
        if (c && c.detail) {
            let next = Object.assign({}, expandedDetails); next[c.section] = true; expandedDetails = next
        }
        revealTimer.controlId=id; revealTimer.restart()
    }
    Timer {
        id: revealTimer
        property string controlId
        interval: 50
        onTriggered: {
            const id=controlId
            for (let i=0; i<sectionsRepeater.count; ++i) {
                const section = sectionsRepeater.itemAt(i)
                const item = section.findControl(id)
                if (item) {
                    const point = item.mapToItem(root.contentItem.contentItem, 0, 0)
                    const flick = root.contentItem
                    let next = flick.contentY
                    if (point.y < next) next = point.y
                    else if (point.y + item.height > next + flick.height) next = point.y + item.height - flick.height
                    flick.contentY = Math.max(0, Math.min(next, Math.max(0, flick.contentHeight - flick.height)))
                }
            }
        }
    }
    function navigate(direction) {
        const visible = sections.reduce((all, section) => all.concat(section.controls), []).filter(c => c.group !== "System" && c.group !== "Geometry" && c.group !== "Curve" && (!c.detail || expandedDetails[c.section]))
        let index = visible.findIndex(c => c.id === activeControl)
        if (visible.length) { const c = visible[(index + direction + visible.length) % visible.length]; controlSelected(c.id); reveal(c.id) }
    }
    Column {
        width: root.availableWidth
        padding: 10; spacing: 16
        Text { text: "FILTERS"; color: root.theme.ink; font.bold: true; font.letterSpacing: 2 }
        Repeater {
            id: sectionsRepeater
            model: root.sections
            delegate: Column {
                id: section
                required property var modelData
                required property int index
                readonly property string enableControl: modelData.controls[0].module + "_enabled"
                readonly property bool moduleEnabled: root.values[enableControl] > .5
                readonly property real effectOpacity: moduleEnabled ? 1 : 0.45
                width: parent.width - 20; spacing: 4
                function findControl(id) {
                    for (let i=0; i<rows.count; ++i) { const item=rows.itemAt(i); if (item.control.id===id && item.visible) return item }
                    return null
                }
                Text {
                    visible: section.index === 0 || root.sections[section.index-1].group !== section.modelData.group
                    text: section.modelData.group.toUpperCase(); color: root.theme.accent
                    font: root.theme.textFont; topPadding: 12; bottomPadding: 8
                }
                RowLayout {
                    width: parent.width
                    opacity: section.effectOpacity
                    ToolButton {
                        id: heading
                        Layout.fillWidth: true
                        padding: 0
                        enabled: root.editable
                        onClicked: root.controlEdited(section.enableControl, section.moduleEnabled ? 0 : 1)
                        Accessible.name: section.modelData.name
                        Accessible.checkable: true
                        Accessible.checked: section.moduleEnabled
                        ToolTip.visible: hovered
                        ToolTip.text: (section.moduleEnabled ? "Disable " : "Enable ") + section.modelData.controls[0].module
                        contentItem: Text {
                            text: section.modelData.name
                            color: heading.hovered ? root.theme.accent : root.theme.ink
                            font: root.theme.textFont
                            wrapMode: Text.WordWrap
                            verticalAlignment: Text.AlignVCenter
                        }
                        background: Rectangle {
                            color: "transparent"
                            border.color: heading.activeFocus ? root.theme.accent : "transparent"
                        }
                    }
                    ToolButton {
                        text: "RESET"; implicitHeight: 24; enabled: root.editable
                        onClicked: { for (let c of section.modelData.controls) root.controlReset(c.id) }
                        Accessible.name: "Reset " + section.modelData.name
                    }
                    ToolButton {
                        visible: section.modelData.controls.some(c => c.detail)
                        text: root.expandedDetails[section.modelData.name] ? "▾" : "▸"
                        implicitHeight: 24
                        onClicked: { let next=Object.assign({}, root.expandedDetails); next[section.modelData.name]=!next[section.modelData.name]; root.expandedDetails=next }
                        Accessible.name: "Details for " + section.modelData.name
                    }
                    ToolButton {
                        id: moduleToggle
                        implicitWidth: 22
                        implicitHeight: 24
                        padding: 0
                        enabled: root.editable
                        onClicked: root.controlEdited(section.enableControl, section.moduleEnabled ? 0 : 1)
                        Accessible.name: section.modelData.name
                        Accessible.checkable: true
                        Accessible.checked: section.moduleEnabled
                        ToolTip.visible: hovered
                        ToolTip.text: (section.moduleEnabled ? "Disable module: " : "Enable module: ") + section.modelData.name.split(" · ")[0]
                        contentItem: Text {
                            text: section.moduleEnabled ? "✓" : "□"
                            color: section.moduleEnabled || moduleToggle.hovered ? root.theme.accent : root.theme.muted
                            font: root.theme.textFont
                            horizontalAlignment: Text.AlignHCenter
                            verticalAlignment: Text.AlignVCenter
                        }
                        background: Rectangle {
                            color: "transparent"
                            border.color: moduleToggle.activeFocus ? root.theme.accent : "transparent"
                        }
                    }
                }
                Button {
                    visible: section.modelData.name === "diffuse or sharpen"
                    opacity: section.effectOpacity
                    text: "Halation recipe (experimental)"
                    enabled: root.editable
                    onClicked: root.halationRequested()
                }
                Repeater {
                    id: rows
                    model: section.modelData.controls
                    delegate: ControlSlider {
                        required property var modelData
                        width: parent.width
                        opacity: section.effectOpacity
                        visible: !modelData.detail || !!root.expandedDetails[modelData.section]
                        theme: root.theme; control: modelData; value: root.values[modelData.id]
                        editable: root.editable; selected: root.activeControl === modelData.id
                        onSelectedRequested: root.controlSelected(modelData.id)
                        onEdited: value => root.controlEdited(modelData.id, value)
                        onResetRequested: root.controlReset(modelData.id)
                    }
                }
                DenoiseCurve {
                    opacity: section.effectOpacity
                    visible: section.modelData.name === "denoise (profiled)"
                    theme: root.theme; values: root.values; editable: root.editable
                    onEdited: (id, value) => root.controlEdited(id,value)
                }
            }
        }
    }
}
