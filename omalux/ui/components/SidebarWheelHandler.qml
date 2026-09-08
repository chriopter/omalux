import QtQuick
import QtQuick.Window

WheelHandler {
    required property var flickable
    target: null
    acceptedDevices: PointerDevice.Mouse | PointerDevice.TouchPad
    readonly property real pixelsPerNotch: 120
    readonly property real touchpadSpeed: 4
    onFlickableChanged: if (flickable) flickable.pixelAligned = false

    onWheel: event => {
        // Follow Omawrite's event-shape distinction: Wayland seat devices can
        // report TouchPad even for angle-only events. Never discard those.
        const distance = event.pixelDelta.y !== 0 ? event.pixelDelta.y * touchpadSpeed
            : event.angleDelta.y / 120 * pixelsPerNotch
        if (distance === 0) {
            event.accepted = true
            return
        }
        flickable.cancelFlick()
        const minimum = flickable.originY
        const maximum = minimum + Math.max(0, flickable.contentHeight - flickable.height)
        const next = Math.max(minimum, Math.min(maximum, flickable.contentY - distance))
        // QML coordinates and wheel deltas are logical pixels. Use the display
        // scale only to align the result to physical pixels, not as a speed gain.
        const dpr = flickable.Screen.devicePixelRatio
        flickable.contentY = Math.max(minimum, Math.min(maximum, Math.round(next * dpr) / dpr))
        event.accepted = true
    }
}
