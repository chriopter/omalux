import QtQuick
import "keyboard.js" as Keyboard

// The editor's one keyboard focus owner. It holds active focus, receives every key and hands
// it to `shortcuts` (the binding table). The sidebar items it moves through are the
// NavTargets of the visible pane in `scope`, in displayed order; hidden panes and collapsed
// modules are never reached. Mouse clicks on controls do not keep focus: whatever non-text
// item takes it, the navigator takes it back, so keys keep working after clicks and dialogs.
// Text fields keep their keys; Escape, Enter or Down return to the navigator.
Item {
    id: root
    // Navigable area: the sidebar. Only its visible subtree is searched.
    property Item scope: null
    // Items whose focus belongs to the application (the window content); focus in popups and
    // dialogs outside it is left alone.
    property Item content: parent
    // Identifies the visible pane, to remember one selection per pane.
    property string pane: ""
    // Binding table with handle(event) → bool (EditorShortcuts).
    property var shortcuts: null
    // The keyboard selection of the visible pane (a NavTarget) or null.
    property var selection: null
    readonly property var hints: _hints
    readonly property string hintText: Keyboard.hintText(_hints)

    property var _hints: []
    property var _remembered: ({})
    property real _selectedAt: 0
    property string _pendingId: ""
    property int _pendingTries: 0
    // A graph that was given the keys with Enter (NavTarget.focusItem), until Escape.
    property Item _lent: null

    focus: true
    Keys.onPressed: event => root.handleKey(event)

    // Entry point for keys: those sent to the navigator and those a text field or other item
    // in the content did not use (Main forwards them).
    function handleKey(event) {
        const window = root.Window.window
        const focused = window ? window.activeFocusItem : null
        if (focused && focused !== root && Keyboard.isText(focused)) {
            // Typing never reaches the shortcuts. Escape and Enter leave the field.
            if (Keyboard.matches(event, "Escape")) { leaveText(focused, 0); event.accepted = true }
            else if (Keyboard.matches(event, "Return") || Keyboard.matches(event, "Enter")
                     || Keyboard.matches(event, "Down")) { leaveText(focused, 1); event.accepted = true }
            return
        }
        if (root._lent && focused && Keyboard.within(focused, root._lent)) {
            // The graph did not use this key: the keys come back to the navigator. Escape
            // only does that; any other key then works as usual.
            root._lent = null
            root.forceActiveFocus()
            if (Keyboard.matches(event, "Escape")) { event.accepted = true; return }
        }
        event.accepted = !!root.shortcuts && root.shortcuts.handle(event)
    }

    // ---- Moving -----------------------------------------------------------------------
    function targets() { return shown().filter(t => t.listed) }
    // Every visible target, also those outside the ↑/↓ order.
    function shown() { return root.scope ? Keyboard.collect(root.scope, false).map(adopt) : [] }
    function adopt(t) { t.navigator = root; return t }
    function alive(t) {
        try { return !!t && t.isNavTarget === true && !!t.owner } catch (e) { return false }
    }

    // Which item the keys act on: a click newer than the last keyboard move wins, then the
    // current selection, then what the pane remembers or marks as active.
    function resolve(list) {
        let best = null
        for (const t of list)
            if (t.claimedAt > root._selectedAt && (!best || t.claimedAt > best.claimedAt)) best = t
        if (best) return best
        const known = [root.selection, root._remembered[root.pane]]
        for (const k of known)
            if (alive(k) && list.indexOf(k) >= 0) return k
        // Re-created delegates or a collapsed module: same id, then same group.
        for (const k of known) {
            let id = "", group = ""
            try { id = k ? k.navId : ""; group = k ? k.group : "" } catch (e) {}
            const same = list.find(t => id !== "" && t.navId === id) || list.find(t => group !== "" && t.group === group)
            if (same) return same
        }
        return list.find(t => t.active) || null
    }

    function select(t, scroll) {
        if (root.selection !== t && alive(root.selection)) root.selection.current = false
        root.selection = t
        root._selectedAt = Date.now()
        if (t) {
            t.current = true
            let remembered = Object.assign({}, root._remembered)
            remembered[root.pane] = t
            root._remembered = remembered
            t.selected()
            if (scroll !== false) ensureVisible(t.owner)
        }
        updateHints()
    }
    function claimed(t) {
        if (shown().indexOf(t) >= 0) select(t, false)
    }

    function current(list) {
        const t = resolve(list || targets())
        if (t && t !== root.selection) select(t, true)
        else if (t && !t.current) t.current = true
        return t
    }
    function move(direction) {
        const list = targets()
        if (!list.length) { scrollPane(direction * 60); return }
        const now = resolve(list)
        if (!now) { select(direction > 0 ? firstEnabled(list, 0, 1) : firstEnabled(list, list.length - 1, -1)); return }
        let i = list.indexOf(now) + direction
        while (i >= 0 && i < list.length && !list[i].enabled) i += direction
        select(i >= 0 && i < list.length ? list[i] : now)
    }
    function firstEnabled(list, from, direction) {
        for (let i = from; i >= 0 && i < list.length; i += direction) if (list[i].enabled) return list[i]
        return list[from] || null
    }
    function moveEdge(last) {
        const list = targets()
        if (!list.length) { scrollPane(last ? 1e9 : -1e9); return }
        select(last ? firstEnabled(list, list.length - 1, -1) : firstEnabled(list, 0, 1))
        // The very top or bottom of the pane, not just the item.
        scrollPane(last ? 1e9 : -1e9, root.selection ? flickableOf(root.selection.owner) : null)
    }
    // By module (group): forward to the first item of the next group, back to the start of
    // this group or the previous one. Items without a group move ten at a time.
    function moveGroup(direction) {
        const list = targets()
        if (!list.length) { scrollPane(direction * 400); return }
        const now = resolve(list)
        if (!now) { moveEdge(direction < 0); return }
        const groupOf = t => t.group !== "" ? t.group : null
        let i = list.indexOf(now)
        if (groupOf(now) === null) {
            select(list[Math.max(0, Math.min(list.length - 1, i + direction * 10))]); return
        }
        if (direction > 0) {
            while (i < list.length && groupOf(list[i]) === groupOf(now)) ++i
            if (i < list.length) select(list[i]); else select(list[list.length - 1])
        } else {
            let start = i
            while (start > 0 && groupOf(list[start - 1]) === groupOf(now)) --start
            if (start < i) { select(list[start]); return }
            if (start === 0) { select(list[0]); return }
            let previous = start - 1
            while (previous > 0 && groupOf(list[previous - 1]) !== null && groupOf(list[previous - 1]) === groupOf(list[start - 1])) --previous
            select(list[previous])
        }
    }

    // ---- Acting on the selection --------------------------------------------------------
    function adjust(steps) {
        const t = current()
        if (t && t.enabled) { t.adjust(steps); ensureVisible(t.owner); Qt.callLater(validate) }
    }
    function activate() {
        const t = current()
        if (!t || !t.enabled) return
        if (t.focusItem) { root._lent = t.focusItem; t.focusItem.forceActiveFocus() }
        t.activate()
        Qt.callLater(validate)
    }
    function reset() { const t = current(); if (t && t.enabled) t.reset() }
    // Shift+R and E act on the module: through its visible heading (kind "module", same group)
    // when there is one, otherwise through the item itself.
    function groupHead(t, list) {
        return t.kind === "module" || t.group === "" ? t
             : list.find(other => other.kind === "module" && other.group === t.group) || t
    }
    function resetGroup() {
        const list = targets(), t = current(list)
        if (t && t.enabled) groupHead(t, list).resetGroup()
    }
    function toggleGroup() {
        const list = targets(), t = current(list)
        if (t && t.enabled) groupHead(t, list).toggleGroup()
    }
    // Focus the visible pane's search field, if it has one.
    function focusSearch() {
        const t = shown().find(t => t.kind === "search")
        if (!t) return false
        select(t)
        t.activate()
        return true
    }
    // A selection that disappeared (collapsed module, refreshed list) moves to its nearest
    // replacement.
    function validate() {
        const list = targets()
        const t = resolve(list)
        if (t !== root.selection) select(t, true)
        else updateHints()
    }

    // Select by id, revealing it first when it is hidden (direct shortcuts such as G/S/M).
    function selectId(id) {
        root._pendingId = id
        root._pendingTries = 0
        trySelectPending()
    }
    function trySelectPending() {
        if (root._pendingId === "") return
        const shown = targets().find(t => t.navId === root._pendingId)
        if (shown) { root._pendingId = ""; select(shown); revealLater.restart(); return }
        if (root._pendingTries++ === 0)
            for (const t of Keyboard.collect(root.scope, true))
                if (t.navId === root._pendingId) t.revealRequested()
        if (root._pendingTries < 8) pendingTimer.restart()
        else root._pendingId = ""
    }
    Timer { id: pendingTimer; interval: 16; onTriggered: root.trySelectPending() }
    // Expanding a module moves what is below it once the layout settles: check again.
    Timer { id: revealLater; interval: 32; onTriggered: if (root.alive(root.selection)) root.ensureVisible(root.selection.owner) }

    // ---- Scrolling ----------------------------------------------------------------------
    function flickableOf(item) {
        for (let p = item ? item.parent : null; p && p !== root.scope; p = p.parent)
            if (p.contentY !== undefined && p.contentItem !== undefined && p.flickableDirection !== undefined) return p
        return null
    }
    function clampScroll(flick, y) {
        const minimum = flick.originY
        const maximum = minimum + Math.max(0, flick.contentHeight - flick.height)
        const dpr = flick.Screen.devicePixelRatio || 1
        return Math.round(Math.max(minimum, Math.min(maximum, y)) * dpr) / dpr
    }
    // Bring the item into view at once, with a small margin; the minimal movement, no animation.
    function ensureVisible(item) {
        const flick = flickableOf(item)
        if (!flick || !item) return
        const p = item.mapToItem(flick.contentItem, 0, 0)
        const margin = 12
        let y = flick.contentY
        if (p.y - margin < y || item.height + 2 * margin > flick.height) y = p.y - margin
        else if (p.y + item.height + margin > y + flick.height) y = p.y + item.height + margin - flick.height
        flick.cancelFlick()
        flick.contentY = clampScroll(flick, y)
    }
    // Panes without items (Info) scroll instead.
    function scrollPane(distance, flickable) {
        if (!root.scope) return
        const flick = flickable || findVisibleFlickable(root.scope)
        if (flick) { flick.cancelFlick(); flick.contentY = clampScroll(flick, flick.contentY + distance) }
    }
    function findVisibleFlickable(item) {
        if (!item || !item.visible) return null
        if (item !== root.scope && item.contentY !== undefined && item.flickableDirection !== undefined
                && item.contentHeight > item.height) return item
        for (let i = 0; i < item.children.length; ++i) {
            const found = findVisibleFlickable(item.children[i])
            if (found) return found
        }
        return null
    }

    // ---- Hints --------------------------------------------------------------------------
    function updateHints() {
        let t = alive(root.selection) ? root.selection : null
        const list = root.scope ? Keyboard.collect(root.scope, false).filter(t => t.listed) : []
        if (!t) t = resolve(list)
        let h = []
        if (list.length > 1) h.push(["↑/↓", "SELECT"])
        if (t) h = h.concat(t.hints)
        else if (!list.length) h.push(["↑/↓", "SCROLL"])
        h.push(["TAB", "PANES"], ["?", "HELP"])
        root._hints = h
    }
    onPaneChanged: {
        // A pane starts without a marked selection; its remembered one returns with the next key.
        if (alive(root.selection)) root.selection.current = false
        root.selection = null
        Qt.callLater(updateHints)
    }
    Component.onCompleted: Qt.callLater(updateHints)
    // Keep the hints in step with the selection's own state (e.g. a module that expands).
    Connections {
        target: root.alive(root.selection) ? root.selection : null
        ignoreUnknownSignals: true
        function onHintsChanged() { root.updateHints() }
    }

    // ---- Focus --------------------------------------------------------------------------
    // Take focus back from anything in the content that is not a text field; popups, menus
    // and dialogs keep theirs until they close.
    function reclaim() {
        const window = root.Window.window
        if (!window || !root.visible) return
        const focused = window.activeFocusItem
        if (focused === root) return
        // Focus left on a container above the content (a field that dropped it) comes back too.
        if (focused && (Keyboard.isText(focused)
                        || !(Keyboard.within(focused, root.content) || Keyboard.within(root.content, focused)))) return
        if (focused && root._lent && Keyboard.within(focused, root._lent)) return
        root._lent = null
        // A combo box keeps the keys while its list is open; they come back when it closes.
        const popup = focused ? focused.popup : null
        if (popup && popup.visible !== undefined && popup.visible) {
            const back = () => { if (!popup.visible) { popup.visibleChanged.disconnect(back); Qt.callLater(root.reclaim) } }
            popup.visibleChanged.connect(back)
            return
        }
        root.forceActiveFocus()
    }
    function leaveText(field, step) {
        const t = shown().find(t => t.input === field)
        root.forceActiveFocus()
        if (!t) return
        if (t.listed) { select(t); if (step) move(step); return }
        // The global search: Enter or ↓ go to the first item of the pane.
        if (step) moveEdge(false)
    }
    Connections {
        target: root.Window.window
        function onActiveFocusItemChanged() { Qt.callLater(root.reclaim) }
    }
}
