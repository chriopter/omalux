import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

// The image toolbar, built like the sidebar's tab strips: recessed strips holding tiles with an
// icon and a calm label. File (Open with its arrow for the example photograph, Export), view
// (Before) and zoom (out, the level, in). Shortcuts are in the tooltips, not on the tiles.
Rectangle {
    id: root
    required property var theme
    required property url logoSource
    required property string filename
    // Where the toolbar's icons are (assets/icons/).
    property url iconsRoot: Qt.resolvedUrl("../../../assets/icons/")
    property real zoom: 1
    property real minimumZoom: 1
    property real maximumZoom: 16
    // Nothing to export or zoom before the first preview.
    property bool photoReady: true
    // An open, export or style is running: nothing else can start until it ends.
    property bool busy: false
    // Before/after: the view shows the photograph as opened.
    property bool comparing: false
    property bool compareAvailable: true
    // The example photograph shipped with the application can be opened.
    property bool exampleAvailable: true
    // Labels give way to icons when the window is too narrow for both.
    readonly property bool compact: width < 720
    readonly property bool fitted: Math.abs(zoom - 1) < .001
    signal compareRequested()
    signal openRequested()
    signal exampleRequested()
    signal saveRequested()
    signal fitRequested()
    signal zoomRequested(real factor)
    // The menu of the Open arrow, also from the keyboard (Shift+O).
    function openMenu() { if (openMore.enabled) openChoices.popup(openGroup, 0, openGroup.height + 2) }
    readonly property alias menuOpen: openChoices.visible
    implicitHeight: 48
    color: root.theme.background

    component Strip: Rectangle {
        default property alias tiles: stripRow.data
        implicitWidth: stripRow.implicitWidth + 6
        implicitHeight: 36
        radius: 7
        color: root.theme.well
        border.color: root.theme.line
        border.width: 1
        Row {
            id: stripRow
            x: 3; y: 3
            spacing: 3
        }
    }
    component Divider: Item {
        width: 1; height: 30
        Rectangle { anchors.centerIn: parent; width: 1; height: 16; color: root.theme.line }
    }

    RowLayout {
        anchors.fill: parent
        anchors.leftMargin: 18
        anchors.rightMargin: 18
        spacing: 8
        Image {
            id: logo
            source: root.logoSource
            Layout.preferredWidth: 92
            Layout.preferredHeight: 26
            fillMode: Image.PreserveAspectFit
        }
        Item {
            Layout.fillWidth: true
        }
        Strip {
            // Open is two parts of one control: the tile opens the file dialog, its arrow a
            // menu with the example photograph.
            Row {
                id: openGroup
                objectName: "toolbar-open-group"
                spacing: 0
                ToolbarButton {
                    objectName: "toolbar-open"
                    theme: root.theme; compact: root.compact
                    iconSource: root.iconsRoot + "open.svg"
                    text: "Open"; tip: "Open a photograph"; keys: "key O"
                    enabled: !root.busy
                    onClicked: root.openRequested()
                }
                Divider {}
                ToolbarButton {
                    id: openMore
                    objectName: "toolbar-open-more"
                    theme: root.theme
                    width: 22
                    iconSource: root.iconsRoot + "chevron-down.svg"
                    icon.width: 12; icon.height: 12
                    tip: "Example photograph"; keys: "Shift+O"
                    enabled: !root.busy && root.exampleAvailable
                    active: openChoices.visible
                    onClicked: root.openMenu()
                    Accessible.name: "More ways to open"
                }
            }
            ToolbarButton {
                objectName: "toolbar-export"
                theme: root.theme; compact: root.compact
                iconSource: root.iconsRoot + "export.svg"
                text: "Export"; tip: "Export as JPEG or PNG"; keys: "Ctrl+S"
                enabled: root.photoReady && !root.busy
                onClicked: root.saveRequested()
            }
        }
        Strip {
            ToolbarButton {
                objectName: "toolbar-before"
                theme: root.theme; compact: root.compact
                iconSource: root.iconsRoot + "compare.svg"
                text: "Before"
                tip: root.comparing ? "Back to the edit" : "Show the photograph as it was opened"; keys: "key B"
                enabled: root.photoReady && root.compareAvailable && !root.busy
                active: root.comparing
                onClicked: root.compareRequested()
                Accessible.name: "Before and after"
                Accessible.checkable: true
                Accessible.checked: root.comparing
            }
        }
        // Zoom: step out, the level (a click fits the photograph), step in.
        Strip {
            ToolbarButton {
                objectName: "toolbar-zoom-out"
                theme: root.theme; glyph: true; text: "−"; tip: "Zoom out"; keys: "key −"
                enabled: root.photoReady && root.zoom > root.minimumZoom + .001
                onClicked: root.zoomRequested(.8)
            }
            ToolbarButton {
                objectName: "toolbar-fit"
                theme: root.theme
                width: 56
                text: root.fitted ? "Fit" : root.zoom.toFixed(1) + "×"
                tip: root.fitted ? "The photograph fits the window" : "Fit the photograph to the window"; keys: "key 0 (zero)"
                enabled: root.photoReady
                active: !root.fitted
                onClicked: root.fitRequested()
                Accessible.name: "Fit photograph"
            }
            ToolbarButton {
                objectName: "toolbar-zoom-in"
                theme: root.theme; glyph: true; text: "+"; tip: "Zoom in"; keys: "key +"
                enabled: root.photoReady && root.zoom < root.maximumZoom - .001
                onClicked: root.zoomRequested(1.25)
            }
        }
        Item {
            Layout.fillWidth: true
        }
        Text {
            objectName: "toolbar-filename"
            text: root.filename
            color: root.theme.muted
            font: root.theme.textFont
            elide: Text.ElideMiddle
            Layout.maximumWidth: 170
            // The whole name when it does not fit.
            HoverHandler { id: nameHover }
            ToolTip.visible: nameHover.hovered && truncated
            ToolTip.delay: 500
            ToolTip.text: root.filename
        }
    }
    Menu {
        id: openChoices
        objectName: "toolbar-open-menu"
        MenuItem {
            objectName: "toolbar-open-example"
            text: "Example photograph"
            icon.source: root.iconsRoot + "photo.svg"
            enabled: root.exampleAvailable
            onTriggered: root.exampleRequested()
        }
        MenuItem {
            objectName: "toolbar-open-file"
            text: "Open a photograph…"
            icon.source: root.iconsRoot + "open.svg"
            onTriggered: root.openRequested()
        }
    }
    Rectangle {
        anchors.bottom: parent.bottom
        width: parent.width
        height: 1
        color: root.theme.line
    }
}
