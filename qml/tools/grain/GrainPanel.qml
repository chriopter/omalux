import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import io.omacom.grainroom

Item {
    id: panel

    required property var theme
    required property bool photoReady
    required property int selectedParameter
    required property bool advancedExpanded

    property alias grainValue: grainControl.value
    property alias grainSizeValue: grainSizeControl.value
    property alias midtonesValue: midtonesControl.value

    signal selectionRequested(int index)
    signal advancedToggleRequested

    function parameterAt(index) {
        switch (index) {
        case 0: return grainControl
        case 1: return grainSizeControl
        case 2: return midtonesControl
        case 3: return exposureControl
        case 4: return contrastControl
        case 5: return highlightsControl
        case 6: return shadowsControl
        default: return vignetteControl
        }
    }

    function ensureVisible(control) {
        const flickable = grainScroll.flickable
        const point = control.mapToItem(flickable, 0, 0)
        if (point.y < 0)
            flickable.contentY = Math.max(0, flickable.contentY + point.y - 4)
        else if (point.y + control.height > grainScroll.availableHeight)
            flickable.contentY += point.y + control.height - grainScroll.availableHeight + 4
    }

    ScrollView {
        id: grainScroll
        readonly property Flickable flickable: contentItem as Flickable
        anchors.fill: parent
        clip: true
        contentWidth: availableWidth
        ScrollBar.horizontal.policy: ScrollBar.AlwaysOff
        ScrollBar.vertical.policy: ScrollBar.AsNeeded

        ColumnLayout {
            width: grainScroll.availableWidth
            spacing: 8

            ParameterSlider {
                id: grainControl
                Layout.fillWidth: true
                theme: panel.theme
                photoReady: panel.photoReady
                selectedParameter: panel.selectedParameter
                parameterIndex: 0
                label: "Grain"
                from: 0
                to: 100
                initialValue: 24
                stepSize: 1
                coarseStep: 10
                expandable: true
                expanded: panel.advancedExpanded
                onSelectionRequested: index => panel.selectionRequested(index)
                onExpansionRequested: panel.advancedToggleRequested()
            }

            Rectangle {
                Layout.fillWidth: true
                implicitHeight: grainSubparameters.implicitHeight + 16
                visible: panel.advancedExpanded
                color: "transparent"
                border.width: 1
                border.color: panel.theme.lineColor

                ColumnLayout {
                    id: grainSubparameters
                    anchors.fill: parent
                    anchors.margins: 8
                    spacing: 6

                    ParameterSlider {
                        id: grainSizeControl
                        Layout.fillWidth: true
                        theme: panel.theme
                        photoReady: panel.photoReady
                        selectedParameter: panel.selectedParameter
                        parameterIndex: 1
                        label: "Size"
                        from: 20
                        to: 6400
                        initialValue: 4000
                        stepSize: 100
                        coarseStep: 500
                        suffix: " ISO"
                        onSelectionRequested: index => panel.selectionRequested(index)
                    }

                    ParameterSlider {
                        id: midtonesControl
                        Layout.fillWidth: true
                        theme: panel.theme
                        photoReady: panel.photoReady
                        selectedParameter: panel.selectedParameter
                        parameterIndex: 2
                        label: "Midtones"
                        from: 0
                        to: 100
                        initialValue: 100
                        stepSize: 1
                        coarseStep: 10
                        onSelectionRequested: index => panel.selectionRequested(index)
                    }
                }
            }

            MockParameterSlider {
                id: exposureControl
                Layout.fillWidth: true
                theme: panel.theme
                photoReady: panel.photoReady
                selectedParameter: panel.selectedParameter
                parameterIndex: 3
                label: "Exposure"
                from: -100
                to: 100
                initialValue: 0
                onSelectionRequested: index => panel.selectionRequested(index)
            }

            MockParameterSlider {
                id: contrastControl
                Layout.fillWidth: true
                theme: panel.theme
                photoReady: panel.photoReady
                selectedParameter: panel.selectedParameter
                parameterIndex: 4
                label: "Contrast"
                onSelectionRequested: index => panel.selectionRequested(index)
            }

            MockParameterSlider {
                id: highlightsControl
                Layout.fillWidth: true
                theme: panel.theme
                photoReady: panel.photoReady
                selectedParameter: panel.selectedParameter
                parameterIndex: 5
                label: "Highlights"
                initialValue: 62
                onSelectionRequested: index => panel.selectionRequested(index)
            }

            MockParameterSlider {
                id: shadowsControl
                Layout.fillWidth: true
                theme: panel.theme
                photoReady: panel.photoReady
                selectedParameter: panel.selectedParameter
                parameterIndex: 6
                label: "Shadows"
                initialValue: 38
                onSelectionRequested: index => panel.selectionRequested(index)
            }

            MockParameterSlider {
                id: vignetteControl
                Layout.fillWidth: true
                theme: panel.theme
                photoReady: panel.photoReady
                selectedParameter: panel.selectedParameter
                parameterIndex: 7
                label: "Vignette"
                initialValue: 18
                onSelectionRequested: index => panel.selectionRequested(index)
            }
        }
    }
}
