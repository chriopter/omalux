import QtQuick
import QtQuick.Controls

Button {
    id: control

    required property var theme
    required property url iconSource
    required property string label
    property bool selected: false

    implicitWidth: 56
    implicitHeight: 30
    activeFocusOnTab: false
    display: AbstractButton.IconOnly
    icon.source: control.iconSource
    icon.width: 16
    icon.height: 16
    icon.color: control.selected ? control.theme.accentColor : control.theme.mutedColor
    Accessible.name: control.label

    ToolTip.visible: hovered
    ToolTip.delay: 500
    ToolTip.text: control.label

    background: Rectangle {
        color: control.selected ? control.theme.selectionColor
                                : control.down ? control.theme.raisedColor
                                               : control.hovered ? control.theme.surfaceColor
                                                                 : control.theme.pageColor

        Rectangle {
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.bottom: parent.bottom
            height: 2
            color: control.theme.accentColor
            visible: control.selected
        }
    }
}
