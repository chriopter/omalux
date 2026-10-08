import QtQuick
import QtQuick.Controls.impl
import QtQuick.Templates as T

// Every menu of the editor (right-click, instance, row, choice and style menus) in the dark
// theme. The padding keeps the first item off the pointer: a menu opened at the pointer starts
// with nothing hovered, as darktable's popups do.
T.Menu {
    id: control

    implicitWidth: Math.max(implicitBackgroundWidth + leftInset + rightInset,
                            implicitContentWidth + leftPadding + rightPadding)
    implicitHeight: Math.max(implicitBackgroundHeight + topInset + bottomInset,
                             implicitContentHeight + topPadding + bottomPadding)

    margins: 0
    padding: 4
    overlap: 1

    delegate: MenuItem { }

    contentItem: ListView {
        // As wide as the widest entry, so no label is cut ("drawn & parametric mask").
        implicitWidth: {
            let w = 0
            for (let i = 0; i < control.count; ++i) {
                const item = control.itemAt(i)
                if (item && item.visible) w = Math.max(w, item.implicitWidth)
            }
            return w
        }
        implicitHeight: contentHeight
        model: control.contentModel
        interactive: Window.window
                     ? contentHeight + control.topPadding + control.bottomPadding > control.height
                     : false
        clip: true
        currentIndex: control.currentIndex

        ScrollIndicator.vertical: ScrollIndicator {}
    }

    background: Rectangle {
        implicitWidth: 160
        implicitHeight: 32
        color: control.palette.window
        border.color: control.palette.mid
        radius: 4
    }

    T.Overlay.modal: Rectangle {
        color: Color.transparent(control.palette.shadow, 0.5)
    }

    T.Overlay.modeless: Rectangle {
        color: Color.transparent(control.palette.shadow, 0.12)
    }
}
