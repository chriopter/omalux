import QtQuick
import QtQuick.Controls

ApplicationWindow {
    width: 800
    height: 520
    visible: true
    title: "Omalux"
    color: "#171923"

    Column {
        anchors.centerIn: parent
        spacing: 16
        Label {
            anchors.horizontalCenter: parent.horizontalCenter
            text: "Hello, Omalux."
            color: "#eef0f6"
            font.pixelSize: 36
        }
        Label {
            anchors.horizontalCenter: parent.horizontalCenter
            text: "Our UI. darktable’s image processing."
            color: "#a6adc3"
            font.pixelSize: 16
        }
    }
}
