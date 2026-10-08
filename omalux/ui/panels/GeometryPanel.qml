import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import "../components"

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
    property real aspectRatio: 0
    signal interactionChanged(bool active)
    signal edited(string id, real value)
    signal cropApplied(var values)
    function begin() {
        wasEnabled = values.crop_enabled > .5
        crop = {x: values.crop_left/100, y: values.crop_top/100,
                width: 1-(values.crop_left+values.crop_right)/100,
                height: 1-(values.crop_top+values.crop_bottom)/100}
        cropping = true
        applyRatio()
        edited("crop_enabled", 0)
    }
    function applyRatio() {
        if (aspectRatio <= 0) return
        const ratio=aspectRatio/Math.max(.001,imageAspect)
        const w=Math.min(crop.width,crop.height*ratio),h=w/ratio
        crop={x:crop.x+(crop.width-w)/2,y:crop.y+(crop.height-h)/2,width:w,height:h}
    }
    function apply() {
        if (!cropping) return
        cropApplied({crop_left: crop.x*100, crop_top: crop.y*100,
                     crop_right: (1-crop.x-crop.width)*100, crop_bottom: (1-crop.y-crop.height)*100, crop_enabled: 1})
        cropping = false
    }
    function chooseRatio(index) {
        aspectRatio = [0, imageAspect, 1, 1.5, 4/3, .8, 16/9][index]
        if (cropping) applyRatio()
    }
    function resetCrop() { cropApplied({crop_left:0,crop_top:0,crop_right:0,crop_bottom:0,crop_enabled:0}) }
    function cancel() { if (cropping) { edited("crop_enabled", wasEnabled ? 1 : 0); cropping = false } }
    ColumnLayout {
        // Same margins as the module lists of the other edit panes (ModuleGroupsPanel).
        width: root.availableWidth - 36
        x: 18; spacing: 10
        // The crop block reads like the module cards below it: darktable's module name as the
        // heading, its rows (rotation, aspect) and the crop actions as outlined buttons.
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
                Text {
                    // In line with the module headings (after their enabled dot).
                    Layout.leftMargin: 8
                    text: "crop"
                    color: root.theme.ink
                    font: root.theme.moduleHeadingFont
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
                // darktable's "aspect" combobox (crop.c:1379) with its labels for these ratios.
                ComboBox {
                    id: ratioChoice
                    objectName: "aspectChoice"
                    wheelEnabled: false
                    Layout.fillWidth: true
                    Layout.rightMargin: 28
                    implicitHeight: 26
                    hoverEnabled: true
                    model: ["freehand", "original image", "square", "3:2, 4x6, 35mm", "4:3, VGA, TV", "5:4, 4x5, 8x10", "16:9, HDTV"]
                    onActivated: root.chooseRatio(currentIndex)
                    NavTarget {
                        id: ratioNav
                        navId: "aspect"; label: "aspect " + ratioChoice.currentText; kind: "choice"
                        activateLabel: ""; resettable: false
                        onAdjust: steps => { ratioChoice.currentIndex = Math.max(0, Math.min(ratioChoice.count - 1, ratioChoice.currentIndex + Math.sign(steps))); root.chooseRatio(ratioChoice.currentIndex) }
                    }
                    leftPadding: 0; rightPadding: 0
                    indicator: Item {}
                    contentItem: RowLayout {
                        spacing: 12
                        Text {
                            Layout.fillWidth: true
                            text: "aspect"
                            color: ratioNav.current ? root.theme.accent : root.theme.ink
                            font: root.theme.settingsFont
                        }
                        Text {
                            text: ratioChoice.displayText
                            color: root.editable ? root.theme.ink : root.theme.muted
                            font: root.theme.textFont
                        }
                        Text { text: "\u25be"; color: root.theme.muted; font: root.theme.textFont }
                    }
                    background: Rectangle {
                        color: ratioChoice.hovered ? root.theme.hover : "transparent"
                        radius: 4
                        border.width: ratioNav.current ? 1 : 0
                        border.color: root.theme.accent
                    }
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
                Text {
                    Layout.fillWidth: true
                    Layout.rightMargin: 28
                    visible: root.cropping
                    text: "drag the frame or its handles; Enter applies, Escape cancels"
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
                        Layout.fillWidth: true
                        text: "edit crop"; enabled: root.editable && !root.cropping; onClicked: root.begin()
                        highlighted: editNav.current
                        NavTarget { id: editNav; navId: "edit-crop"; label: "Edit crop"; enabled: editButton.enabled; onActivate: root.begin() }
                    }
                    Button {
                        id: resetButton
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
                    Button { Layout.fillWidth: true; text: "apply  [Enter]"; enabled: root.cropping; onClicked: root.apply() }
                    Button { Layout.fillWidth: true; text: "cancel  [Esc]"; enabled: root.cropping; onClicked: root.cancel() }
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
