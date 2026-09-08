import QtQuick
import QtTest
import "../../qml/components"

Item {
    width: 300
    height: 100
    readonly property var testTheme: ({ inkColor: "white", accentColor: "orange",
        mutedColor: "gray", lineColor: "gray" })

    ParameterSlider {
        id: exposure
        anchors.fill: parent
        theme: parent.testTheme
        parameterIndex: 0
        selectedParameter: 0
        label: "Exposure"
        from: -5
        to: 5
        initialValue: 0
        photoReady: true
        stepSize: 0.1
        coarseStep: 1
        decimalPlaces: 1
        suffix: " EV"
    }

    TestCase {
        name: "ParameterSlider"
        when: windowShown

        function test_fractionalExposureSteps() {
            exposure.value = 0
            exposure.nudge(1, false)
            verify(Math.abs(exposure.value - 0.1) < 0.0001)
            compare(exposure.formattedValue(), "0.1")
            exposure.nudge(-1, true)
            verify(Math.abs(exposure.value + 0.9) < 0.0001)
            compare(exposure.formattedValue(), "-0.9")
        }

        function test_resetUsesNeutralValue() {
            exposure.value = 2.4
            exposure.resetValue()
            compare(exposure.value, 0)
            compare(exposure.formattedValue(), "0.0")
        }
    }
}
