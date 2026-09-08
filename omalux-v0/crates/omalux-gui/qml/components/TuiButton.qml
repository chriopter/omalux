import QtQuick
import QtQuick.Controls

Button {
    id: control

    required property var theme
    property bool primary: false

    activeFocusOnTab: false
    leftPadding: 11
    rightPadding: 11
    topPadding: 6
    bottomPadding: 6

    contentItem: Text {
        text: control.text
        color: control.enabled
            ? (control.primary ? control.theme.pageColor : control.theme.inkColor)
            : control.theme.mutedColor
        horizontalAlignment: Text.AlignHCenter
        verticalAlignment: Text.AlignVCenter
        font.family: control.theme.monoFont
        font.pixelSize: 12
        font.bold: control.primary
    }

    background: Rectangle {
        implicitHeight: 30
        implicitWidth: 56
        color: control.primary
            ? (control.down ? Qt.darker(control.theme.inkColor, 1.25)
                            : control.theme.inkColor)
            : control.down ? control.theme.raisedColor
                           : control.hovered ? control.theme.raisedColor : "transparent"
        border.width: 1
        border.color: control.primary
            ? control.theme.inkColor
            : control.hovered ? control.theme.mutedColor : control.theme.lineColor
    }
}
