import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

Item {
    id: control

    required property var theme
    required property int parameterIndex
    required property int selectedParameter
    required property string label
    required property real from
    required property real to
    required property real initialValue
    required property bool photoReady
    property bool supported: true
    property real stepSize: 1
    property real coarseStep: stepSize * 10
    property string suffix: ""
    property bool expandable: false
    property bool expanded: false
    property alias value: slider.value
    readonly property bool selected: selectedParameter === parameterIndex

    signal expansionRequested
    signal selectionRequested(int index)
    signal valueCommitted(real value)

    implicitHeight: 44
    Accessible.role: Accessible.Slider
    Accessible.name: label
    Accessible.description: Math.round(value) + suffix

    function nudge(direction, coarse) {
        if (!photoReady || !supported)
            return
        const amount = coarse ? coarseStep : stepSize
        slider.value = Math.max(from, Math.min(to, slider.value + direction * amount))
        valueCommitted(slider.value)
    }

    function resetValue() {
        if (!supported)
            return
        slider.value = initialValue
        valueCommitted(slider.value)
    }

    ColumnLayout {
        anchors.fill: parent
        anchors.leftMargin: 12
        anchors.rightMargin: 12
        anchors.topMargin: 3
        anchors.bottomMargin: 2
        spacing: 0

        RowLayout {
            Layout.fillWidth: true

            Text {
                text: control.label.toUpperCase()
                color: control.selected ? control.theme.accentColor : control.theme.inkColor
                font.family: control.theme.monoFont
                font.pixelSize: 11
                font.bold: true
            }

            Button {
                visible: control.expandable
                implicitWidth: 20
                implicitHeight: 18
                padding: 0
                activeFocusOnTab: false
                onClicked: control.expansionRequested()
                Accessible.name: control.expanded
                    ? "Hide " + control.label + " subparameters"
                    : "Show " + control.label + " subparameters"

                contentItem: Text {
                    text: control.expanded ? "⌄" : "›"
                    color: control.theme.accentColor
                    font.family: control.theme.monoFont
                    font.pixelSize: 12
                    font.bold: true
                    horizontalAlignment: Text.AlignHCenter
                    verticalAlignment: Text.AlignVCenter
                }
                background: Item {}
            }

            Item { Layout.fillWidth: true }

            Text {
                text: Math.round(control.value) + control.suffix
                color: control.selected ? control.theme.accentColor : control.theme.inkColor
                font.family: control.theme.monoFont
                font.pixelSize: 11
                font.bold: true
            }
        }

        Slider {
            id: slider
            Layout.fillWidth: true
            Layout.preferredHeight: 16
            from: control.from
            to: control.to
            value: control.initialValue
            stepSize: control.stepSize
            snapMode: Slider.SnapAlways
            enabled: control.photoReady && control.supported
            activeFocusOnTab: false
            onPressedChanged: if (pressed)
                control.selectionRequested(control.parameterIndex)
            onMoved: control.valueCommitted(value)

            background: Rectangle {
                x: slider.leftPadding
                y: slider.topPadding + slider.availableHeight / 2 - height / 2
                width: slider.availableWidth
                height: 2
                color: control.theme.lineColor

                Rectangle {
                    width: slider.visualPosition * parent.width
                    height: parent.height
                    color: control.selected ? control.theme.accentColor : control.theme.mutedColor
                }
            }

            handle: Rectangle {
                x: slider.leftPadding + slider.visualPosition * (slider.availableWidth - width)
                y: slider.topPadding + slider.availableHeight / 2 - height / 2
                width: 8
                height: 8
                color: control.selected ? control.theme.accentColor : control.theme.mutedColor
            }
        }
    }

    HoverHandler {
        onHoveredChanged: if (hovered)
            control.selectionRequested(control.parameterIndex)
    }
}
