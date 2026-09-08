import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

Rectangle {
    id: root
    required property var theme
    required property string message
    implicitHeight: gpuNotice.implicitHeight + 22
    visible: root.message !== ""
    color: "#342d22"
    Accessible.role: Accessible.AlertMessage
    Accessible.name: root.message
    Text {
        id: gpuNotice
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.leftMargin: 16
        anchors.rightMargin: 16
        anchors.verticalCenter: parent.verticalCenter
        text: "⚠  " + root.message
        color: "#f9d58b"
        font: root.theme.textFont
        wrapMode: Text.WordWrap
    }
}
