import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

// Every Omalux camera preset, grouped by camera maker, to put on the photograph by hand:
//
//   ▾ Fujifilm ………………………………… 1 of 3
//     ● X-T10 · exposure …………… applied
//       rgb levels · matches this camera
//     ○ embedded lens correction
//       lens correction · this photo carries no embedded lens data
//
// One maker is open at a time; the photograph's maker is opened when it is known. A click on
// a row puts the preset on, a click on an applied one takes it off again. A preset that
// cannot work on this photograph is greyed and says why. `presets` is the engine's list
// (Editor.cameraPresets); the choice leaves as presetRequested(name, on).
Column {
    id: root
    required property var theme
    property var presets: []
    // false while the engine is busy or no photograph is open: rows do not act.
    property bool ready: true
    signal presetRequested(string name, bool on)

    spacing: 2

    // [{ maker, presets, applied, mine, general }] in the engine's order (by maker pattern).
    readonly property var makers: {
        const groups = []
        for (const preset of root.presets || []) {
            let group = groups.find(g => g.maker === preset.maker)
            if (!group) groups.push(group = { maker: preset.maker, presets: [], applied: 0, mine: false, general: !!preset.general })
            group.presets.push(preset)
            if (preset.applied) ++group.applied
            if (preset.makerMatches) group.mine = true
        }
        // The photograph's maker first, then the presets for every camera, then the rest.
        function rank(g) { return g.mine ? 0 : g.general ? 1 : 2 }
        return groups.map((g, i) => ({ g: g, i: i })).sort((a, b) => rank(a.g) - rank(b.g) || a.i - b.i).map(e => e.g)
    }
    // The maker of the open photograph among the groups ("" when not identified).
    readonly property string photoMaker: { const g = makers.find(g => g.mine); return g ? g.maker : "" }
    property string openMaker: ""
    // Opened once per photograph: its maker, else the first group. The user's choice then stays.
    property string openedFor: "\n"
    function openDefault() {
        if (makers.length === 0 || openedFor === photoMaker) return
        openedFor = photoMaker
        openMaker = photoMaker !== "" ? photoMaker : makers[0].maker
    }
    onMakersChanged: openDefault()
    onPhotoMakerChanged: openDefault()
    Component.onCompleted: openDefault()
    function toggleMaker(maker) { openMaker = openMaker === maker ? "" : maker }
    TextMetrics { id: stateWidth; font: root.theme.textFont; text: "take off" }
    function rowName(preset) { return preset.model !== "" ? preset.model + " · " + preset.title : preset.title }

    Text {
        width: parent.width
        height: 26
        verticalAlignment: Text.AlignVCenter
        text: "all camera presets"
        color: root.theme.ink
        font: root.theme.moduleHeadingFont
    }
    Repeater {
        model: root.makers
        Column {
            id: section
            required property var modelData
            readonly property bool open: root.openMaker === modelData.maker
            width: root.width
            ToolButton {
                id: heading
                objectName: "camera-maker-" + section.modelData.maker
                width: parent.width
                height: 30
                padding: 0
                hoverEnabled: true
                onClicked: { makerNav.claim(); root.toggleMaker(section.modelData.maker) }
                NavTarget {
                    id: makerNav
                    navId: "camera-maker:" + section.modelData.maker
                    label: section.modelData.maker
                    kind: "group"
                    group: "camera:" + section.modelData.maker
                    activateLabel: section.open ? "COLLAPSE" : "EXPAND"
                    onActivate: root.toggleMaker(section.modelData.maker)
                    onAdjust: steps => { if ((steps > 0) !== section.open) root.toggleMaker(section.modelData.maker) }
                }
                Accessible.name: section.modelData.maker
                Accessible.description: section.open ? "Collapse group" : "Expand group"
                background: Rectangle {
                    radius: 3
                    color: heading.pressed ? root.theme.active : heading.hovered ? root.theme.hover : "transparent"
                    border.width: heading.visualFocus || makerNav.current ? 1 : 0
                    border.color: root.theme.accent
                }
                contentItem: RowLayout {
                    spacing: 6
                    Text {
                        Layout.leftMargin: 4
                        Layout.preferredWidth: 12
                        text: section.open ? "▾" : "▸"
                        color: root.theme.muted; font: root.theme.textFont
                    }
                    Text {
                        text: section.modelData.maker
                        color: makerNav.current ? root.theme.accent
                             : section.open || heading.hovered || section.modelData.applied > 0 ? root.theme.ink : root.theme.muted
                        font: root.theme.settingsFont
                    }
                    Text {
                        Layout.fillWidth: true
                        visible: section.modelData.mine
                        text: "this camera"
                        color: root.theme.muted; font: root.theme.textFont
                        elide: Text.ElideRight
                    }
                    Item { Layout.fillWidth: true; visible: !section.modelData.mine }
                    Text {
                        objectName: "camera-maker-count-" + section.modelData.maker
                        Layout.rightMargin: 6
                        text: section.modelData.applied > 0 ? section.modelData.applied + " of " + section.modelData.presets.length
                                                             : section.modelData.presets.length
                        color: section.modelData.applied > 0 ? root.theme.accent : root.theme.muted
                        font: root.theme.textFont
                        opacity: section.modelData.applied > 0 ? 1 : .75
                    }
                }
            }
            Repeater {
                model: section.open ? section.modelData.presets : []
                AbstractButton {
                    id: row
                    required property var modelData
                    readonly property bool applied: !!modelData.applied
                    readonly property bool available: !!modelData.available
                    readonly property string note: !available ? modelData.reason
                                                 : modelData.module + (modelData.matches ? " · matches this camera" : "")
                    objectName: "camera-preset-" + modelData.name
                    width: section.width
                    implicitHeight: Math.max(42, labels.implicitHeight + 12)
                    padding: 0
                    hoverEnabled: true
                    enabled: root.ready && available
                    onClicked: { rowNav.claim(); root.presetRequested(modelData.name, !applied) }
                    Accessible.name: root.rowName(modelData)
                    Accessible.description: !available ? modelData.reason : applied ? "Applied; take off" : "Apply"
                    Accessible.checkable: true; Accessible.checked: applied
                    NavTarget {
                        id: rowNav
                        navId: "camera-preset:" + row.modelData.name
                        label: root.rowName(row.modelData)
                        kind: "switch"
                        group: "camera:" + section.modelData.maker
                        enabled: row.enabled
                        active: row.applied
                        resettable: false
                        adjustLabel: ""
                        activateLabel: row.applied ? "TAKE OFF" : "APPLY PRESET"
                        onActivate: root.presetRequested(row.modelData.name, !row.applied)
                    }
                    background: Rectangle {
                        radius: 4
                        color: row.pressed ? root.theme.active : row.hovered ? root.theme.hover : "transparent"
                        border.width: rowNav.current || row.visualFocus ? 1 : 0
                        border.color: root.theme.accent
                    }
                    contentItem: Item {
                        opacity: row.available ? 1 : .55
                        // Applied: a filled mark. Not applied: a ring, the switch one looks for.
                        Rectangle {
                            x: 22; y: 12
                            width: 7; height: 7; radius: 3.5
                            color: row.applied ? root.theme.accent : "transparent"
                            border.width: row.applied ? 0 : 1
                            border.color: root.theme.muted
                        }
                        Column {
                            id: labels
                            x: 38; y: 6
                            width: parent.width - x - 6
                            spacing: 2
                            RowLayout {
                                width: parent.width
                                spacing: 8
                                Text {
                                    Layout.fillWidth: true
                                    text: root.rowName(row.modelData)
                                    color: rowNav.current ? root.theme.accent : row.applied || row.hovered ? root.theme.ink : root.theme.subInk
                                    font: root.theme.textFont
                                    wrapMode: Text.WordWrap
                                }
                                Text {
                                    objectName: "camera-preset-state-" + row.modelData.name
                                    Layout.alignment: Qt.AlignTop
                                    // Always as wide as its longest word, so hovering never rewraps the name.
                                    Layout.preferredWidth: stateWidth.width
                                    horizontalAlignment: Text.AlignRight
                                    text: !row.available ? "" : row.applied ? (row.hovered ? "take off" : "applied") : row.hovered || rowNav.current ? "apply" : ""
                                    color: row.applied && !row.hovered ? root.theme.accent : root.theme.ink
                                    font: root.theme.textFont
                                }
                            }
                            Text {
                                objectName: "camera-preset-note-" + row.modelData.name
                                width: parent.width
                                text: row.note
                                color: root.theme.muted
                                font: root.theme.textFont
                                wrapMode: Text.WordWrap
                            }
                        }
                    }
                }
            }
        }
    }
}
