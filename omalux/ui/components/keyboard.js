.pragma library

// Key matching and target ordering for KeyboardNavigator and EditorShortcuts. Pure functions,
// no QML scope.

const named = {
    "Up": 0x01000013, "Down": 0x01000015, "Left": 0x01000012, "Right": 0x01000014,
    "Home": 0x01000010, "End": 0x01000011, "PgUp": 0x01000016, "PgDown": 0x01000017,
    "Return": 0x01000004, "Enter": 0x01000005, "Space": 0x20, "Tab": 0x01000001,
    "Backtab": 0x01000002, "Escape": 0x01000000, "F1": 0x01000030
}
const SHIFT = 0x02000000, CTRL = 0x04000000, ALT = 0x08000000, META = 0x10000000

// "Ctrl+Shift+R", "Shift+Tab", "?", "Ctrl++" → { key, modifiers, symbol }
function parse(spec) {
    let key = spec, prefix = ""
    if (spec.length > 1 && spec.endsWith("++")) { key = "+"; prefix = spec.slice(0, -2) }
    else if (spec.lastIndexOf("+") > 0) { key = spec.slice(spec.lastIndexOf("+") + 1); prefix = spec.slice(0, spec.lastIndexOf("+")) }
    let modifiers = 0
    for (const m of prefix ? prefix.split("+") : []) {
        if (m === "Shift") modifiers |= SHIFT
        else if (m === "Ctrl") modifiers |= CTRL
        else if (m === "Alt") modifiers |= ALT
        else if (m === "Meta") modifiers |= META
    }
    let code = named[key]
    if (code === undefined) code = key.toUpperCase().charCodeAt(0)
    // Printable non-letters such as ?, +, / sit on shifted keys in many layouts: the
    // character identifies them, Shift does not.
    const symbol = key.length === 1 && !/[A-Za-z]/.test(key)
    return { key: code, modifiers: modifiers, symbol: symbol }
}

function matches(event, spec) {
    const want = typeof spec === "string" ? parse(spec) : spec
    let key = event.key, modifiers = event.modifiers & (SHIFT | CTRL | ALT | META)
    if (key === named.Backtab) { key = named.Tab; modifiers |= SHIFT }
    if (key !== want.key) return false
    if (want.symbol) modifiers &= ~SHIFT
    return modifiers === want.modifiers
}

function isText(item) {
    return !!item && typeof item.selectAll === "function" && item.cursorPosition !== undefined
}

function within(item, ancestor) {
    for (let p = item; p; p = p.parent) if (p === ancestor) return true
    return false
}

// Every NavTarget below root, in displayed order (top to bottom, then left to right).
// Invisible subtrees are skipped unless hidden targets are asked for.
function collect(root, includeHidden) {
    const found = []
    function walk(item) {
        if (!item || (!includeHidden && (!item.visible || item.opacity === 0))) return
        const resources = item.resources
        if (resources)
            for (let i = 0; i < resources.length; ++i) {
                const r = resources[i]
                if (r && r.isNavTarget === true) { r.owner = item; found.push(r) }
            }
        const children = item.children
        for (let i = 0; i < children.length; ++i) walk(children[i])
        // Positioners (Column, Row) and views place new children only once per frame; place
        // them now, inner first, so the order below sees where items are about to be shown.
        if (!includeHidden && typeof item.forceLayout === "function") item.forceLayout()
    }
    walk(root)
    const visible = found.filter(t => t.owner.visible && t.owner.height > 0)
    const list = includeHidden ? found : visible
    const position = new Map()
    for (const t of list) {
        const p = t.owner.mapToItem(root, 0, 0)
        position.set(t, [Math.round(p.y), Math.round(p.x)])
    }
    return list.sort((a, b) => {
        const pa = position.get(a), pb = position.get(b)
        return pa[0] - pb[0] || pa[1] - pb[1]
    })
}

// The shown key hint line: "[↑/↓] SELECT   [←/→] exposure   …"
function hintText(hints) {
    return hints.map(h => "[" + h[0] + "] " + h[1]).join("   ")
}
