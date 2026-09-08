import QtQuick

Item {
    id: root
    required property string moduleKey
    implicitWidth: 16
    implicitHeight: 16
    Accessible.ignored: true
    Image {
        anchors.fill: parent
        source: root.moduleKey ? Qt.resolvedUrl("../../../assets/icons/module-" + root.moduleKey + ".svg") : ""
        sourceSize.width: 32
        sourceSize.height: 32
        fillMode: Image.PreserveAspectFit
    }
}
