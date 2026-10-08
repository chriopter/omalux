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
        width: root.availableWidth - 20
        x: 10; spacing: 14
        Text { text: "CROP & ROTATE"; color: root.theme.ink; font.bold: true }
        Repeater {
            model: root.controls.filter(c => c.id === "rotation")
            ControlSlider {
                required property var modelData
                Layout.fillWidth: true
                theme: root.theme; control: modelData; value: root.values[modelData.id]; editable: root.editable && !root.cropping
                onInteractionChanged: active => root.interactionChanged(active)
                onEdited: value => root.edited(modelData.id, value)
                onResetRequested: root.edited(modelData.id, 0)
            }
        }
        ComboBox {
            id: ratioChoice
            objectName: "aspectChoice"
            wheelEnabled: false
            Layout.fillWidth: true
            model: ["Free", "Original", "1:1", "3:2", "4:3", "4:5", "16:9"]
            onActivated: root.chooseRatio(currentIndex)
            NavTarget {
                id: ratioNav
                navId: "aspect"; label: "aspect " + ratioChoice.currentText; kind: "choice"
                activateLabel: ""; resettable: false
                onAdjust: steps => { ratioChoice.currentIndex = Math.max(0, Math.min(ratioChoice.count - 1, ratioChoice.currentIndex + Math.sign(steps))); root.chooseRatio(ratioChoice.currentIndex) }
            }
            Rectangle { anchors.fill: parent; visible: ratioNav.current; color: "transparent"; border.color: root.theme.accent }
        }
        Button {
            id: editButton
            text: "Edit crop"; enabled: root.editable && !root.cropping; onClicked: root.begin()
            highlighted: editNav.current
            NavTarget { id: editNav; navId: "edit-crop"; label: "Edit crop"; enabled: editButton.enabled; onActivate: root.begin() }
        }
        Text { Layout.fillWidth: true; visible: root.cropping; text: "Drag the frame or its handles. Enter applies; Escape cancels."; wrapMode: Text.WordWrap; color: root.theme.muted; font: root.theme.textFont }
        RowLayout {
            Button { text: "Apply [Enter]"; enabled: root.cropping; onClicked: root.apply() }
            Button { text: "Cancel [Esc]"; enabled: root.cropping; onClicked: root.cancel() }
        }
        Button {
            id: resetButton
            text: "Reset crop"; enabled: root.editable && !root.cropping
            onClicked: root.resetCrop()
            highlighted: resetNav.current
            NavTarget { id: resetNav; navId: "reset-crop"; label: "Reset crop"; enabled: resetButton.enabled; onActivate: root.resetCrop() }
        }
        Loader {
            Layout.fillWidth: true
            Layout.leftMargin: 14
            Layout.rightMargin: 4
            active: !!root.catalogModel
            sourceComponent: ModuleList {
                theme: root.theme
                groups: root.catalogModel.groupsForTab("geometry")
                catalogModel: root.catalogModel
                states: root.states
                overrides: root.overrides
                editable: root.editable && !root.cropping
                term: root.term
                activeControl: root.activeControl
                settingsKey: "geometry"
                caption: "MODULES"
                onChangesRequested: (operation, instance, changes) => root.changesRequested(operation, instance, changes)
                onEnableRequested: (operation, instance, enabled) => root.enableRequested(operation, instance, enabled)
                onResetRequested: (operation, instance, module) => root.moduleResetRequested(operation, instance, module)
                onInteractionChanged: active => root.moduleInteractionChanged(active)
                onControlSelected: id => root.controlSelected(id)
            }
        }
    }
}
