import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

// The film profiles of the camera catalogue (lookup tables for darktable's LUT 3D), one group
// per set, to put under the look by hand:
//
//   ▾ DHH ……………………………………… Portra 400 - C
//     [ Filter 89 films            ]
//     ▾ Kodak ………………………………… 33
//       Portra 400 ……………………… [C] [L]
//       Portra 800 ……………………… [C] [L]
//     ▸ Fuji …………………………………… 25
//
// A film stock is one row with its variants as a compact choice. One film is on at a time:
// choosing another replaces it, a click on the one that is on takes it off. `films` are the
// engine's rows with "film" set (Editor.cameraPresets); the choice leaves as
// presetRequested(name, on).
Column {
    id: root
    required property var theme
    property var films: []
    // false while the engine is busy or no photograph is open: rows do not act.
    property bool ready: true
    signal presetRequested(string name, bool on)

    spacing: 2
    visible: groups.length > 0

    // "Portra 160" before "Portra 400" before "Portra 1600": digits compare as numbers.
    function natural(a, b) {
        const x = a.toLowerCase().split(/(\d+)/), y = b.toLowerCase().split(/(\d+)/)
        for (let i = 0; i < Math.min(x.length, y.length); ++i) {
            if (x[i] === y[i]) continue
            const n = parseInt(x[i]), m = parseInt(y[i])
            if (!isNaN(n) && !isNaN(m)) return n - m
            return x[i] < y[i] ? -1 : 1
        }
        return x.length - y.length
    }
    // Under its brand a film drops the brand's name ("Kodak Portra 400" reads "Portra 400").
    function stockLabel(stock, brand) {
        return brand !== "" && stock.indexOf(brand + " ") === 0 ? stock.slice(brand.length + 1) : stock
    }
    // [{ group, count, applied (the film that is on, or null),
    //    brands: [{ brand, stocks: [{ stock, label, variants: [film…] }] }] }]
    readonly property var groups: {
        const out = []
        for (const film of root.films || []) {
            let group = out.find(g => g.group === film.group)
            if (!group) out.push(group = { group: film.group, count: 0, applied: null, brands: [] })
            const name = film.brand || ""
            let brand = group.brands.find(b => b.brand === name)
            if (!brand) group.brands.push(brand = { brand: name, stocks: [] })
            let stock = brand.stocks.find(s => s.stock === film.stock)
            if (!stock) { brand.stocks.push(stock = { stock: film.stock, label: stockLabel(film.stock, name), variants: [] }); ++group.count }
            stock.variants.push(film)
            if (film.applied) group.applied = film
        }
        for (const group of out) {
            // Brands by name, films without a known one last.
            group.brands.sort((a, b) => (a.brand === "Other" || a.brand === "") - (b.brand === "Other" || b.brand === "") || natural(a.brand, b.brand))
            for (const brand of group.brands) {
                brand.stocks.sort((a, b) => natural(a.label, b.label))
                for (const stock of brand.stocks) stock.variants.sort((a, b) => natural(a.variant, b.variant))
            }
        }
        return out
    }
    property string openGroup: ""
    // "group|brand"; one brand is open at a time.
    property string openBrand: ""
    property string query: ""
    function toggleGroup(group) { openGroup = openGroup === group ? "" : group }
    function toggleBrand(key) { openBrand = openBrand === key ? "" : key }
    function matches(stock, brand, q) {
        if (q === "") return true
        const text = (stock.stock + " " + brand + " " + stock.variants.map(v => v.title).join(" ")).toLowerCase()
        return q.toLowerCase().split(/\s+/).filter(w => w !== "").every(w => text.indexOf(w) >= 0)
    }
    // The film that is on, across groups; a choice in one group replaces it.
    function choose(film) { if (root.ready && film.available) root.presetRequested(film.name, !film.applied) }
    // Keyboard on a row: Enter puts the first variant on (or takes the film off), ←/→ step
    // through off and the variants.
    function activateStock(stock) {
        const on = stock.variants.find(v => v.applied)
        const first = stock.variants.find(v => v.available)
        if (on) choose(on); else if (first) choose(first)
    }
    function stepStock(stock, steps) {
        const usable = stock.variants.filter(v => v.available)
        const at = usable.findIndex(v => v.applied)
        const next = at + (steps > 0 ? 1 : -1)
        if (next >= 0 && next < usable.length) choose(usable[next])
        else if (next < 0 && at >= 0) choose(usable[at])
    }

    Repeater {
        model: root.groups
        Column {
            id: section
            required property var modelData
            readonly property string name: modelData.group
            readonly property bool open: root.openGroup === name
            readonly property string q: open ? root.query.trim() : ""
            readonly property var shownBrands: {
                if (!open) return []
                const out = []
                for (const brand of modelData.brands) {
                    const stocks = brand.stocks.filter(s => root.matches(s, brand.brand, q))
                    if (stocks.length > 0) out.push({ brand: brand.brand, key: name + "|" + brand.brand, stocks: stocks, total: brand.stocks.length })
                }
                return out
            }
            width: root.width
            spacing: 2
            ToolButton {
                id: heading
                objectName: "camera-films-" + section.name
                width: parent.width
                height: 30
                padding: 0
                hoverEnabled: true
                onClicked: { groupNav.claim(); root.toggleGroup(section.name) }
                NavTarget {
                    id: groupNav
                    navId: "camera-films:" + section.name
                    label: section.name + " film profiles"
                    kind: "group"
                    group: "films:" + section.name
                    activateLabel: section.open ? "COLLAPSE" : "EXPAND"
                    onActivate: root.toggleGroup(section.name)
                    onAdjust: steps => { if ((steps > 0) !== section.open) root.toggleGroup(section.name) }
                }
                Accessible.name: section.name + " film profiles"
                Accessible.description: section.open ? "Collapse group" : "Expand group"
                background: Rectangle {
                    radius: 3
                    color: heading.pressed ? root.theme.active : heading.hovered ? root.theme.hover : "transparent"
                    border.width: heading.visualFocus || groupNav.current ? 1 : 0
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
                        text: section.name
                        color: groupNav.current ? root.theme.accent
                             : section.open || heading.hovered || section.modelData.applied ? root.theme.ink : root.theme.muted
                        font: root.theme.settingsFont
                    }
                    Text {
                        text: "film profiles"
                        color: root.theme.muted; font: root.theme.textFont
                    }
                    Text {
                        objectName: "camera-films-state-" + section.name
                        Layout.fillWidth: true
                        Layout.rightMargin: 6
                        horizontalAlignment: Text.AlignRight
                        elide: Text.ElideLeft
                        text: section.modelData.applied ? section.modelData.applied.title : section.modelData.count
                        color: section.modelData.applied ? root.theme.accent : root.theme.muted
                        font: root.theme.textFont
                        opacity: section.modelData.applied ? 1 : .75
                    }
                }
            }
            Text {
                visible: section.open
                x: 22
                width: parent.width - x - 6
                text: "One film at a time, under the look. " + (section.modelData.brands.some(b => b.stocks.some(s => s.variants.length > 1))
                      ? "The letters are the variants of the set." : "")
                color: root.theme.muted; font: root.theme.textFont; wrapMode: Text.WordWrap
                bottomPadding: 4
            }
            TextField {
                id: filter
                objectName: "film-filter-" + section.name
                visible: section.open
                x: 22
                width: parent.width - x
                implicitHeight: 30
                leftPadding: 10
                placeholderText: "Filter " + section.modelData.count + " films"
                color: root.theme.ink
                placeholderTextColor: root.theme.muted; font: root.theme.textFont
                selectByMouse: true
                text: root.query
                onTextChanged: if (section.open) root.query = text
                Keys.onEscapePressed: { text = ""; focus = false }
                NavTarget { id: filterNav; navId: "film-filter:" + section.name; kind: "search"; label: "filter films"; input: filter; onActivate: filter.forceActiveFocus() }
                background: Rectangle { color: root.theme.well; border.color: filter.activeFocus || filterNav.current ? root.theme.accent : root.theme.line; radius: 5 }
                Accessible.name: "Filter films"
            }
            Text {
                objectName: "film-none-" + section.name
                visible: section.open && section.shownBrands.length === 0
                x: 22
                width: parent.width - x
                topPadding: 4
                text: "No film matches “" + section.q + "”."
                color: root.theme.muted; font: root.theme.textFont; wrapMode: Text.WordWrap
            }
            Repeater {
                model: section.shownBrands
                Column {
                    id: brand
                    required property var modelData
                    // While filtering every brand with a match is open.
                    readonly property bool open: section.q !== "" || root.openBrand === modelData.key
                    x: 14
                    width: section.width - x
                    ToolButton {
                        id: brandHeading
                        objectName: "film-brand-" + brand.modelData.key
                        width: parent.width
                        height: 28
                        padding: 0
                        hoverEnabled: true
                        visible: brand.modelData.brand !== "" || section.shownBrands.length > 1
                        onClicked: { brandNav.claim(); root.toggleBrand(brand.modelData.key) }
                        NavTarget {
                            id: brandNav
                            navId: "film-brand:" + brand.modelData.key
                            label: brand.modelData.brand
                            kind: "group"
                            group: "films:" + brand.modelData.key
                            enabled: brandHeading.visible
                            activateLabel: brand.open ? "COLLAPSE" : "EXPAND"
                            onActivate: root.toggleBrand(brand.modelData.key)
                            onAdjust: steps => { if ((steps > 0) !== brand.open) root.toggleBrand(brand.modelData.key) }
                        }
                        Accessible.name: brand.modelData.brand
                        Accessible.description: brand.open ? "Collapse group" : "Expand group"
                        background: Rectangle {
                            radius: 3
                            color: brandHeading.pressed ? root.theme.active : brandHeading.hovered ? root.theme.hover : "transparent"
                            border.width: brandHeading.visualFocus || brandNav.current ? 1 : 0
                            border.color: root.theme.accent
                        }
                        contentItem: RowLayout {
                            spacing: 6
                            Text {
                                Layout.leftMargin: 4
                                Layout.preferredWidth: 12
                                text: brand.open ? "▾" : "▸"
                                color: root.theme.muted; font: root.theme.textFont
                            }
                            Text {
                                Layout.fillWidth: true
                                text: brand.modelData.brand
                                color: brandNav.current ? root.theme.accent : brand.open || brandHeading.hovered ? root.theme.ink : root.theme.subInk
                                font: root.theme.textFont
                            }
                            Text {
                                objectName: "film-brand-count-" + brand.modelData.key
                                Layout.rightMargin: 6
                                text: section.q !== "" ? brand.modelData.stocks.length + " of " + brand.modelData.total : brand.modelData.total
                                color: root.theme.muted; font: root.theme.textFont
                                opacity: .75
                            }
                        }
                    }
                    Repeater {
                        model: brand.open || !brandHeading.visible ? brand.modelData.stocks : []
                        Item {
                            id: row
                            required property var modelData
                            readonly property var on: modelData.variants.find(v => v.applied) || null
                            readonly property bool usable: modelData.variants.some(v => v.available)
                            readonly property string reason: usable ? "" : modelData.variants[0].reason
                            objectName: "film-" + modelData.stock
                            width: brand.width
                            height: Math.max(30, reasonText.visible ? 44 : 30)
                            NavTarget {
                                id: rowNav
                                navId: "film:" + section.name + ":" + row.modelData.stock
                                label: row.modelData.stock
                                kind: "choice"
                                group: "films:" + brand.modelData.key
                                enabled: root.ready && row.usable
                                active: !!row.on
                                resettable: false
                                adjustLabel: row.modelData.variants.length > 1 ? "VARIANT" : ""
                                activateLabel: row.on ? "TAKE OFF" : "APPLY FILM"
                                onActivate: root.activateStock(row.modelData)
                                onAdjust: steps => root.stepStock(row.modelData, steps)
                            }
                            Rectangle {
                                anchors.fill: parent
                                radius: 4
                                color: rowHover.hovered ? root.theme.hover : "transparent"
                                border.width: rowNav.current ? 1 : 0
                                border.color: root.theme.accent
                            }
                            HoverHandler { id: rowHover }
                            // On: a filled mark, as in the camera preset rows.
                            Rectangle {
                                x: 8; y: 12
                                width: 7; height: 7; radius: 3.5
                                opacity: row.usable ? 1 : .55
                                color: row.on ? root.theme.accent : "transparent"
                                border.width: row.on ? 0 : 1
                                border.color: root.theme.muted
                            }
                            Text {
                                id: stockName
                                x: 24; y: 0
                                height: 30
                                width: chips.x - x - 6
                                verticalAlignment: Text.AlignVCenter
                                text: row.modelData.label
                                elide: Text.ElideRight
                                opacity: row.usable ? 1 : .55
                                color: rowNav.current ? root.theme.accent : row.on || rowHover.hovered ? root.theme.ink : root.theme.subInk
                                font: root.theme.textFont
                            }
                            Text {
                                id: reasonText
                                objectName: "film-note-" + row.modelData.stock
                                visible: row.reason !== ""
                                x: 24; y: 24
                                width: parent.width - x - 6
                                text: row.reason
                                elide: Text.ElideRight
                                color: root.theme.muted; font: root.theme.textFont
                            }
                            Row {
                                id: chips
                                anchors.right: parent.right
                                anchors.rightMargin: 6
                                y: 4
                                spacing: 4
                                Repeater {
                                    model: row.modelData.variants
                                    AbstractButton {
                                        id: chip
                                        required property var modelData
                                        readonly property bool applied: !!modelData.applied
                                        objectName: "film-variant-" + modelData.name
                                        width: Math.max(26, chipText.implicitWidth + 12)
                                        height: 22
                                        padding: 0
                                        hoverEnabled: true
                                        enabled: root.ready && !!modelData.available
                                        onClicked: { rowNav.claim(); root.choose(modelData) }
                                        Accessible.name: modelData.title
                                        Accessible.description: !modelData.available ? modelData.reason : applied ? "On; take off" : "Put this film on"
                                        Accessible.checkable: true; Accessible.checked: applied
                                        ToolTip.visible: hovered
                                        ToolTip.delay: 500
                                        ToolTip.text: !modelData.available ? modelData.title + ": " + modelData.reason
                                                    : applied ? modelData.title + " — take off" : modelData.title
                                        background: Rectangle {
                                            radius: 4
                                            color: chip.applied ? root.theme.accent : chip.pressed ? root.theme.active : chip.hovered ? root.theme.hover : root.theme.well
                                            border.width: 1
                                            border.color: chip.applied || chip.hovered || chip.visualFocus ? root.theme.accent : root.theme.line
                                            opacity: chip.enabled || !!chip.modelData.available ? 1 : .5
                                        }
                                        contentItem: Text {
                                            id: chipText
                                            text: chip.modelData.variant !== "" ? chip.modelData.variant : "on"
                                            horizontalAlignment: Text.AlignHCenter
                                            verticalAlignment: Text.AlignVCenter
                                            color: chip.applied ? root.theme.well : chip.hovered ? root.theme.ink : root.theme.subInk
                                            font: root.theme.textFont
                                            opacity: !!chip.modelData.available ? 1 : .5
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
