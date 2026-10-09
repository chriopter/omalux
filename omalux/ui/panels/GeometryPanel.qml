import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import "../components"
import "../components/CropMath.js" as Crop

SidebarScrollView {
    id: root
    objectName: "geometryPanel"
    required property var theme
    required property var controls
    required property var values
    required property bool editable
    required property real imageAspect
    // darktable modules that belong to this pane (generated layout).
    property var catalogModel: null
    property var states: ({})
    property var overrides: ({})
    property string term: ""
    property string activeControl: ""
    signal changesRequested(string operation, int instance, var changes)
    signal enableRequested(string operation, int instance, bool enabled)
    signal moduleResetRequested(string operation, int instance, var module)
    signal moduleInteractionChanged(bool active)
    signal controlSelected(string id)
    property bool cropping: false
    property bool wasEnabled: false
    property var crop: ({x: 0, y: 0, width: 1, height: 1})
    // The wanted width over height of the frame in photo pixels (0: freehand).
    property real aspectRatio: 0
    // darktable's crop parameters ratio_d and ratio_n (crop.c): the chosen entry of the aspect
    // list; ratio_d's sign is the orientation. They are written with the crop, so darktable
    // shows the same aspect for the photo.
    property int ratioD: 0
    property int ratioN: 0
    property bool marginsOpen: false
    readonly property var cropState: states ? states["crop/0"] : undefined
    readonly property string storedRatio: cropState && cropState.values ? Number(cropState.values.ratio_d) + "/" + Number(cropState.values.ratio_n) : "-1/-1"
    // A photo opened or a history step taken: show its aspect (-1/-1: none stored yet).
    onStoredRatioChanged: {
        const stored = storedRatio.split("/").map(Number)
        if (cropping || (stored[0] === -1 && stored[1] === -1) || (stored[0] === ratioD && stored[1] === ratioN)) return
        if (Crop.aspectIndex(stored[0], stored[1]) < 0) return
        ratioD = stored[0]; ratioN = stored[1]
        aspectRatio = Crop.ratio(ratioD, ratioN, imageAspect)
    }
    onImageAspectChanged: if (ratioD !== 0) aspectRatio = Crop.ratio(ratioD, ratioN, imageAspect)
    readonly property int aspectIndex: Math.max(0, Crop.aspectIndex(ratioD, ratioN))
    // Choosing from the list breaks the combobox's binding; history steps still have to show.
    onAspectIndexChanged: ratioChoice.currentIndex = aspectIndex
    signal interactionChanged(bool active)
    signal edited(string id, real value)
    signal cropApplied(var values)
    // The stored crop as a box in fractions of the photo.
    function storedBox() {
        return {x: values.crop_left/100, y: values.crop_top/100,
                width: 1-(values.crop_left+values.crop_right)/100,
                height: 1-(values.crop_top+values.crop_bottom)/100}
    }
    function boxValues(c) {
        return {crop_left: c.x*100, crop_top: c.y*100,
                crop_right: (1-c.x-c.width)*100, crop_bottom: (1-c.y-c.height)*100, crop_enabled: 1}
    }
    function begin() {
        wasEnabled = values.crop_enabled > .5
        crop = storedBox()
        cropping = true
        applyRatio()
        edited("crop_enabled", 0)
    }
    // Choosing a ratio keeps the frame's centre.
    function applyRatio() {
        if (aspectRatio <= 0) return
        crop = Crop.fit(crop, aspectRatio, imageAspect, 1)
    }
    function storeRatio() {
        if (storedRatio !== ratioD + "/" + ratioN)
            changesRequested("crop", 0, {ratio_d: ratioD, ratio_n: ratioN})
    }
    function apply() {
        if (!cropping) return
        storeRatio()
        cropApplied(boxValues(crop))
        cropping = false
    }
    function chooseRatio(index) {
        const a = Crop.aspects[index]
        if (!a) return
        ratioD = a.d; ratioN = a.n
        aspectRatio = Crop.ratio(ratioD, ratioN, imageAspect)
        if (cropping) applyRatio()
    }
    // darktable's button on the aspect row: portrait and landscape of the same ratio.
    function flipAspect() {
        if (ratioD === 0 || ratioD === ratioN) return
        ratioD = -ratioD
        aspectRatio = Crop.ratio(ratioD, ratioN, imageAspect)
        if (cropping) applyRatio()
    }
    function resetCrop() { cropApplied({crop_left:0,crop_top:0,crop_right:0,crop_bottom:0,crop_enabled:0}) }
    function cancel() { if (cropping) { edited("crop_enabled", wasEnabled ? 1 : 0); cropping = false } }
    // The margins as darktable shows them, per cent from each side; while the frame is edited
    // they are the frame's.
    readonly property var shownBox: cropping ? crop : storedBox()
    function marginValue(id) {
        const c = shownBox
        const v = id === "crop_left" ? c.x*100 : id === "crop_top" ? c.y*100
                : id === "crop_right" ? (1-c.x-c.width)*100 : (1-c.y-c.height)*100
        return Math.max(0, Math.min(100, v))
    }
    // One margin moved (crop.c gui_changed): the opposite side stays, the aspect is kept.
    function setMargin(id, value) {
        const side = id === "crop_left" ? Crop.LEFT : id === "crop_top" ? Crop.TOP : id === "crop_right" ? Crop.RIGHT : Crop.BOTTOM
        const edge = side === Crop.LEFT || side === Crop.TOP ? value/100 : 1 - value/100
        const next = Crop.setEdge(shownBox, side, edge, aspectRatio, imageAspect, 1)
        if (cropping) crop = next
        else { storeRatio(); cropApplied(boxValues(next)) }
    }
    ColumnLayout {
        // Same margins as the module lists of the other edit panes (ModuleGroupsPanel).
        width: root.availableWidth - 36
        x: 18; spacing: 10
        // The crop block reads like the module cards below it: darktable's module name as the
        // heading, its rows (rotation, aspect, margins) and the crop actions as outlined buttons.
        Rectangle {
            Layout.fillWidth: true
            Layout.topMargin: 18
            // Module cards reach 14 px left of their list's text column.
            Layout.leftMargin: -14
            implicitHeight: cropColumn.implicitHeight + 16
            color: root.theme.surface
            ColumnLayout {
                id: cropColumn
                x: 14; y: 8
                width: parent.width - 14
                spacing: 8
                RowLayout {
                    Layout.leftMargin: 8
                    spacing: 8
                    ModuleIcon { moduleKey: "crop"; opacity: .85 }
                    Text {
                        text: "crop"
                        color: root.theme.ink
                        font: root.theme.moduleHeadingFont
                    }
                }
                Repeater {
                    model: root.controls.filter(c => c.id === "rotation")
                    ControlSlider {
                        required property var modelData
                        Layout.fillWidth: true
                        compact: true
                        moduleToggleAvailable: false
                        theme: root.theme; control: modelData; value: root.values[modelData.id]; editable: root.editable && !root.cropping
                        onInteractionChanged: active => root.interactionChanged(active)
                        onEdited: value => root.edited(modelData.id, value)
                        onResetRequested: root.edited(modelData.id, 0)
                    }
                }
                RowLayout {
                    Layout.fillWidth: true
                    Layout.rightMargin: 28
                    spacing: 6
                    // darktable's "aspect" combobox with its whole list and labels (crop.c:1267).
                    ComboBox {
                        id: ratioChoice
                        objectName: "aspectChoice"
                        wheelEnabled: false
                        Layout.fillWidth: true
                        implicitHeight: 26
                        hoverEnabled: true
                        enabled: root.editable
                        model: Crop.aspects.map(a => Crop.aspectName(a))
                        currentIndex: root.aspectIndex
                        onActivated: index => root.chooseRatio(index)
                        NavTarget {
                            id: ratioNav
                            navId: "aspect"; label: "aspect " + ratioChoice.currentText; kind: "choice"
                            activateLabel: ""; resettable: false
                            onAdjust: steps => root.chooseRatio(Math.max(0, Math.min(ratioChoice.count - 1, root.aspectIndex + Math.sign(steps))))
                        }
                        leftPadding: 0; rightPadding: 0
                        indicator: Item {}
                        contentItem: RowLayout {
                            spacing: 12
                            Text {
                                text: "aspect"
                                color: ratioNav.current ? root.theme.accent : root.theme.ink
                                font: root.theme.settingsFont
                            }
                            Text {
                                Layout.fillWidth: true
                                horizontalAlignment: Text.AlignRight
                                text: ratioChoice.displayText
                                color: root.editable ? root.theme.ink : root.theme.muted
                                font: root.theme.textFont
                                elide: Text.ElideMiddle
                            }
                            Text { text: "▾"; color: root.theme.muted; font: root.theme.textFont }
                        }
                        background: Rectangle {
                            color: ratioChoice.hovered ? root.theme.hover : "transparent"
                            radius: 4
                            border.width: ratioNav.current ? 1 : 0
                            border.color: root.theme.accent
                        }
                        popup.height: Math.min(popup.contentItem.implicitHeight + 2, 330)
                        delegate: ItemDelegate {
                            id: ratioItem
                            required property var modelData
                            required property int index
                            width: ListView.view.width
                            implicitHeight: 26
                            highlighted: ratioChoice.highlightedIndex === index
                            hoverEnabled: true
                            contentItem: Text {
                                text: ratioItem.modelData
                                color: ratioItem.highlighted || ratioChoice.currentIndex === ratioItem.index ? root.theme.accent : root.theme.ink
                                font: root.theme.textFont
                                verticalAlignment: Text.AlignVCenter
                            }
                            background: Rectangle { color: ratioItem.highlighted ? root.theme.active : "transparent"; radius: 3 }
                        }
                    }
                    // darktable's quad button on the aspect row (dtgtk_cairo_paint_aspectflip).
                    ToolButton {
                        id: flipButton
                        objectName: "aspectFlip"
                        implicitWidth: 26; implicitHeight: 26; padding: 0
                        hoverEnabled: true
                        enabled: root.editable && root.ratioD !== 0 && root.ratioD !== root.ratioN
                        onClicked: root.flipAspect()
                        ToolTip.visible: hovered
                        ToolTip.delay: 600
                        ToolTip.text: "flip crop orientation between horizontal and vertical"
                        Accessible.name: "flip crop orientation"
                        NavTarget { id: flipNav; navId: "aspect-flip"; label: "Flip crop orientation"; enabled: flipButton.enabled; resettable: false; onActivate: root.flipAspect() }
                        contentItem: Item {
                            // A landscape and a portrait frame; the one in use is drawn solid.
                            readonly property bool portrait: root.aspectRatio > 0 && root.aspectRatio < 1
                            opacity: flipButton.enabled ? 1 : .35
                            Rectangle {
                                anchors.centerIn: parent; width: 16; height: 10; color: "transparent"
                                border.color: flipButton.hovered || flipNav.current ? root.theme.accent : root.theme.ink
                                opacity: parent.portrait ? .45 : 1
                            }
                            Rectangle {
                                anchors.centerIn: parent; width: 10; height: 16; color: "transparent"
                                border.color: flipButton.hovered || flipNav.current ? root.theme.accent : root.theme.ink
                                opacity: parent.portrait ? 1 : .45
                            }
                        }
                        background: Rectangle {
                            radius: 4
                            color: flipButton.pressed ? root.theme.active : flipButton.hovered ? root.theme.hover : "transparent"
                            border.width: flipNav.current ? 1 : 0
                            border.color: root.theme.accent
                        }
                    }
                }
                // darktable's collapsible "margins" section (crop.c:1404).
                ToolButton {
                    id: marginsButton
                    objectName: "crop-margins"
                    Layout.fillWidth: true
                    Layout.rightMargin: 28
                    implicitHeight: 22; padding: 0
                    hoverEnabled: true
                    onClicked: root.marginsOpen = !root.marginsOpen
                    NavTarget { id: marginsNav; navId: "crop-margins"; label: "margins"; resettable: false; activateLabel: root.marginsOpen ? "COLLAPSE" : "EXPAND"; onActivate: root.marginsOpen = !root.marginsOpen }
                    contentItem: RowLayout {
                        spacing: 6
                        Text {
                            Layout.fillWidth: true
                            text: "margins"
                            color: marginsButton.hovered || marginsNav.current ? root.theme.accent : root.theme.muted
                            font: root.theme.textFont
                        }
                        Text {
                            text: root.marginsOpen ? "▾" : "▸"
                            color: root.theme.muted; font: root.theme.textFont
                        }
                    }
                    background: Item {}
                }
                Repeater {
                    model: root.marginsOpen ? ["crop_left", "crop_right", "crop_top", "crop_bottom"].map(id => root.controls.find(c => c.id === id)).filter(c => !!c) : []
                    ControlSlider {
                        required property var modelData
                        objectName: "margin-" + modelData.id
                        Layout.fillWidth: true
                        compact: true
                        moduleToggleAvailable: false
                        theme: root.theme; control: modelData; value: root.marginValue(modelData.id); editable: root.editable
                        onInteractionChanged: active => root.interactionChanged(active)
                        onEdited: value => root.setMargin(modelData.id, value)
                        onResetRequested: root.setMargin(modelData.id, 0)
                    }
                }
                Text {
                    Layout.fillWidth: true
                    Layout.rightMargin: 28
                    visible: root.cropping
                    text: "drag an edge or corner to resize, inside to move (Shift and Ctrl constrain); right-click resets"
                    wrapMode: Text.WordWrap
                    color: root.theme.muted
                    font: root.theme.textFont
                }
                RowLayout {
                    Layout.fillWidth: true
                    Layout.rightMargin: 28
                    spacing: 6
                    Button {
                        id: editButton
                        objectName: "crop-edit"
                        Layout.fillWidth: true
                        text: "edit crop"; enabled: root.editable && !root.cropping; onClicked: root.begin()
                        highlighted: editNav.current
                        NavTarget { id: editNav; navId: "edit-crop"; label: "Edit crop"; enabled: editButton.enabled; onActivate: root.begin() }
                    }
                    Button {
                        id: resetButton
                        objectName: "crop-reset"
                        Layout.fillWidth: true
                        text: "reset crop"; enabled: root.editable && !root.cropping
                        onClicked: root.resetCrop()
                        highlighted: resetNav.current
                        NavTarget { id: resetNav; navId: "reset-crop"; label: "Reset crop"; enabled: resetButton.enabled; onActivate: root.resetCrop() }
                    }
                }
                RowLayout {
                    Layout.fillWidth: true
                    Layout.rightMargin: 28
                    visible: root.cropping
                    spacing: 6
                    Button { objectName: "crop-apply"; Layout.fillWidth: true; text: "apply  [Enter]"; enabled: root.cropping; onClicked: root.apply() }
                    Button { objectName: "crop-cancel"; Layout.fillWidth: true; text: "cancel  [Esc]"; enabled: root.cropping; onClicked: root.cancel() }
                }
            }
        }
        Loader {
            Layout.fillWidth: true
            active: !!root.catalogModel
            sourceComponent: ModuleList {
                theme: root.theme
                groups: root.catalogModel.groupsForTab("geometry")
                catalogModel: root.catalogModel
                states: root.states
                overrides: root.overrides
                editable: root.editable && !root.cropping
                // Only while the pane is on screen (search opens every match).
                term: root.visible ? root.term : ""
                activeControl: root.activeControl
                settingsKey: "geometry"
                onChangesRequested: (operation, instance, changes) => root.changesRequested(operation, instance, changes)
                onEnableRequested: (operation, instance, enabled) => root.enableRequested(operation, instance, enabled)
                onResetRequested: (operation, instance, module) => root.moduleResetRequested(operation, instance, module)
                onInteractionChanged: active => root.moduleInteractionChanged(active)
                onControlSelected: id => root.controlSelected(id)
            }
        }
    }
}
