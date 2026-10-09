import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

// The slim line under the kept main row of an unfolded module: darktable's name of the module
// the rows below belong to (the main row may carry a plainer name: "vibrance" is color balance
// rgb, "contrast" is sigmoid) and, at the end of the value column, the module's reset and the
// ⋯ button with darktable's multi-instance actions. Switching the module stays with the main
// row's name; the strip is not a keyboard stop (Shift+R resets, I opens the instance menu).
Item {
    id: root
    required property var theme
    required property string operation
    required property string name
    property string instanceLabel: ""
    property var moduleState
    property bool ready: true
    property bool showInstances: true
    property int instanceCount: 1
    property string navGroup: ""
    property string instancesSuffix: operation
    property alias instanceButton: instanceButton
    readonly property string title: name + (instanceLabel !== "" ? " • " + instanceLabel : "")
    signal resetRequested()
    signal instanceRequested(string action, string name)
    implicitHeight: 30
    // One rule parts the main row from the module's rows, as the section rows below are parted.
    Rectangle { x: -8; y: 2; width: parent.width + 8; height: 1; color: root.theme.line; opacity: .55 }
    RowLayout {
        anchors.fill: parent
        anchors.topMargin: 6
        anchors.rightMargin: 28
        spacing: 2
        Text {
            objectName: "module-strip-" + root.operation
            Layout.fillWidth: true
            text: root.title
            color: root.theme.muted
            font: root.theme.textFont
            elide: Text.ElideRight
        }
        // More than one instance of the module exists: never hidden behind the menu.
        Text {
            objectName: "module-instance-count-" + root.operation
            visible: root.instanceCount > 1
            text: "×" + root.instanceCount
            color: root.theme.muted
            font: root.theme.textFont
            rightPadding: 4
        }
        ResetButton {
            objectName: "module-reset-" + root.operation
            theme: root.theme
            title: root.title
            enabled: root.ready
            onClicked: root.resetRequested()
        }
        InstanceButton {
            id: instanceButton
            objectName: "module-instances-" + root.instancesSuffix
            visible: root.showInstances && !!root.moduleState
            theme: root.theme
            moduleState: root.moduleState
            title: root.title
            editable: root.ready
            navGroup: root.navGroup
            onInstanceRequested: (action, name) => root.instanceRequested(action, name)
        }
    }
}
