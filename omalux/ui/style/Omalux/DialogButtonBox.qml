import QtQuick
import QtQuick.Templates as T

// The buttons of a dialog: the editor's outlined buttons at their own width, to the right, the
// confirming one marked with the accent (Enter presses it).
T.DialogButtonBox {
    id: control

    implicitWidth: Math.max(implicitBackgroundWidth + leftInset + rightInset,
                            implicitContentWidth + leftPadding + rightPadding)
    implicitHeight: Math.max(implicitBackgroundHeight + topInset + bottomInset,
                             implicitContentHeight + topPadding + bottomPadding)
    contentWidth: (contentItem as ListView)?.contentWidth

    spacing: 8
    padding: 16
    topPadding: 4
    alignment: Qt.AlignRight

    delegate: Button {
        readonly property int role: T.DialogButtonBox.buttonRole
        // QDialogButtonBox roles: AcceptRole 0, YesRole 5, ApplyRole 8 (the enum is not
        // reachable through the template import here).
        highlighted: role === 0 || role === 5 || role === 8
        horizontalPadding: 16
    }

    contentItem: ListView {
        implicitWidth: contentWidth
        implicitHeight: 30
        model: control.contentModel
        spacing: control.spacing
        orientation: ListView.Horizontal
        boundsBehavior: Flickable.StopAtBounds
        snapMode: ListView.SnapToItem
    }

    background: Item {
        implicitHeight: 30
    }
}
