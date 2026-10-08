import QtQuick

// Sends generated-module edits to the engine one request at a time. The engine worker keeps a
// single pending action, so two calls in the same frame would lose the first; edits made
// while a request is in flight are merged per module (latest value wins) and sent when the
// catalog reports the previous one. Uses backend.setParameters (one history item) when the
// engine has it, otherwise one setParameter per field. `overrides` holds the values just sent
// so graphs show them until the catalog catches up.
QtObject {
    id: root
    property var backend
    property var overrides: ({})
    property var _queue: []          // [{ operation, instance, changes }]
    property bool _inFlight: false
    property var _sentKeys: []

    readonly property bool batchSupported: !!backend && typeof backend.setParameters === "function"

    function key(operation, instance, path) { return operation + "/" + instance + "/" + path }
    function override(operation, instance, path) { return overrides[key(operation, instance, path)] }

    function send(operation, instance, changes) {
        const next = Object.assign({}, overrides)
        for (const path in changes) next[key(operation, instance, path)] = changes[path]
        overrides = next
        const queue = _queue.slice()
        const last = queue.length ? queue[queue.length - 1] : null
        // Merge into a queued (not yet sent) request for the same module. "@enabled" is kept as
        // its own request so it reaches the engine before the values.
        if (last && last.operation === operation && last.instance === instance
                && !("@enabled" in changes) && !("@enabled" in last.changes))
            last.changes = Object.assign({}, last.changes, changes)
        else
            queue.push({ operation: operation, instance: instance, changes: Object.assign({}, changes) })
        _queue = queue
        pump()
    }
    function pump() {
        if (_inFlight || !_queue.length || !backend) return
        const queue = _queue.slice()
        const item = queue[0]
        const paths = Object.keys(item.changes)
        _sentKeys = []
        if (paths.length > 1 && batchSupported) {
            backend.setParameters(item.operation, item.instance, item.changes)
            _sentKeys = paths.map(p => key(item.operation, item.instance, p))
            queue.shift()
        } else {
            const path = paths[0]
            backend.setParameter(item.operation, item.instance, path, item.changes[path])
            _sentKeys = [key(item.operation, item.instance, path)]
            delete item.changes[path]
            if (!Object.keys(item.changes).length) queue.shift()
        }
        _queue = queue
        _inFlight = true
        settle.restart()
    }
    // The catalog changed: the request in flight has been applied.
    function acknowledge() {
        if (!_inFlight) return
        settle.stop()
        _inFlight = false
        const pending = {}
        for (const item of _queue) for (const p in item.changes) pending[key(item.operation, item.instance, p)] = true
        const next = Object.assign({}, overrides)
        let changed = false
        for (const k of _sentKeys) if (!pending[k] && k in next) { delete next[k]; changed = true }
        if (changed) overrides = next
        pump()
    }
    // An edit that does not change the catalog (same value) produces no update; do not wait forever.
    property Timer settle: Timer { interval: 400; onTriggered: root.acknowledge() }
}
