.pragma library

// Scrolling of the sidebar panes around content that opens or closes under the pointer.
// Pure functions on the item's enclosing Flickable; SidebarScrollView releases the extra
// bottom margin that hold() adds once the person scrolls up.

function flickableOf(item) {
    for (let p = item ? item.parent : null; p; p = p.parent)
        if (p.contentY !== undefined && p.contentItem !== undefined && p.flickableDirection !== undefined) return p
    return null
}

function limit(flick) {
    return flick.originY + Math.max(0, flick.contentHeight - flick.height)
}

// Call before content shrinks. The pane stays where it was (a bottom margin makes up for the
// lost height), so what was under the pointer stays there instead of the pane jumping to its
// new end. Applies when the content height changes; the returned function ends the watch.
function hold(item) {
    const flick = flickableOf(item)
    if (!flick) return function() {}
    const y = flick.contentY
    function keep() {
        const slack = Math.max(0, y - limit(flick))
        if (slack > flick.bottomMargin) flick.bottomMargin = slack
        if (slack > 0) flick.contentY = y
    }
    flick.contentHeightChanged.connect(keep)
    return function() { flick.contentHeightChanged.disconnect(keep) }
}

// Bring the content from `top` down to `bottom` of `item` into view after it opened, keeping
// `top` on screen; nothing moves when it is already visible.
function reveal(item, top, bottom) {
    const flick = flickableOf(item)
    if (!flick) return
    const a = item.mapToItem(flick.contentItem, 0, top).y - 12
    const b = item.mapToItem(flick.contentItem, 0, bottom).y + 12
    if (b <= flick.contentY + flick.height || a < flick.contentY) return
    flick.cancelFlick()
    flick.contentY = Math.max(flick.originY, Math.min(limit(flick) + flick.bottomMargin, Math.min(a, b - flick.height)))
}

// Where the pane of `item` stands; a caller that follows growing content compares it with the
// position after its last reveal() and stops once the person has scrolled in between.
function position(item) {
    const flick = flickableOf(item)
    return flick ? flick.contentY : 0
}
