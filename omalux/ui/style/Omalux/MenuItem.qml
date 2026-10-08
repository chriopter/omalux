import QtQuick
import QtQuick.Controls.impl
import QtQuick.Templates as T

// A menu entry like the pane tabs: the hovered or keyboard-current entry gets the raised fill
// and the accent label, a disabled one a dimmed label.
T.MenuItem {
    id: control

    implicitWidth: Math.max(implicitBackgroundWidth + leftInset + rightInset,
                            implicitContentWidth + leftPadding + rightPadding)
    implicitHeight: Math.max(implicitBackgroundHeight + topInset + bottomInset,
                             implicitContentHeight + topPadding + bottomPadding,
                             implicitIndicatorHeight + topPadding + bottomPadding)

    verticalPadding: 5
    horizontalPadding: 10
    spacing: 8

    icon.width: 16
    icon.height: 16

    readonly property color labelColor: !control.enabled ? Color.transparent(control.palette.windowText, 0.4)
                                        : control.highlighted || control.down ? control.palette.highlight
                                        : control.palette.windowText

    contentItem: IconLabel {
        readonly property real arrowPadding: control.subMenu && control.arrow ? control.arrow.width + control.spacing : 0
        readonly property real indicatorPadding: control.checkable && control.indicator ? control.indicator.width + control.spacing : 0
        leftPadding: !control.mirrored ? indicatorPadding : arrowPadding
        rightPadding: control.mirrored ? indicatorPadding : arrowPadding

        spacing: control.spacing
        mirrored: control.mirrored
        display: control.display
        alignment: Qt.AlignLeft

        icon: control.icon
        text: control.text
        font: control.font
        color: control.labelColor
        defaultIconColor: control.labelColor
    }

    indicator: ColorImage {
        x: control.mirrored ? control.width - width - control.rightPadding : control.leftPadding
        y: control.topPadding + (control.availableHeight - height) / 2
        width: 14
        height: 14
        visible: control.checked
        source: control.checkable ? "qrc:/qt-project.org/imports/QtQuick/Controls/Basic/images/check.png" : ""
        color: control.labelColor
    }

    arrow: ColorImage {
        x: control.mirrored ? control.leftPadding : control.width - width - control.rightPadding
        y: control.topPadding + (control.availableHeight - height) / 2
        visible: control.subMenu
        mirror: control.mirrored
        source: control.subMenu ? "qrc:/qt-project.org/imports/QtQuick/Controls/Basic/images/arrow-indicator.png" : ""
        color: control.labelColor
    }

    background: Rectangle {
        implicitWidth: 160
        implicitHeight: 26
        radius: 3
        color: control.down ? control.palette.button : control.highlighted ? control.palette.button : "transparent"
        border.color: control.palette.highlight
        border.width: Qt.styleHints.accessibility.contrastPreference === Qt.HighContrast && control.highlighted ? 1 : 0
    }
}
