import QtQuick
import QtQuick.Controls.impl
import QtQuick.Templates as T

// Dialogs in the dark theme: a raised, rounded panel with the title in bold and the editor's
// compact outlined buttons (DialogButtonBox.qml) at the bottom right.
T.Dialog {
    id: control

    implicitWidth: Math.max(implicitBackgroundWidth + leftInset + rightInset,
                            implicitContentWidth + leftPadding + rightPadding,
                            implicitHeaderWidth,
                            implicitFooterWidth)
    implicitHeight: Math.max(implicitBackgroundHeight + topInset + bottomInset,
                             implicitContentHeight + topPadding + bottomPadding
                             + (implicitHeaderHeight > 0 ? implicitHeaderHeight + spacing : 0)
                             + (implicitFooterHeight > 0 ? implicitFooterHeight + spacing : 0))

    padding: 16
    topPadding: 12
    // Keys go to the dialog while it is open and back to the editor when it closes.
    focus: true

    background: Rectangle {
        color: control.palette.window
        border.color: control.palette.mid
        radius: 8
    }

    header: T.Label {
        text: control.title
        visible: parent?.parent === T.Overlay.overlay && control.title
        elide: T.Label.ElideRight
        color: control.palette.windowText
        font.family: control.font.family
        font.pixelSize: control.font.pixelSize + 1
        font.bold: true
        padding: 16
        bottomPadding: 0
    }

    footer: DialogButtonBox {
        visible: count > 0
    }

    T.Overlay.modal: Rectangle {
        color: Color.transparent(control.palette.shadow, 0.5)
    }

    T.Overlay.modeless: Rectangle {
        color: Color.transparent(control.palette.shadow, 0.12)
    }
}
