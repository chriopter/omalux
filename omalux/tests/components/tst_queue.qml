import QtQuick
import QtTest
import "../../ui/components"

// The parameter queue keeps a row's fresh value until the catalog shows it:
//   QT_QPA_PLATFORM=offscreen /usr/lib/qt6/bin/qmltestrunner -input omalux/tests/components/tst_queue.qml
Item {
    id: top
    property var sent: []
    property var catalog: ({ a: 0, b: 0 })
    QtObject {
        id: backend
        function setParameter(operation, instance, path, value) { top.sent.push([path, value]) }
    }
    ParameterQueue {
        id: queue
        backend: backend
        confirmed: (key, value) => top.catalog[key.split("/")[2]] === value
    }

    TestCase {
        name: "queue"
        function test_older_update_keeps_fresh_value() {
            queue.send("demo", 0, { a: 1 })              // slider A
            queue.send("demo", 0, { b: 2 })              // slider B, 100 ms later
            compare(top.sent.length, 1, "one request at a time")
            top.catalog = { a: 1, b: 0 }                 // the update for A arrives
            queue.acknowledge()
            compare(queue.override("demo", 0, "a"), undefined, "A is confirmed")
            compare(queue.override("demo", 0, "b"), 2, "B keeps the value just set")
            compare(top.sent[1], ["b", 2], "B goes out next")
            queue.acknowledge()                          // another update still without B
            compare(queue.override("demo", 0, "b"), 2, "an older update does not snap B back")
            top.catalog = { a: 1, b: 2 }
            queue.acknowledge()
            tryCompare(queue, "overrides", ({}), 1000)
        }
        function test_unconfirmed_value_expires() {
            top.catalog = { a: 0, b: 0 }
            queue.send("demo", 0, { a: 5 })              // the engine clamps it: never shown
            queue.acknowledge()
            compare(queue.override("demo", 0, "a"), 5)
            tryCompare(queue, "overrides", ({}), 4500)
        }
    }
}
