import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import io.omacom.grainroom

Item {
    id: panel

    required property var theme
    required property bool photoReady
    property int selectedIndex: 0
    property int selectedRatio: 1
    property bool landscape: false
    property int gridIndex: 0

    readonly property var aspectRatios: [
        "FREE",
        "AS SHOT  ·  3024:4032",
        "CUSTOM  ·  10 H : 10 W",
        "1:1  SQUARE",
        "2:3  35MM",
        "3:4  CLASSIC",
        "4:5  PORTRAIT",
        "5:7  PRINT",
        "9:16  STORY",
        "1:1.414  ISO A4",
        "8.5:11  US LETTER"
    ]

    function parameterAt(index) {
        switch (index) {
        case 0: return straightenControl
        case 1: return horizontalControl
        default: return verticalControl
        }
    }

    function moveSelection(direction) {
        selectedIndex = (selectedIndex + direction + 3) % 3
    }

    function adjustSelection(direction, coarse) {
        parameterAt(selectedIndex).nudge(direction, coarse)
    }

    function resetSelection() {
        parameterAt(selectedIndex).resetValue()
    }

    component CropActionButton: Button {
        id: action
        required property string symbol
        required property string description

        implicitWidth: 48
        implicitHeight: 34
        enabled: panel.photoReady
        activeFocusOnTab: false
        Accessible.name: description

        contentItem: Text {
            text: action.symbol
            color: action.enabled ? panel.theme.inkColor : panel.theme.mutedColor
            font.family: panel.theme.monoFont
            font.pixelSize: 17
            horizontalAlignment: Text.AlignHCenter
            verticalAlignment: Text.AlignVCenter
        }

        background: Rectangle {
            color: action.down ? panel.theme.raisedColor : "transparent"
            border.width: 1
            border.color: action.hovered ? panel.theme.accentColor : panel.theme.lineColor
        }
    }

    ColumnLayout {
        anchors.fill: parent
        spacing: 8

        RowLayout {
            Layout.fillWidth: true

            Text {
                text: "01 / CROP"
                color: panel.theme.inkColor
                font.family: panel.theme.monoFont
                font.pixelSize: 13
                font.bold: true
                font.letterSpacing: 1
            }

            Item { Layout.fillWidth: true }

            Text {
                text: "UI MOCK"
                color: panel.theme.accentColor
                font.family: panel.theme.monoFont
                font.pixelSize: 9
                font.bold: true
            }
        }

        ScrollView {
            id: cropScroll
            Layout.fillWidth: true
            Layout.fillHeight: true
            clip: true
            contentWidth: availableWidth
            ScrollBar.horizontal.policy: ScrollBar.AlwaysOff
            ScrollBar.vertical.policy: ScrollBar.AsNeeded

            ColumnLayout {
                width: cropScroll.availableWidth
                spacing: 7

                RowLayout {
                    Layout.fillWidth: true
                    spacing: 6

                    CropActionButton {
                        Layout.fillWidth: true
                        symbol: "↶"
                        description: "Rotate left"
                    }
                    CropActionButton {
                        Layout.fillWidth: true
                        symbol: "↷"
                        description: "Rotate right"
                    }
                    CropActionButton {
                        Layout.fillWidth: true
                        symbol: "↔"
                        description: "Flip horizontally"
                    }
                    CropActionButton {
                        Layout.fillWidth: true
                        symbol: "↕"
                        description: "Flip vertically"
                    }
                }

                ParameterSlider {
                    id: straightenControl
                    Layout.fillWidth: true
                    theme: panel.theme
                    photoReady: panel.photoReady
                    selectedParameter: panel.selectedIndex
                    parameterIndex: 0
                    label: "Straighten"
                    from: -45
                    to: 45
                    initialValue: 0
                    suffix: "°"
                    onSelectionRequested: index => panel.selectedIndex = index
                }

                ParameterSlider {
                    id: horizontalControl
                    Layout.fillWidth: true
                    theme: panel.theme
                    photoReady: panel.photoReady
                    selectedParameter: panel.selectedIndex
                    parameterIndex: 1
                    label: "Horizontal"
                    from: -100
                    to: 100
                    initialValue: 0
                    onSelectionRequested: index => panel.selectedIndex = index
                }

                ParameterSlider {
                    id: verticalControl
                    Layout.fillWidth: true
                    theme: panel.theme
                    photoReady: panel.photoReady
                    selectedParameter: panel.selectedIndex
                    parameterIndex: 2
                    label: "Vertical"
                    from: -100
                    to: 100
                    initialValue: 0
                    onSelectionRequested: index => panel.selectedIndex = index
                }

                Rectangle {
                    Layout.fillWidth: true
                    Layout.preferredHeight: 1
                    color: panel.theme.lineColor
                }

                RowLayout {
                    Layout.fillWidth: true

                    Text {
                        text: "GRID"
                        color: panel.theme.mutedColor
                        font.family: panel.theme.monoFont
                        font.pixelSize: 10
                        font.bold: true
                    }

                    Item { Layout.fillWidth: true }

                    Button {
                        implicitWidth: 104
                        implicitHeight: 28
                        activeFocusOnTab: false
                        onClicked: panel.gridIndex = (panel.gridIndex + 1) % 3

                        contentItem: Text {
                            text: ["▦  3×3", "φ  GOLDEN", "╱  DIAGONAL"][panel.gridIndex]
                            color: panel.theme.inkColor
                            font.family: panel.theme.monoFont
                            font.pixelSize: 10
                            horizontalAlignment: Text.AlignHCenter
                            verticalAlignment: Text.AlignVCenter
                        }
                        background: Rectangle {
                            color: parent.down ? panel.theme.raisedColor : panel.theme.surfaceColor
                            border.width: 1
                            border.color: panel.theme.lineColor
                        }
                    }
                }

                RowLayout {
                    Layout.fillWidth: true

                    Text {
                        text: "ORIENTATION"
                        color: panel.theme.mutedColor
                        font.family: panel.theme.monoFont
                        font.pixelSize: 10
                        font.bold: true
                    }

                    Item { Layout.fillWidth: true }

                    Rectangle {
                        implicitWidth: 104
                        implicitHeight: 30
                        color: panel.theme.surfaceColor
                        border.width: 1
                        border.color: panel.theme.lineColor

                        Row {
                            anchors.fill: parent

                            Repeater {
                                model: ["▯", "▭"]

                                delegate: Rectangle {
                                    id: orientationOption
                                    required property int index
                                    required property string modelData
                                    width: 52
                                    height: 30
                                    color: panel.landscape === (index === 1)
                                        ? panel.theme.selectionColor : "transparent"

                                    Text {
                                        anchors.centerIn: parent
                                        text: orientationOption.modelData
                                        color: panel.landscape === (orientationOption.index === 1)
                                            ? panel.theme.inkColor : panel.theme.mutedColor
                                        font.family: panel.theme.monoFont
                                        font.pixelSize: 15
                                    }

                                    TapHandler {
                                        onTapped: panel.landscape = orientationOption.index === 1
                                    }
                                }
                            }
                        }
                    }
                }

                Text {
                    text: "ASPECT RATIO"
                    color: panel.theme.accentColor
                    font.family: panel.theme.monoFont
                    font.pixelSize: 10
                    font.bold: true
                }

                Repeater {
                    model: panel.aspectRatios

                    delegate: Rectangle {
                        id: ratioOption
                        required property int index
                        required property string modelData

                        Layout.fillWidth: true
                        implicitHeight: 31
                        color: panel.selectedRatio === index
                            ? panel.theme.selectionColor : "transparent"
                        border.width: panel.selectedRatio === index ? 1 : 0
                        border.color: panel.theme.accentColor

                        RowLayout {
                            anchors.fill: parent
                            anchors.leftMargin: 8
                            anchors.rightMargin: 8

                            Text {
                                text: panel.selectedRatio === ratioOption.index ? "✓" : " "
                                color: panel.theme.accentColor
                                font.family: panel.theme.monoFont
                                font.pixelSize: 10
                            }

                            Text {
                                Layout.fillWidth: true
                                text: ratioOption.modelData
                                color: panel.selectedRatio === ratioOption.index
                                    ? panel.theme.inkColor : panel.theme.mutedColor
                                font.family: panel.theme.monoFont
                                font.pixelSize: 10
                            }
                        }

                        TapHandler {
                            onTapped: panel.selectedRatio = ratioOption.index
                        }
                    }
                }
            }
        }
    }
}
