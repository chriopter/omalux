import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

// A parameter that is either on or off. The track fills towards the accent when set, so the
// state reads from the shape alone rather than from the word next to it.
Item {
    id: root
    required property var theme
    required property string label
    required property real value
    required property bool editable
    signal edited(real value)

    readonly property bool on: value > 0.5
    implicitHeight: 26

    RowLayout {
        anchors.fill: parent
        anchors.leftMargin: 8
        anchors.rightMargin: 8
        spacing: 8
        Text {
            Layout.fillWidth: true
            text: root.label
            color: root.theme.muted
            font: root.theme.textFont
            elide: Text.ElideRight
        }
        Rectangle {
            implicitWidth: 26
            implicitHeight: 13
            radius: 6.5
            color: root.on ? Qt.rgba(root.theme.accent.r, root.theme.accent.g, root.theme.accent.b, 0.35)
                           : Qt.lighter(root.theme.background, 1.3)
            border.color: root.on ? root.theme.accent : root.theme.line
            opacity: root.editable ? 1 : 0.5
            Rectangle {
                width: 9
                height: 9
                radius: 4.5
                y: 2
                x: root.on ? parent.width - width - 2 : 2
                color: root.on ? root.theme.accent : root.theme.muted
                Behavior on x { NumberAnimation { duration: 90 } }
            }
        }
    }

    MouseArea {
        anchors.fill: parent
        enabled: root.editable
        onClicked: root.edited(root.on ? 0 : 1)
    }
}
