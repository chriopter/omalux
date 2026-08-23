import QtQuick
import QtQuick.Controls

Button {
    id: control

    required property var theme
    required property string symbol
    required property string label
    property bool selected: false

    implicitWidth: 76
    implicitHeight: 34
    activeFocusOnTab: false

    contentItem: Row {
        anchors.centerIn: parent
        spacing: 6

        Text {
            anchors.verticalCenter: parent.verticalCenter
            text: control.symbol
            color: control.selected ? control.theme.accentColor : control.theme.mutedColor
            font.family: control.theme.monoFont
            font.pixelSize: 13
        }

        Text {
            anchors.verticalCenter: parent.verticalCenter
            text: control.label.toUpperCase()
            color: control.selected ? control.theme.inkColor : control.theme.mutedColor
            font.family: control.theme.monoFont
            font.pixelSize: 9
            font.bold: true
            font.letterSpacing: 0.2
        }
    }

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
