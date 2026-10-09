import QtQuick
import QtQuick.Controls
import QtQuick.Controls.impl

// A tile of the image toolbar, drawn like the sidebar's tabs: an icon and a calm label on the
// recessed strip, the raised fill on hover. Hover never takes the accent (that marks a switch
// that is on, like the chosen tab). The shortcut is not printed on the tile: resting the
// pointer shows `tip` followed by `keys` ("key O", "key 0 (zero)").
AbstractButton {
    id: root
    required property var theme
    property url iconSource: ""
    property string tip: ""
    property string keys: ""
    // A switch that is on (before/after).
    property bool active: false
    // Icon only: a narrow window, or a tile whose icon says it all.
    property bool compact: false
    // A larger glyph instead of an icon (− and +).
    property bool glyph: false
    readonly property bool iconOnly: iconSource != "" && (compact || text === "")
    ToolTip.visible: hovered && !pressed && tip !== ""
    ToolTip.delay: 500
    ToolTip.text: tip + (keys !== "" ? "  ·  " + keys : "")
    // The keys stay with the editor: a click must not leave the focus on the toolbar.
    focusPolicy: Qt.NoFocus
    implicitHeight: 30
    implicitWidth: Math.max(30, implicitContentWidth + leftPadding + rightPadding)
    padding: 0
    leftPadding: iconOnly || glyph ? 0 : 10
    rightPadding: iconOnly || glyph ? 0 : 10
    hoverEnabled: true
    spacing: 6
    icon.source: iconSource
    icon.width: 16; icon.height: 16
    font.family: theme.textFont.family
    font.pixelSize: glyph ? theme.textFont.pixelSize + 3 : theme.textFont.pixelSize
    font.weight: Font.Medium
    opacity: enabled ? 1 : .4
    Accessible.name: tip !== "" ? tip : text
    contentItem: IconLabel {
        spacing: root.spacing
        display: root.iconOnly ? AbstractButton.IconOnly : AbstractButton.TextBesideIcon
        icon: root.icon
        text: root.text
        font: root.font
        color: root.active ? root.theme.accent : root.theme.ink
        defaultIconColor: root.active ? root.theme.accent : root.hovered && root.enabled ? root.theme.ink : root.theme.muted
    }
    background: Rectangle {
        radius: 5
        color: !root.enabled ? "transparent"
               : root.pressed || root.active ? root.theme.active : root.hovered ? root.theme.hover : "transparent"
        border.width: root.active ? 1 : 0
        border.color: root.theme.line
    }
}
