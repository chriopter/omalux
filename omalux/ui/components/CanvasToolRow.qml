import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

// A module row for what darktable draws on the image (graduated density's line, retouch
// shapes, liquify warps…): the label, a button that shows the tool on the photo and a short
// hint how to use it there. `active` while the photo shows this module's tool.
Item {
    id: root
    required property var theme
    property string label: ""
    property string hint: ""
    property bool active: false
    property bool editable: true
    signal activated()
    property alias navTarget: nav
    implicitHeight: column.implicitHeight + 4

    NavTarget {
        id: nav
        navId: root.label
        label: root.label
        kind: "button"
        enabled: root.editable
        activateLabel: root.active ? "" : "SHOW ON PHOTO"
        resettable: false
        onActivate: root.activated()
    }
    Column {
        id: column
        width: parent.width
        spacing: 2
        RowLayout {
            width: parent.width
            spacing: 8
            Text {
                Layout.fillWidth: true
                text: root.label
                color: nav.current ? root.theme.accent : root.theme.ink
                font: root.theme.settingsFont
                elide: Text.ElideRight
            }
            AbstractButton {
                id: button
                objectName: "canvas-show-" + root.label
                implicitWidth: buttonLabel.implicitWidth + 16
                implicitHeight: 22
                enabled: root.editable
                hoverEnabled: true
                onClicked: { nav.claim(); root.activated() }
                Accessible.name: (root.active ? "Shown on the photo: " : "Show on the photo: ") + root.label
                contentItem: Text {
                    id: buttonLabel
                    text: root.active ? "on photo" : "show on photo"
                    color: root.active || button.hovered || nav.current ? root.theme.accent : root.theme.ink
                    font: root.theme.textFont
                    horizontalAlignment: Text.AlignHCenter
                    verticalAlignment: Text.AlignVCenter
                }
                background: Rectangle {
                    radius: 4
                    color: root.active ? root.theme.active : button.hovered ? root.theme.hover : "transparent"
                    border.color: root.active || nav.current ? root.theme.accent : root.theme.line
                }
            }
        }
        Text {
            width: parent.width
            visible: root.hint !== ""
            text: root.hint
            color: root.theme.muted
            font: root.theme.textFont
            wrapMode: Text.WordWrap
        }
    }
}
