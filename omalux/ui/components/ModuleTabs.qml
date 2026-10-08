import QtQuick
import QtQuick.Controls

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
    implicitHeight: layout.rows * 22 + (layout.rows - 1) * 2 + 4
    radius: 6
    color: theme.well
    border.color: theme.line
    border.width: 1
    activeFocusOnTab: true
    Keys.onLeftPressed: if (currentIndex > 0) tabSelected(currentIndex - 1)
    Keys.onRightPressed: if (currentIndex < tabs.length - 1) tabSelected(currentIndex + 1)
    Accessible.role: Accessible.PageTabList

    // Every label stays readable: tabs keep one row while their labels fit, otherwise they
    // wrap into as few rows as needed with the tabs spread evenly (5 tabs: 3 + 2), each row
    // filling the width. Only a single label wider than the whole strip still elides, with the
    // full label as tooltip.
    FontMetrics { id: metrics; font: root.theme.textFont }
    readonly property var layout: {
        const n = tabs.length
        const avail = Math.max(0, width - 4)
        if (!n) return { rows: 1, boxes: [] }
        const natural = tabs.map(t => Math.ceil(metrics.advanceWidth(t)) + 14)
        let per = n
        for (let k = 1; k <= n; ++k) {
            per = Math.ceil(n / k)
            let fits = true
            for (let i = 0; i < n && fits; i += per) {
                const chunk = natural.slice(i, i + per)
                fits = chunk.reduce((x, y) => x + y, 0) + (chunk.length - 1) * 2 <= avail
            }
            if (fits) break
        }
        const boxes = []
        let row = 0
        for (let i = 0; i < n; i += per, ++row) {
            const chunk = natural.slice(i, i + per)
            const room = avail - (chunk.length - 1) * 2
            const total = chunk.reduce((x, y) => x + y, 0)
            let x = 2
            for (let j = 0; j < chunk.length; ++j) {
                const w = total <= room ? chunk[j] + (room - total) / chunk.length : room / chunk.length
                boxes.push({ x: x, y: 2 + row * 24, w: w })
                x += w + 2
            }
        }
        return { rows: row, boxes: boxes }
    }

    Item {
        anchors.fill: parent
        Repeater {
            model: root.tabs
            Button {
                id: tab
                required property string modelData
                required property int index
                readonly property bool current: root.currentIndex === index
                objectName: "module-tab-" + modelData
                readonly property var box: root.layout.boxes[index] || { x: 0, y: 0, w: 0 }
                x: box.x
                y: box.y
                width: box.w
                height: 22
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
