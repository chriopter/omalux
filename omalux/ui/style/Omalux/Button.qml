import QtQuick
import QtQuick.Controls.impl
import QtQuick.Templates as T

// A plain button (Edit crop, Save current look…) like the toolbar buttons: outlined on the
// dark background, the raised fill on hover, the accent for hover, focus and "highlighted"
// (the keyboard selection). Buttons with their own background keep it.
T.Button {
    id: control

    implicitWidth: Math.max(implicitBackgroundWidth + leftInset + rightInset,
                            implicitContentWidth + leftPadding + rightPadding)
    implicitHeight: Math.max(implicitBackgroundHeight + topInset + bottomInset,
                             implicitContentHeight + topPadding + bottomPadding)

    padding: 6
    horizontalPadding: padding + 2
    spacing: 6
    hoverEnabled: true

    icon.width: 16
    icon.height: 16

    contentItem: IconLabel {
        spacing: control.spacing
        mirrored: control.mirrored
        display: control.display

        icon: control.icon
        defaultIconColor: !control.enabled ? Color.transparent(control.palette.windowText, 0.4)
                          : control.hovered || control.highlighted || control.visualFocus || control.checked
                            ? control.palette.highlight : control.palette.buttonText
        text: control.text
        font: control.font
        color: defaultIconColor
    }

    background: Rectangle {
        implicitWidth: 64
        implicitHeight: 30
        radius: 5
        visible: !control.flat || control.down || control.checked || control.highlighted || control.hovered
        color: control.down || control.checked ? control.palette.button
               : control.hovered && control.enabled ? Qt.lighter(control.palette.window, 1.3) : "transparent"
        border.width: 1
        border.color: control.visualFocus || control.highlighted ? control.palette.highlight : control.palette.mid
    }
}
