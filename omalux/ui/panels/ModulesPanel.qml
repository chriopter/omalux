import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import QtCore
import "../components"

// Every processing module darktable describes about itself. Controls are built from that
// description, not from our registry, so a module needs no entry here to be adjustable.
SidebarScrollView {
    id: root
    objectName: "modulesScroll"
    required property var theme
    required property string catalog
    // How darktable shows each slider: unit, factor, digits, and the range it covers first.
    required property string display
    required property bool editable
    // Show only what the curated panel does not cover yet, so the list is the remaining work.
    property bool onlyOpen: true
    signal interactionChanged(bool active)
    signal parameterEdited(string operation, int instance, string field, real value)

    property bool restoring: true
    property var expanded: ({})
    property string search: ""

    Settings {
        id: preferences
        category: "ModulesPanel"
        property string expanded: '{}'
    }
    Component.onCompleted: {
        try { root.expanded = JSON.parse(preferences.expanded) } catch (e) {}
        restoring = false
    }
    onExpandedChanged: if (!restoring) preferences.expanded = JSON.stringify(expanded)

    readonly property var modules: {
        let parsed = []
        try { parsed = JSON.parse(root.catalog || "[]") } catch (e) { return [] }
        const term = root.search.trim().toLowerCase()
        return parsed.filter(m => !m.hidden)
                     .map(m => Object.assign({}, m, {
                         parameters: m.parameters.filter(p => (!root.onlyOpen || !p.curated)
                             && (!term || (p.label || p.field).toLowerCase().includes(term)
                                       || m.label.toLowerCase().includes(term)
                                       || m.operation.toLowerCase().includes(term)))
                     }))
                     .filter(m => m.parameters.length > 0)
    }
    readonly property int shownParameters: modules.reduce((n, m) => n + m.parameters.length, 0)
    readonly property int describedParameters: {
        let parsed = []
        try { parsed = JSON.parse(root.catalog || "[]") } catch (e) { return 0 }
        return parsed.filter(m => !m.hidden).reduce((n, m) => n + m.parameters.length, 0)
    }
    readonly property int curatedParameters: {
        let parsed = []
        try { parsed = JSON.parse(root.catalog || "[]") } catch (e) { return 0 }
        return parsed.filter(m => !m.hidden)
                     .reduce((n, m) => n + m.parameters.filter(p => p.curated).length, 0)
    }

    function slidable(parameter) {
        return (parameter.type === "float" || parameter.type === "int")
               && isFinite(parameter.minimum) && isFinite(parameter.maximum)
               && parameter.maximum > parameter.minimum
    }
    readonly property var displayData: {
        try { return JSON.parse(root.display || "{}") } catch (e) { return ({}) }
    }
    // Ranges from the module sources are hard limits. Where darktable says how it shows the
    // value, follow it; otherwise pick a readable step and precision.
    function control(module, parameter) {
        const shown = root.displayData[module.operation + "/" + parameter.field] || {}
        const factor = shown.factor || 1
        const span = (parameter.maximum - parameter.minimum) * Math.abs(factor)
        const decimals = shown.digits !== undefined ? shown.digits
                       : parameter.type === "int" ? 0 : span > 200 ? 1 : span > 20 ? 2 : 3
        const low = Math.min(parameter.minimum * factor, parameter.maximum * factor)
        const high = Math.max(parameter.minimum * factor, parameter.maximum * factor)
        return {
            id: module.operation + "/" + module.instance + "/" + parameter.name,
            label: parameter.label || parameter.field,
            unit: shown.format || "", colors: "", decimals: decimals,
            step: parameter.type === "int" ? 1 : span / 1000,
            minimum: low, maximum: high,
            softMinimum: shown.soft_minimum !== undefined ? shown.soft_minimum * factor : low,
            softMaximum: shown.soft_maximum !== undefined ? shown.soft_maximum * factor : high,
            factor: factor
        }
    }
    function setExpanded(key, value) {
        let next = Object.assign({}, root.expanded); next[key] = value; root.expanded = next
    }

    Column {
        width: root.availableWidth
        padding: 18
        spacing: 10

        RowLayout {
            width: parent.width - 36
            Text {
                text: root.onlyOpen ? "NOT YET DESIGNED" : "ALL MODULES"
                color: root.theme.ink; font.bold: true; font.letterSpacing: 2
            }
            Item { Layout.fillWidth: true }
            Text {
                text: root.modules.length + " · " + root.shownParameters
                color: root.theme.muted; font: root.theme.textFont
            }
        }
        Text {
            width: parent.width - 36
            text: root.onlyOpen
                  ? root.curatedParameters + " of " + root.describedParameters
                    + " parameters have a designed control. What is left is listed here."
                  : "Every parameter darktable describes, designed or not."
            color: root.theme.muted; font: root.theme.textFont; wrapMode: Text.WordWrap
        }
        TextField {
            id: filter
            width: parent.width - 36
            placeholderText: "Search modules and parameters"
            color: root.theme.ink
            font: root.theme.textFont
            onTextChanged: root.search = text
            background: Rectangle {
                color: Qt.lighter(root.theme.background, 1.16)
                border.color: filter.activeFocus ? root.theme.accent : root.theme.line
                radius: 4
            }
        }

        Repeater {
            model: root.modules
            delegate: Column {
                id: moduleBlock
                required property var modelData
                width: parent.width - 36
                spacing: 4
                readonly property bool open: !!root.expanded[modelData.operation + "/" + modelData.instance]

                Item {
                    width: parent.width
                    implicitHeight: 32
                    Rectangle {
                        anchors.fill: parent
                        color: moduleBlock.open ? Qt.lighter(root.theme.background, 1.16) : "transparent"
                        radius: 4
                    }
                    RowLayout {
                        anchors.fill: parent
                        anchors.leftMargin: 8; anchors.rightMargin: 8
                        spacing: 8
                        Text {
                            text: moduleBlock.open ? "▾" : "▸"
                            color: root.theme.muted; font: root.theme.textFont
                        }
                        Text {
                            Layout.fillWidth: true
                            text: moduleBlock.modelData.label
                                  + (moduleBlock.modelData.instance > 0 ? " " + (moduleBlock.modelData.instance + 1) : "")
                            color: moduleBlock.modelData.enabled ? root.theme.ink : root.theme.muted
                            font: root.theme.textFont
                            elide: Text.ElideRight
                        }
                        Text {
                            text: moduleBlock.modelData.enabled ? "on" : "off"
                            color: moduleBlock.modelData.enabled ? root.theme.accent : root.theme.muted
                            font: root.theme.textFont
                        }
                        Text {
                            text: moduleBlock.modelData.parameters.length
                            color: root.theme.muted; font: root.theme.textFont
                        }
                    }
                    MouseArea {
                        anchors.fill: parent
                        onClicked: root.setExpanded(moduleBlock.modelData.operation + "/" + moduleBlock.modelData.instance,
                                                    !moduleBlock.open)
                    }
                }

                Column {
                    visible: moduleBlock.open
                    width: parent.width
                    spacing: 2
                    Repeater {
                        model: moduleBlock.modelData.parameters
                        delegate: Loader {
                            required property var modelData
                            width: moduleBlock.width
                            sourceComponent: root.slidable(modelData) ? sliderRow : plainRow
                            property var parameter: modelData
                            property var module: moduleBlock.modelData
                        }
                    }
                }
            }
        }
    }

    Component {
        id: sliderRow
        ControlSlider {
            theme: root.theme
            control: root.control(module, parameter)
            value: parameter.value * (root.control(module, parameter).factor || 1)
            editable: root.editable
            compact: true
            moduleToggleAvailable: false
            qualifyLabel: false
            onEdited: value => root.parameterEdited(module.operation, module.instance, parameter.name,
                                                   value / (root.control(module, parameter).factor || 1))
            onInteractionChanged: active => root.interactionChanged(active)
        }
    }
    Component {
        id: plainRow
        Item {
            implicitHeight: 26
            RowLayout {
                anchors.fill: parent
                anchors.leftMargin: 8; anchors.rightMargin: 8
                spacing: 8
                Text {
                    Layout.fillWidth: true
                    text: parameter.label || parameter.field
                    color: root.theme.muted; font: root.theme.textFont; elide: Text.ElideRight
                }
                Text {
                    text: parameter.type === "enum" && parameter.values
                          ? (parameter.values.find(v => v.value === parameter.value) || {}).label || parameter.value
                          : parameter.type === "bool" ? (parameter.value > 0.5 ? "on" : "off")
                          : parameter.type === "array" ? parameter.count + "×" + parameter.element
                          : parameter.type
                    color: root.theme.muted; font: root.theme.textFont
                }
            }
        }
    }
}
