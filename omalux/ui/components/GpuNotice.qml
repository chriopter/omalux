import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

// The warning that rendering runs without the GPU. It can be put away with its × (it returns
// when the message changes) and stays out of the fullscreen photograph.
Rectangle {
    id: root
    required property var theme
    required property string message
    // Hidden without forgetting the message (photograph fullscreen).
    property bool suppressed: false
    // The message the person closed.
    property string dismissed: ""
    implicitHeight: gpuNotice.implicitHeight + 22
    visible: root.message !== "" && root.message !== root.dismissed && !root.suppressed
    color: "#342d22"
    Accessible.role: Accessible.AlertMessage
    Accessible.name: root.message
    Text {
        id: gpuNotice
        anchors.left: parent.left
        anchors.right: closeButton.left
        anchors.leftMargin: 16
        anchors.rightMargin: 8
        anchors.verticalCenter: parent.verticalCenter
        text: "⚠  " + root.message
        color: "#f9d58b"
        font: root.theme.textFont
        wrapMode: Text.WordWrap
    }
    AbstractButton {
        id: closeButton
        objectName: "gpu-notice-close"
        anchors.right: parent.right
        anchors.rightMargin: 8
        anchors.verticalCenter: parent.verticalCenter
        width: 28; height: 28
        hoverEnabled: true
        focusPolicy: Qt.NoFocus
        onClicked: root.dismissed = root.message
        Accessible.name: "Hide this warning"
        ToolTip.visible: hovered && !pressed
        ToolTip.delay: 500
        ToolTip.text: "Hide this warning"
        contentItem: Text {
            text: "×"
            color: "#f9d58b"
            opacity: closeButton.hovered ? 1 : .7
            font: root.theme.settingsFont
            horizontalAlignment: Text.AlignHCenter
            verticalAlignment: Text.AlignVCenter
        }
        background: Rectangle {
            radius: 5
            color: closeButton.pressed ? "#5a4c33" : closeButton.hovered ? "#4a3f2c" : "transparent"
        }
    }
}
