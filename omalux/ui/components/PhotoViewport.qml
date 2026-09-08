import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

Rectangle {
    id: root
    required property var theme
    required property string preview
    required property string status
    color: "#0b0b0d"
    Image {
        id: photo
        anchors.fill: parent
        anchors.margins: 20
        source: root.preview
        fillMode: Image.PreserveAspectFit
        cache: false
        asynchronous: false
    }
    Text {
        anchors.centerIn: parent
        visible: root.preview === ""
        text: root.status
        color: root.theme.muted
        font: root.theme.textFont
        width: parent.width - 40
        wrapMode: Text.WordWrap
        horizontalAlignment: Text.AlignHCenter
    }
}
