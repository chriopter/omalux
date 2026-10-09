import QtQuick
import QtQuick.Controls

// A plain slider for dialogs, like the editor's own: a 3 px track, filled up to a small round
// knob; the knob's ring takes the accent while it has the keys or is dragged.
Slider {
    id: control

    padding: 0

    readonly property int trackTop: topPadding + Math.floor((availableHeight - 3) / 2)

    handle: Rectangle {
        x: control.leftPadding + Math.round(control.visualPosition * (control.availableWidth - width))
        y: control.trackTop + 1.5 - height / 2
        implicitWidth: 13
        implicitHeight: 13
        radius: width / 2
        color: control.palette.window
        border.width: control.visualFocus || control.pressed ? 2 : 1
        border.color: !control.enabled ? control.palette.mid
                      : control.visualFocus || control.pressed ? control.palette.highlight : control.palette.windowText
    }

    background: Rectangle {
        x: control.leftPadding
        y: control.trackTop
        implicitWidth: 200
        implicitHeight: 22
        width: control.availableWidth
        height: 3
        radius: 1.5
        color: control.palette.mid
        Rectangle {
            width: control.handle ? control.handle.x + control.handle.width / 2 - control.leftPadding : 0
            height: parent.height
            radius: 1.5
            color: control.enabled ? control.palette.windowText : control.palette.mid
        }
    }
}
