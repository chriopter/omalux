import QtQuick

// A popup or menu built the first time it is needed instead of with its row: every slider,
// choice and module heading has one, and building them all with the rows made opening a pane
// or expanding a module slow. get() creates it in `parent` (so popup coordinates stay the
// parent's) and keeps it for later uses.
//
//     OnDemand { id: menu; parent: root; Menu { … } }
//     … menu.get().popup(x, y)
QtObject {
    id: root
    default property Component component
    property Item parent: null
    property var item: null
    function get() {
        if (!root.item) {
            root.item = root.component.createObject(root.parent)
            if (!root.item) console.warn("OnDemand:", root.component.errorString())
        }
        return root.item
    }
}
