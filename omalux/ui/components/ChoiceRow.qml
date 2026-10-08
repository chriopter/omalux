import QtQuick
import QtQuick.Controls
import QtQuick.Dialogs
import QtQuick.Layouts

// A value darktable picks from a list it fills at runtime (colour and noise profiles, lensfun
// cameras and lenses, focal length, LUT files, watermark markers) or from a file dialog. The
// row shows the current value; clicking it (or Enter) opens the list with a search field. The
// list itself comes from the host: `requested(query)` asks for it, `items` holds the answer
// ([{ label, detail, section }]), `chosen(index)` reports the pick and `fileChosen(path)` a
// file from the dialog. The component never edits a parameter itself.
Item {
    id: root
    required property var theme
    required property string label
    property string valueText: ""
    property var items: []
    property int current: -1
    property int more: 0
    property string error: ""
    property bool loading: false
    property bool editable: true
    // false: only the file dialog (e.g. the overlay image).
    property bool listed: true
    // Name filters for the file dialog; empty: no "choose file" button.
    property var fileFilters: []
    property string fileTitle: "Choose a file"
    property url fileFolder
    // Typed numbers become an entry (focal length, aperture, distance).
    property string placeholder: "search"
    property font labelFont: theme.settingsFont
    property color labelColor: theme.ink
    signal requested(string query)
    signal chosen(int index)
    signal fileChosen(string path)
    signal resetRequested()
    property alias navTarget: navTarget
    property alias popup: popup

    NavTarget {
        id: navTarget
        navId: root.label
        label: root.label
        kind: "button"
        enabled: root.editable
        activateLabel: "CHOOSE"
        resettable: true
        onActivate: root.open()
        onReset: root.resetRequested()
    }
    // Same as a click (used by the smoke driver's clickItem).
    function clicked() { navTarget.claim(); root.open() }
    function open() {
        if (!root.editable)
            return
        if (!root.listed) {
            dialog.open()
            return
        }
        search.text = ""
        root.requested("")
        popup.open()
    }

    implicitHeight: 24
    RowLayout {
        anchors.fill: parent
        anchors.leftMargin: 8
        anchors.rightMargin: 8
        spacing: 8
        Text {
            Layout.fillWidth: true
            Layout.minimumWidth: 60
            text: root.label
            color: navTarget.current ? root.theme.accent : root.labelColor
            font: root.labelFont
            elide: Text.ElideRight
        }
        Text {
            id: value
            Layout.maximumWidth: root.width * .62
            text: root.valueText || "none"
            color: root.editable ? root.theme.ink : root.theme.muted
            font: root.theme.textFont
            elide: Text.ElideMiddle
            HoverHandler { id: hover }
            ToolTip.visible: hover.hovered && value.truncated
            ToolTip.text: root.valueText
            ToolTip.delay: 400
        }
        Text {
            text: root.listed ? "▾" : "…"
            color: root.theme.muted
            font: root.theme.textFont
        }
    }
    MouseArea {
        anchors.fill: parent
        acceptedButtons: Qt.LeftButton
        enabled: root.editable
        cursorShape: Qt.PointingHandCursor
        onClicked: { navTarget.claim(); root.open() }
    }

    Popup {
        id: popup
        objectName: "choice-popup-" + root.label
        // Below the row, or above it when the window has no room left underneath.
        readonly property real below: root.Window.height - root.mapToItem(null, 0, root.height).y
        y: below >= height + 8 ? root.height : -height
        x: 0
        width: Math.max(root.width, 260)
        height: Math.min(360, column.implicitHeight + 16)
        margins: 6
        padding: 8
        focus: true
        closePolicy: Popup.CloseOnEscape | Popup.CloseOnPressOutside
        onOpened: { list.currentIndex = root.current; search.forceActiveFocus() }
        background: Rectangle {
            color: root.theme.background
            border.color: root.theme.line
            radius: 6
        }
        contentItem: ColumnLayout {
            id: column
            spacing: 6
            TextField {
                id: search
                objectName: "choice-search"
                Layout.fillWidth: true
                placeholderText: root.placeholder
                font: root.theme.textFont
                color: root.theme.ink
                background: Rectangle { color: root.theme.well; radius: 4; border.color: root.theme.line }
                onTextEdited: searchDelay.restart()
                Keys.onDownPressed: list.currentIndex = Math.min(list.count - 1, list.currentIndex + 1)
                Keys.onUpPressed: list.currentIndex = Math.max(0, list.currentIndex - 1)
                Keys.onReturnPressed: root.pick(list.currentIndex >= 0 ? list.currentIndex : 0)
                Keys.onEnterPressed: root.pick(list.currentIndex >= 0 ? list.currentIndex : 0)
                Keys.onEscapePressed: popup.close()
            }
            Timer { id: searchDelay; interval: 180; onTriggered: root.requested(search.text) }
            Text {
                visible: root.error !== "" || (!root.loading && root.items.length === 0)
                Layout.fillWidth: true
                text: root.error !== "" ? root.error : "nothing found"
                color: root.theme.muted
                font: root.theme.textFont
                wrapMode: Text.Wrap
            }
            ListView {
                id: list
                objectName: "choice-list"
                Layout.fillWidth: true
                Layout.preferredHeight: Math.min(contentHeight, 260)
                clip: true
                model: root.items
                currentIndex: root.current
                boundsBehavior: Flickable.StopAtBounds
                ScrollBar.vertical: ScrollBar { policy: list.contentHeight > list.height ? ScrollBar.AlwaysOn : ScrollBar.AsNeeded }
                delegate: Column {
                    id: entry
                    required property var modelData
                    required property int index
                    width: list.width
                    readonly property string sectionText: modelData.section || ""
                    readonly property bool sectionStarts: sectionText !== ""
                        && (index === 0 || (root.items[index - 1].section || "") !== sectionText)
                    Text {
                        visible: entry.sectionStarts
                        width: list.width
                        text: entry.sectionText
                        topPadding: 4
                        bottomPadding: 2
                        color: root.theme.muted
                        font: root.theme.textFont
                        elide: Text.ElideRight
                    }
                    ItemDelegate {
                        id: button
                        width: list.width
                        height: 24
                        padding: 0
                        highlighted: entry.ListView.isCurrentItem
                        onClicked: root.pick(entry.index)
                        contentItem: RowLayout {
                            spacing: 8
                            Text {
                                Layout.fillWidth: true
                                leftPadding: 6
                                text: entry.modelData.label
                                color: entry.index === root.current ? root.theme.accent : root.theme.ink
                                font: root.theme.textFont
                                elide: Text.ElideRight
                            }
                            Text {
                                visible: !!entry.modelData.detail
                                Layout.maximumWidth: list.width * .45
                                rightPadding: 14
                                text: entry.modelData.detail || ""
                                color: root.theme.muted
                                font: root.theme.textFont
                                elide: Text.ElideLeft
                            }
                        }
                        background: Rectangle {
                            color: button.highlighted || button.hovered ? root.theme.well : "transparent"
                            radius: 3
                        }
                    }
                }
            }
            Text {
                visible: root.more > 0
                text: root.more + " more — refine the search"
                color: root.theme.muted
                font: root.theme.textFont
            }
            Button {
                visible: root.fileFilters.length > 0
                objectName: "choice-browse"
                text: "choose file…"
                flat: true
                font: root.theme.textFont
                onClicked: { popup.close(); dialog.open() }
            }
        }
    }
    function pick(index) {
        if (index < 0 || index >= root.items.length)
            return
        popup.close()
        root.chosen(index)
    }
    FileDialog {
        id: dialog
        title: root.fileTitle
        nameFilters: root.fileFilters
        currentFolder: root.fileFolder
        onAccepted: {
            const url = selectedFile.toString()
            root.fileChosen(decodeURIComponent(url.replace(/^file:\/\//, "")))
        }
    }
}
