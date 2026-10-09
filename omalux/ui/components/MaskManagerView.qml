import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

// darktable's mask manager (src/libs/masks.c) for one module's drawn mask: the group's shapes in
// darktable's tree order with their combine mode, inversion and opacity, and the tree's menu —
// move up / move down, remove from group, rename, duplicate, delete this shape, add existing
// shape (with "use same shapes as" another module) and delete unused shapes. `report` is the
// engine's report (mask_manager.c); every choice leaves as requested(action, args).
Column {
    id: root
    required property var theme
    property var report: null            // { group: [...], available: [...], modules: [...] }
    property bool editable: true
    property string navGroup: ""
    signal requested(string action, var args)
    spacing: 2
    readonly property var modes: [{ value: 8, label: "union", sign: "∪" }, { value: 16, label: "intersection", sign: "∩" },
                                  { value: 32, label: "difference", sign: "−" }, { value: 128, label: "sum", sign: "+" },
                                  { value: 64, label: "exclusion", sign: "⊕" }]
    function modeOf(state) { return root.modes.find(m => state & m.value) || null }
    property int renaming: -1

    Repeater {
        model: root.report ? root.report.group : []
        RowLayout {
            id: line
            required property var modelData
            required property int index
            width: root.width
            spacing: 4
            NavTarget {
                id: nav
                navId: root.navGroup + "/blend/shape/" + line.modelData.id
                label: line.modelData.name
                group: root.navGroup
                kind: "button"
                activateLabel: "SHAPE MENU"
                enabled: root.editable
                onActivate: menu.popup(name, 0, name.height)
                onReset: root.requested("remove", { id: line.modelData.id })
            }
            // combine mode with the shapes above it (the first shape has none, as in darktable)
            Text {
                Layout.preferredWidth: 14
                text: line.modelData.first ? "" : (root.modeOf(line.modelData.state) || { sign: "" }).sign
                color: root.theme.muted; font: root.theme.settingsFont
                horizontalAlignment: Text.AlignHCenter
            }
            Text {
                id: name
                visible: root.renaming !== line.modelData.id
                Layout.fillWidth: true
                text: line.modelData.name + ((line.modelData.state & 4) ? "  (inverted)" : "")
                color: nav.current ? root.theme.accent : root.theme.ink
                font: root.theme.settingsFont
                elide: Text.ElideRight
                TapHandler { acceptedButtons: Qt.LeftButton | Qt.RightButton; onTapped: { nav.claim(); menu.popup(name, 0, name.height) } }
                TapHandler { onDoubleTapped: root.renaming = line.modelData.id }
            }
            TextField {
                visible: root.renaming === line.modelData.id
                Layout.fillWidth: true
                text: line.modelData.name
                font: root.theme.textFont
                onVisibleChanged: if (visible) { forceActiveFocus(); selectAll() }
                onAccepted: { root.requested("rename", { id: line.modelData.id, name: text }); root.renaming = -1 }
                Keys.onEscapePressed: root.renaming = -1
            }
            Text {
                text: Math.round(line.modelData.opacity * 100) + " %"
                color: root.theme.muted; font: root.theme.settingsFont
                WheelHandler {
                    enabled: root.editable
                    onWheel: event => root.requested("opacity", { id: line.modelData.id,
                                                                   value: Math.max(.05, Math.min(1, line.modelData.opacity + (event.angleDelta.y > 0 ? .05 : -.05))) })
                }
            }
            Menu {
                id: menu
                objectName: "mask-menu-" + line.modelData.id
                MenuItem { text: "use inverted shape"; checkable: true; checked: (line.modelData.state & 4) !== 0
                           onTriggered: root.requested("invert", { id: line.modelData.id }) }
                MenuSeparator {}
                Repeater {
                    model: root.modes
                    MenuItem {
                        required property var modelData
                        text: "mode: " + modelData.label
                        checkable: true
                        checked: (line.modelData.state & modelData.value) !== 0
                        enabled: !line.modelData.first
                        onTriggered: root.requested("mode", { id: line.modelData.id, value: modelData.value })
                    }
                }
                MenuSeparator {}
                MenuItem { text: "move up"; enabled: line.index > 0; onTriggered: root.requested("up", { id: line.modelData.id }) }
                MenuItem { text: "move down"; enabled: !line.modelData.first; onTriggered: root.requested("down", { id: line.modelData.id }) }
                MenuSeparator {}
                MenuItem { text: "rename"; onTriggered: root.renaming = line.modelData.id }
                MenuItem { text: "duplicate this shape"; onTriggered: root.requested("duplicate", { id: line.modelData.id }) }
                MenuItem { text: "remove from group"; onTriggered: root.requested("remove", { id: line.modelData.id }) }
                MenuItem { text: "delete this shape"; onTriggered: root.requested("delete", { id: line.modelData.id }) }
            }
        }
    }
    // darktable's "properties" of the mask manager (libs/masks.c:110, 1930): size, feather,
    // hardness, rotation, curvature and compression of the shape selected on the photo, or of
    // every shape while none is selected; each slider shows the shapes' mean and moves them
    // together (sizes, feathers and compressions by the same factor). As in darktable the values
    // are in per cent with two decimals and a size slider is relative: its track covers a
    // quarter to four times the value (darktable: a logarithmic track); typing reaches the rest.
    Text {
        visible: properties.count > 0
        topPadding: 6
        text: "properties" + (root.report && root.report.selected ? "" : (root.report && root.report.group.length > 1 ? "  ·  all shapes" : ""))
        color: root.theme.muted; font: root.theme.textFont
    }
    property var pending: ({})
    Repeater {
        id: properties
        model: root.report && root.report.properties ? root.report.properties : []
        ControlSlider {
            id: property
            required property var modelData
            readonly property real factor: modelData.unit === "%" ? 100 : 1
            readonly property real pendingValue: root.pending[modelData.key] !== undefined ? root.pending[modelData.key] : modelData.value * factor
            property bool dragging: false
            objectName: "mask-property-" + modelData.key
            // A compact slider keeps a column for a chevron on its right; these rows have none.
            width: root.width + 28
            compact: true
            moduleToggleAvailable: false
            qualifyLabel: false
            theme: root.theme
            editable: root.editable
            control: ({ id: root.navGroup + "/blend/property/" + modelData.key, label: modelData.key, section: "properties",
                        minimum: modelData.min * factor, maximum: modelData.max * factor,
                        softMinimum: modelData.relative ? Math.max(modelData.min, modelData.value / 4) * factor : modelData.min * factor,
                        softMaximum: modelData.relative ? Math.min(modelData.max, modelData.value * 4) * factor : modelData.max * factor,
                        step: modelData.unit === "%" ? 0.1 : 1, decimals: 2, unit: modelData.unit, colors: "" })
            value: pendingValue
            navTarget.group: root.navGroup
            function send(value) {
                const v = Math.max(control.minimum, Math.min(control.maximum, value))
                root.requested("property", { property: modelData.key, old: modelData.value, value: v / factor })
            }
            onInteractionChanged: active => {
                dragging = active
                if (active || root.pending[modelData.key] === undefined) return
                const v = root.pending[modelData.key]
                const next = Object.assign({}, root.pending); delete next[modelData.key]; root.pending = next
                send(v)
            }
            onEdited: value => {
                if (!dragging) { send(value); return }
                const next = Object.assign({}, root.pending); next[modelData.key] = value; root.pending = next
            }
        }
    }
    // Outlined chips like the shape buttons above (BlendSection); they wrap when both do not
    // fit the module's width.
    component Chip: Rectangle {
        property Item button: parent
        property bool marked: false
        radius: 4
        color: button.pressed ? root.theme.active : button.hovered && button.enabled ? root.theme.hover : "transparent"
        border.width: 1
        border.color: marked ? root.theme.accent : root.theme.line
        opacity: button.enabled ? 1 : .6
    }
    Flow {
        width: root.width
        spacing: 6
        AbstractButton {
            id: addButton
            objectName: "mask-add-existing-" + root.navGroup
            enabled: root.editable && !!root.report && (root.report.available.length + root.report.modules.length) > 0
            hoverEnabled: true
            onClicked: { addNav.claim(); addMenu.popup(addButton, 0, addButton.height) }
            NavTarget { id: addNav; navId: root.navGroup + "/blend/add-existing"; label: "add existing shape"; group: root.navGroup
                        kind: "button"; enabled: addButton.enabled; onActivate: addMenu.popup(addButton, 0, addButton.height) }
            contentItem: Text { text: "add existing shape ▾"; font: root.theme.textFont
                                color: !addButton.enabled ? root.theme.muted : addNav.current ? root.theme.accent : addButton.hovered ? root.theme.ink : root.theme.muted }
            leftPadding: 7; rightPadding: 7; topPadding: 3; bottomPadding: 3
            background: Chip { marked: addNav.current }
            Menu {
                id: addMenu
                Repeater {
                    model: root.report ? root.report.modules : []
                    MenuItem { required property var modelData; text: "use same shapes as " + modelData.label
                               onTriggered: root.requested("same", { operation: modelData.operation, instance: modelData.instance }) }
                }
                Repeater {
                    model: root.report ? root.report.available : []
                    MenuItem { required property var modelData; text: modelData.name + (modelData.used ? "" : "  (unused)")
                               onTriggered: root.requested("add", { id: modelData.id }) }
                }
            }
        }
        AbstractButton {
            id: cleanButton
            enabled: root.editable && !!root.report && root.report.available.some(a => !a.used)
            hoverEnabled: true
            onClicked: { cleanNav.claim(); root.requested("cleanup", {}) }
            NavTarget { id: cleanNav; navId: root.navGroup + "/blend/cleanup"; label: "delete unused shapes"; group: root.navGroup
                        kind: "button"; enabled: cleanButton.enabled; onActivate: root.requested("cleanup", {}) }
            contentItem: Text { text: "delete unused shapes"; font: root.theme.textFont
                                color: !cleanButton.enabled ? root.theme.muted : cleanNav.current ? root.theme.accent : cleanButton.hovered ? root.theme.ink : root.theme.muted }
            leftPadding: 7; rightPadding: 7; topPadding: 3; bottomPadding: 3
            background: Chip { marked: cleanNav.current }
        }
    }
}
