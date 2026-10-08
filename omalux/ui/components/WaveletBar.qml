import QtQuick
import QtQuick.Layouts

// retouch's wavelet decompose bar (retouch.c rt_wdbar_draw 1387, button press 1242, scroll 1294,
// motion 1324, rt_num_scales_update 1150, rt_curr_scale_update 1172, rt_merge_from_scale_update
// 1209) and its cut/paste of shapes between scales (rt_copypaste_scale_callback 1624,
// rt_paste_forms_from_scale 606). 17 boxes: the original image (0), scales 1…15 and the residual;
// the bottom margin sets the number of scales, the top margin where merging starts, a box the
// current scale (new shapes go to it); the wheel changes the one under the pointer. A line under
// a box marks a scale with shapes. Labels as darktable's ("scales:", "current:", "merge from:").
// Edits leave through edited({num_scales | merge_from_scale | curr_scale | "rt_forms[i].scale"}).
Column {
    id: root
    required property var theme
    property int numScales: 0
    property int currScale: 0
    property int mergeFrom: 0
    property var formScales: []        // scale of every used shape slot (formid valid)
    property var formSlots: []         // their indices in rt_forms
    property bool editable: true
    property int copiedScale: -1
    property alias navTarget: nav
    signal edited(var changes)
    spacing: 4
    readonly property int maxScales: 15
    readonly property int boxes: maxScales + 2
    function hasShapes(i) { return root.formScales.indexOf(i) >= 0 }
    function setNum(n) { n = Math.max(0, Math.min(maxScales, n)); if (n === numScales) return
                         const c = { num_scales: n }; if (n < mergeFrom) c.merge_from_scale = n; root.edited(c) }
    function setCurr(n) { n = Math.max(0, Math.min(maxScales + 1, n)); if (n !== currScale) root.edited({ curr_scale: n }) }
    function setMerge(n) { n = Math.max(0, Math.min(numScales, n)); if (n !== mergeFrom) root.edited({ merge_from_scale: n }) }
    function paste() {
        if (copiedScale < 0 || copiedScale === currScale) { copiedScale = -1; return }
        const c = {}
        for (let i = 0; i < formSlots.length; ++i) if (formScales[i] === copiedScale) c["rt_forms[" + formSlots[i] + "].scale"] = currScale
        copiedScale = -1
        if (Object.keys(c).length) root.edited(c)
    }
    NavTarget {
        id: nav
        navId: "wavelet-bar"
        label: "wavelet decompose"
        kind: "graph"
        focusItem: bar
        enabled: root.editable
        activateLabel: "EDIT SCALES"
    }
    RowLayout {
        width: root.width
        Text { text: "scales: " + root.numScales; color: root.theme.muted; font: root.theme.textFont }
        Text { text: "current: " + root.currScale; color: root.theme.muted; font: root.theme.textFont }
        Text { text: "merge from: " + root.mergeFrom; color: root.theme.muted; font: root.theme.textFont }
    }
    Item {
        id: bar
        objectName: "wavelet-bar"
        width: root.width; height: 40
        readonly property real inset: Math.round(0.2 * height)
        readonly property real sh: 3 + inset
        readonly property real boxW: (width - 2 * inset) / root.boxes
        property int hover: -1
        property string margin: ""        // "top" / "bottom" while the pointer is in a margin
        property string dragging: ""
        // ←/→ current scale, ↑/↓ number of scales, Shift+↑/↓ merge from
        Keys.onPressed: event => {
            if (event.key === Qt.Key_Left || event.key === Qt.Key_Right) root.setCurr(root.currScale + (event.key === Qt.Key_Right ? 1 : -1))
            else if (event.key === Qt.Key_Up || event.key === Qt.Key_Down) {
                const d = event.key === Qt.Key_Up ? 1 : -1
                if (event.modifiers & Qt.ShiftModifier) root.setMerge(root.mergeFrom + d); else root.setNum(root.numScales + d)
            } else return
            event.accepted = true
        }
        function zoneAt(x, y) {
            const mx = Math.max(0, Math.min(width - 2 * inset - 1, x - inset))
            return { box: Math.floor(mx / boxW), margin: y <= sh ? "top" : y >= height - sh ? "bottom" : "" }
        }
        Canvas {
            id: canvas
            anchors.fill: parent
            Connections { target: root; function onNumScalesChanged() { canvas.requestPaint() } function onCurrScaleChanged() { canvas.requestPaint() }
                          function onMergeFromChanged() { canvas.requestPaint() } function onFormScalesChanged() { canvas.requestPaint() } }
            Connections { target: bar; function onHoverChanged() { canvas.requestPaint() } }
            onWidthChanged: requestPaint()
            onPaint: {
                const c = getContext("2d"), inset = bar.inset, sh = bar.sh, bw = bar.boxW, bh = height - 2 * sh
                c.fillStyle = Qt.rgba(.15, .15, .15, 1); c.fillRect(0, 0, width, height)
                for (let i = 0; i < root.boxes; ++i) {
                    let v = .15
                    if (i === 0) v = .1
                    else if (i === root.numScales + 1) v = .8
                    else if (i >= root.mergeFrom && i <= root.numScales && root.mergeFrom > 0) v = .5
                    else if (i <= root.numScales) v = .35
                    c.fillStyle = Qt.rgba(v, v, v, 1); c.fillRect(bw * i + inset, sh, bw, bh)
                    if (root.hasShapes(i)) { c.fillStyle = Qt.rgba(.75, .5, 0, 1); c.fillRect(bw * i + inset + .5, height - sh, bw - 1, 2) }
                    c.strokeStyle = Qt.rgba(.066, .066, .066, 1); c.lineWidth = 1; c.strokeRect(bw * i + inset, sh, bw, bh)
                }
                const dot = root.currScale >= root.mergeFrom && root.currScale <= root.numScales && root.mergeFrom > 0 ? .35 : .5
                c.fillStyle = Qt.rgba(dot, dot, dot, 1)
                c.beginPath(); c.arc(bw * (.5 + root.currScale) + inset, .5 * bh + sh, .5 * inset, 0, 2 * Math.PI); c.fill()
                // the two arrows: merge from (top) and number of scales (bottom)
                c.fillStyle = root.theme.ink
                const top = bw * (.5 + root.mergeFrom) + inset, bottom = bw * (.5 + root.numScales) + inset
                c.beginPath(); c.moveTo(top - inset, 0); c.lineTo(top + inset, 0); c.lineTo(top, sh); c.closePath(); c.fill()
                c.beginPath(); c.moveTo(bottom - inset, height); c.lineTo(bottom + inset, height); c.lineTo(bottom, height - sh); c.closePath(); c.fill()
                if (bar.hover >= 0) { c.strokeStyle = root.theme.accent; c.strokeRect(bw * bar.hover + inset + .5, sh + .5, bw - 1, bh - 1) }
                if (nav.current || bar.activeFocus) { c.strokeStyle = root.theme.accent; c.strokeRect(.5, .5, width - 1, height - 1) }
            }
        }
        MouseArea {
            anchors.fill: parent
            enabled: root.editable
            hoverEnabled: true
            preventStealing: true
            onPositionChanged: mouse => {
                const z = bar.zoneAt(mouse.x, mouse.y)
                bar.hover = z.margin ? -1 : z.box
                if (bar.dragging === "bottom") root.setNum(z.box)
                else if (bar.dragging === "top") root.setMerge(z.box)
            }
            onExited: bar.hover = -1
            onPressed: mouse => {
                nav.claim()
                const z = bar.zoneAt(mouse.x, mouse.y)
                if (z.margin === "bottom") { if (Math.abs(z.box - root.numScales) <= 0) bar.dragging = "bottom"; else root.setNum(z.box) }
                else if (z.margin === "top") { if (z.box === root.mergeFrom) bar.dragging = "top"; else root.setMerge(z.box) }
                else root.setCurr(z.box)
            }
            onReleased: bar.dragging = ""
            onWheel: wheel => {
                const d = wheel.angleDelta.y > 0 ? 1 : wheel.angleDelta.y < 0 ? -1 : 0   // up = −delta_y
                if (!d) return
                const z = bar.zoneAt(wheel.x, wheel.y)
                if (z.margin === "bottom") root.setNum(root.numScales + d)
                else if (z.margin === "top") root.setMerge(root.mergeFrom + d)
                else root.setCurr(root.currScale + d)
            }
        }
    }
    // darktable's cut and paste of the shapes of a scale
    ModuleToolButtons {
        width: root.width
        theme: root.theme
        editable: root.editable
        navPrefix: "wavelet-tools"
        navGroup: nav.group
        entries: [{ label: "cut shapes from current scale", kind: "button", active: root.copiedScale >= 0 },
                  { label: "paste cut shapes to current scale", kind: "button", enabled: root.copiedScale >= 0 }]
        onTriggered: (index, choice) => {
            if (index === 0) root.copiedScale = root.copiedScale >= 0 ? -1 : root.currScale
            else root.paste()
        }
    }
}
