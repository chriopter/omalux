import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

// Pages inside one darktable module (its notebook tabs, e.g. color balance rgb "master",
// "4 ways", "masks"). Same recessed strip as the sidebar pane tabs, with the current page
// raised inside it; labels are darktable's own. Left/Right switch pages when focused.
Rectangle {
    id: root
    required property var theme
    property var tabs: []           // darktable tab labels
    property int currentIndex: 0
    property bool editable: true
    signal tabSelected(int index)
    // Keyboard: ←/→ switch pages from the sidebar selection (see NavTarget).
    property alias navTarget: navTarget
    NavTarget {
        id: navTarget
        navId: "tabs"
        label: "page " + (root.tabs[root.currentIndex] || "")
        kind: "choice"
        resettable: false
        activateLabel: ""
        enabled: root.editable
        onAdjust: steps => {
            const next = Math.max(0, Math.min(root.tabs.length - 1, root.currentIndex + Math.sign(steps)))
            if (next !== root.currentIndex) root.tabSelected(next)
        }
    }

    implicitWidth: 260
    implicitHeight: 26
    radius: 6
    color: theme.well
    border.color: theme.line
    border.width: 1
    activeFocusOnTab: true
    Keys.onLeftPressed: if (currentIndex > 0) tabSelected(currentIndex - 1)
    Keys.onRightPressed: if (currentIndex < tabs.length - 1) tabSelected(currentIndex + 1)
    Accessible.role: Accessible.PageTabList

    // Each tab first gets its label's width up to an equal share; what is left goes to the
    // tabs still short of their label, so short labels (CAT, R, look) keep their width and long
    // ones (colorfulness) give way first.
    FontMetrics { id: metrics; font: root.theme.textFont }
    readonly property var widths: {
        const n = tabs.length
        const avail = Math.max(0, width - 4 - Math.max(0, n - 1) * 2)
        if (!n) return []
        const natural = tabs.map(t => metrics.advanceWidth(t) + 10)
        const total = natural.reduce((x, y) => x + y, 0)
        if (total <= avail) return natural.map(w => w + (avail - total) / n)
        const base = natural.map(w => Math.min(w, avail / n))
        const left = avail - base.reduce((x, y) => x + y, 0)
        const deficit = natural.map((w, i) => w - base[i])
        const sum = deficit.reduce((x, y) => x + y, 0)
        return base.map((w, i) => w + (sum > 0 ? left * deficit[i] / sum : 0))
    }

    RowLayout {
        anchors.fill: parent
        anchors.margins: 2
        spacing: 2
        Repeater {
            model: root.tabs
            Button {
                id: tab
                required property string modelData
                required property int index
                readonly property bool current: root.currentIndex === index
                Layout.fillHeight: true
                Layout.preferredWidth: root.widths[index] || 0
                padding: 0
                hoverEnabled: true
                focusPolicy: Qt.NoFocus
                enabled: root.editable
                onClicked: root.tabSelected(index)
                Accessible.role: Accessible.PageTab
                Accessible.name: modelData
                Accessible.selected: current
                ToolTip.visible: hovered && label.truncated
                ToolTip.delay: 500
                ToolTip.text: modelData
                contentItem: Text {
                    id: label
                    text: tab.modelData
                    color: tab.current ? root.theme.accent : tab.hovered ? root.theme.ink : root.theme.muted
                    font: root.theme.textFont
                    horizontalAlignment: Text.AlignHCenter
                    verticalAlignment: Text.AlignVCenter
                    elide: Text.ElideRight
                    leftPadding: 3; rightPadding: 3
                }
                background: Rectangle {
                    radius: 4
                    color: tab.current || tab.pressed ? root.theme.active : tab.hovered ? root.theme.hover : "transparent"
                    border.width: tab.current || (root.activeFocus && tab.current) ? 1 : 0
                    border.color: root.activeFocus || navTarget.current ? root.theme.accent : root.theme.line
                }
            }
        }
    }
}
