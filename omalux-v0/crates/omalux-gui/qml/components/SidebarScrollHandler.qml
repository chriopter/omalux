import QtQuick

WheelHandler {
    required property Flickable flickable
    onFlickableChanged: if (flickable) flickable.pixelAligned = false
    target: null
    acceptedDevices: PointerDevice.Mouse | PointerDevice.TouchPad

    // Keep fractional touchpad movement and the system's scroll direction.
    // A mouse notch advances roughly two compact preset rows.
    function scroll(pixelDelta, angleDelta) {
        const delta = pixelDelta !== 0 ? pixelDelta * 3 : angleDelta / 120 * 144
        const top = flickable.originY
        const bottom = top + Math.max(0, flickable.contentHeight - flickable.height)
        flickable.contentY = Math.max(top, Math.min(bottom, flickable.contentY - delta))
    }

    onWheel: event => {
        scroll(event.pixelDelta.y, event.angleDelta.y)
        event.accepted = true
    }
}
