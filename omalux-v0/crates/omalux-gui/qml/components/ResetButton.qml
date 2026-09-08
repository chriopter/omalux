import QtQuick
import QtQuick.Controls

Button {
    id: control

    required property var theme
    property string accessibleLabel: text
    property bool compact: false

    activeFocusOnTab: false
    hoverEnabled: true
    padding: 0
    implicitWidth: compact ? 20 : Math.max(52, contentItem.implicitWidth + 12)
    implicitHeight: compact ? 18 : 24
    Accessible.name: accessibleLabel

    contentItem: Text {
        text: control.text
        color: control.enabled
            ? (control.hovered ? control.theme.accentColor : control.theme.mutedColor)
            : control.theme.lineColor
        horizontalAlignment: Text.AlignHCenter
        verticalAlignment: Text.AlignVCenter
        font.family: control.theme.monoFont
        font.pixelSize: control.compact ? 12 : 10
        font.bold: true
    }

    background: Rectangle {
        color: control.down ? control.theme.raisedColor : "transparent"
        border.width: control.hovered ? 1 : 0
        border.color: control.theme.lineColor
    }
}
