import QtQuick
import QtQuick.Controls

// One look in the Styles grid: its thumbnail and name. A click applies it, hovering (or the
// keyboard selection) previews it on the photograph, the star marks a favourite (shown on
// favourites, and on the hovered tile to add one). The applied look carries an accent frame.
// The right-click menu holds the rest: favourite, settings, export, delete. Thumbnails load
// asynchronously at their shown size, so a long list stays light.
Item {
    id: root
    required property var theme
    required property var style
    required property bool photoReady
    required property bool busy
    required property string appliedStyle
    required property string applyingStyle
    required property bool favourite
    property bool detailsOpen: false
    // Distinguishes the copy of a favourite shown above its family (object names, keyboard ids).
    property string keyPrefix: ""
    // The name without its family words ("Contrast" under Late summer); the full name in tooltips.
    property string shortName: ""
    signal previewRequested(bool active)
    signal applyRequested()
    signal favouriteToggled()
    signal detailsToggleRequested()
    signal exportRequested()
    signal deleteRequested()
    property alias navTarget: navTarget

    readonly property bool applied: appliedStyle === style.name
    readonly property bool available: !busy && photoReady && style.error === ""
    readonly property bool previewHovered: (area.containsMouse || navTarget.current) && available && visible
    implicitHeight: thumb.height + nameText.implicitHeight + 6

    NavTarget {
        id: navTarget
        navId: root.keyPrefix + root.style.id
        label: root.style.name
        kind: "style"
        active: root.applied
        adjustLabel: "SETTINGS"
        activateLabel: root.available ? "APPLY STYLE" : ""
        extraHints: [["E", root.favourite ? "UNFAVOURITE" : "FAVOURITE"]]
        onActivate: if (root.available) { hoverDelay.stop(); root.previewRequested(false); root.applyRequested() }
        onAdjust: steps => { if ((steps > 0) !== root.detailsOpen) root.detailsToggleRequested() }
        onToggleGroup: root.favouriteToggled()
    }
    onPreviewHoveredChanged: {
        if (previewHovered) hoverDelay.restart()
        else { hoverDelay.stop(); root.previewRequested(false) }
    }
    Component.onDestruction: root.previewRequested(false)
    Timer { id: hoverDelay; interval: 150; onTriggered: if (root.previewHovered) root.previewRequested(true) }

    Rectangle {
        id: thumb
        objectName: root.keyPrefix + "style-apply-" + root.style.id
        width: parent.width
        height: Math.round(width * 2 / 3)
        radius: 3
        color: "#11111b"
        clip: true
        Accessible.role: Accessible.Button
        Accessible.name: "Apply " + root.style.name
        Image {
            id: preview
            objectName: "stylePreview-" + root.style.id
            anchors.fill: parent
            anchors.margins: 1
            source: root.style.previewUrl
            fillMode: Image.PreserveAspectCrop
            asynchronous: true
            smooth: true
            sourceSize.width: 2 * width
            sourceSize.height: 2 * height
            opacity: root.style.error === "" ? 1 : .35
        }
        Text {
            anchors.centerIn: parent
            visible: preview.status !== Image.Ready
            text: root.style.error !== "" ? "unavailable" : "no preview"
            font: root.theme.textFont
            color: root.theme.muted
        }
        // The applied look, the keyboard selection and hover frame the picture.
        Rectangle {
            anchors.fill: parent
            color: "transparent"
            radius: 3
            border.width: root.applied || navTarget.current ? 2 : area.containsMouse ? 1 : 0
            border.color: root.applied ? root.theme.accent : navTarget.current ? root.theme.accent : root.theme.ink
            opacity: root.applied || navTarget.current ? 1 : .6
        }
        Text {
            visible: root.busy && root.applyingStyle === root.style.id
            anchors.centerIn: parent
            text: "applying…"
            color: root.theme.ink; font: root.theme.textFont
            style: Text.Outline; styleColor: "#11111b"
        }
        MouseArea {
            id: area
            anchors.fill: parent
            hoverEnabled: true
            acceptedButtons: Qt.LeftButton | Qt.RightButton
            cursorShape: root.available ? Qt.PointingHandCursor : Qt.ArrowCursor
            onClicked: mouse => {
                navTarget.claim()
                if (mouse.button === Qt.RightButton) { menu.get().popup(mouse.x, mouse.y); return }
                if (!root.available) return
                hoverDelay.stop(); root.previewRequested(false); root.applyRequested()
            }
        }
        ToolTip.visible: area.containsMouse && (root.style.error !== "" || nameText.truncated || root.shortName !== "")
        ToolTip.delay: root.style.error !== "" ? 0 : 600
        ToolTip.text: root.style.error || root.style.name
        // Favourite star: always on favourites, on hover for the others.
        ToolButton {
            id: star
            objectName: root.keyPrefix + "style-favourite-" + root.style.id
            visible: root.favourite || area.containsMouse || hovered
            anchors.right: parent.right; anchors.top: parent.top
            width: 22; height: 22; padding: 0
            hoverEnabled: true
            onClicked: { navTarget.claim(); root.favouriteToggled() }
            Accessible.name: (root.favourite ? "Remove " : "Add ") + root.style.name + (root.favourite ? " from favourites" : " to favourites")
            contentItem: Text {
                text: root.favourite ? "★" : "☆"
                color: root.favourite ? "#f9e2af" : star.hovered ? "#f9e2af" : root.theme.ink
                font.pixelSize: 14
                horizontalAlignment: Text.AlignHCenter; verticalAlignment: Text.AlignVCenter
                style: Text.Outline; styleColor: "#11111b"
            }
            background: Item {}
        }
    }
    Text {
        id: nameText
        anchors.top: thumb.bottom; anchors.topMargin: 3
        width: parent.width
        text: root.shortName || root.style.name
        color: root.applied || navTarget.current ? root.theme.accent : area.containsMouse ? root.theme.ink : "#a6adc8"
        font.family: root.theme.textFont.family
        font.pixelSize: root.theme.textFont.pixelSize - 1
        font.bold: root.applied
        elide: Text.ElideRight
    }
    OnDemand {
        id: menu
        parent: root
        Menu {
            MenuItem { text: root.favourite ? "Remove from favourites" : "Add to favourites"; onTriggered: root.favouriteToggled() }
            MenuItem { text: root.detailsOpen ? "Hide settings" : "Show settings"; onTriggered: root.detailsToggleRequested() }
            MenuSeparator {}
            MenuItem { text: "Export bundle…"; onTriggered: root.exportRequested() }
            MenuItem { text: "Delete…"; enabled: root.style.id.startsWith("my-styles/"); onTriggered: root.deleteRequested() }
        }
    }
}
