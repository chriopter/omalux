import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import "../components"

// Info: what the file says about the photograph, grouped and written as a photographer reads
// it (1/640 s, f/5.6, 24 mm). Values the file does not carry are left out; nothing is edited here.
SidebarScrollView {
    id: root
    required property var theme
    required property var metadata

    // darktable's own rule for exposure times (dt_util_format_exposure), with "s" for seconds.
    function exposureText(seconds) {
        if (!(seconds > 0)) return ""
        const whole = v => Math.abs(Math.round(v) - v) < 1e-4
        if (seconds >= 1) return (whole(seconds) ? seconds.toFixed(0) : seconds.toFixed(1)) + " s"
        const inverse = 1 / seconds
        if (seconds < .29 || whole(inverse)) return "1/" + inverse.toFixed(0) + " s"
        if (10 * Math.round(10 / seconds) === Math.round(100 / seconds)) return "1/" + inverse.toFixed(1) + " s"
        return seconds.toFixed(1) + " s"
    }
    function sizeText(bytes) {
        if (!(bytes > 0)) return ""
        if (bytes < 1024) return bytes + " B"
        if (bytes < 1024 * 1024) return (bytes / 1024).toFixed(0) + " kB"
        if (bytes < 1024 * 1024 * 1024) return (bytes / 1024 / 1024).toFixed(1) + " MB"
        return (bytes / 1024 / 1024 / 1024).toFixed(2) + " GB"
    }
    function clean(text) { return String(text === undefined || text === null ? "" : text).replace(/\(null\)/g, "").trim() }
    function coordinate(value, positive, negative) {
        return Math.abs(value).toFixed(5) + "° " + (value >= 0 ? positive : negative)
    }
    // [{ title, rows: [[label, value], …] }], without empty rows and empty groups.
    function groups(m) {
        m = m || {}
        const path = clean(m.path)
        const slash = path.lastIndexOf("/")
        const name = slash >= 0 ? path.slice(slash + 1) : path
        const dot = name.lastIndexOf(".")
        const kind = [dot > 0 ? name.slice(dot + 1).toUpperCase() : "", m.raw ? "raw" : "", m.hdr ? "HDR" : "",
                      m.monochrome ? "monochrome" : ""].filter(part => part !== "").join(" · ")
        const width = Number(m.width) || 0, height = Number(m.height) || 0
        const focal = Number(m["focal length mm"]) || 0, crop = Number(m["crop factor"]) || 0
        const camera = clean(m.camera) || [clean(m.maker), clean(m.model)].filter(part => part !== "").join(" ")
        const list = [
            { title: "File", rows: [
                ["name", name],
                ["folder", slash > 0 ? path.slice(0, slash) : ""],
                ["type", kind],
                ["size", sizeText(Number(m.bytes) || 0)],
                ["pixels", width > 0 && height > 0 ? width + " × " + height + " · " + (width * height / 1e6).toFixed(1) + " MP" : ""] ] },
            { title: "Capture", rows: [
                ["date", clean(m.datetime).replace(/^(\d{4}):(\d{2}):(\d{2})/, "$1-$2-$3")],
                ["camera", camera],
                ["lens", clean(m.lens)],
                ["exposure", exposureText(Number(m["exposure seconds"]) || 0)],
                ["aperture", m.aperture > 0 ? "f/" + Number(m.aperture).toFixed(1) : ""],
                ["ISO", m.ISO > 0 ? String(Math.round(m.ISO)) : ""],
                ["focal length", focal > 0 ? (Math.abs(Math.round(focal) - focal) < .05 ? focal.toFixed(0) : focal.toFixed(1)) + " mm"
                                             + (crop > 0 && Math.abs(crop - 1) > .01 ? " · " + (focal * crop).toFixed(0) + " mm at 35 mm" : "") : ""],
                ["exposure bias", m["exposure bias known"] && (m.ISO > 0 || m.aperture > 0)
                                  ? (m["exposure bias"] > 0 ? "+" : "") + Number(m["exposure bias"]).toFixed(1) + " EV" : ""],
                ["focus distance", m["focus distance"] > 0 ? Number(m["focus distance"]).toFixed(2) + " m" : ""],
                ["program", clean(m["exposure program"])],
                ["metering", clean(m.metering)],
                ["white balance", clean(m["white balance"])],
                ["flash", clean(m.flash)] ] },
            // 0° N 0° E is what cameras without a fix write, not a place.
            { title: "Location", rows: m.latitude !== undefined && m.longitude !== undefined
                                       && (Math.abs(m.latitude) > 1e-6 || Math.abs(m.longitude) > 1e-6) ? [
                ["latitude", coordinate(m.latitude, "N", "S")],
                ["longitude", coordinate(m.longitude, "E", "W")],
                ["elevation", m.elevation !== undefined ? Number(m.elevation).toFixed(0) + " m" : ""] ] : [] }
        ]
        // Without camera data darktable falls back to the file's own time: not a capture date.
        const capture = list[1].rows
        if (!capture.some(row => row[0] !== "date" && row[1] !== "")) capture.length = 0
        return list.map(group => ({ title: group.title, rows: group.rows.filter(row => row[1] !== "") }))
                   .filter(group => group.rows.length > 0)
    }
    readonly property var shown: groups(root.metadata)
    readonly property bool hasCapture: shown.some(group => group.title === "Capture")

    Column {
        width: root.availableWidth
        spacing: 12
        Text {
            text: "INFO"
            color: root.theme.ink
            font.family: root.theme.moduleHeadingFont.family
            font.pixelSize: root.theme.moduleHeadingFont.pixelSize
            font.weight: Font.Bold
            font.letterSpacing: 2
        }
        Repeater {
            model: root.shown
            Rectangle {
                id: card
                required property var modelData
                objectName: "info-group-" + modelData.title.toLowerCase()
                width: parent.width
                height: cardColumn.implicitHeight + 20
                color: root.theme.surface
                radius: 4
                Column {
                    id: cardColumn
                    x: 10; y: 10
                    width: parent.width - 20
                    spacing: 6
                    Text {
                        text: card.modelData.title
                        color: root.theme.ink
                        font.family: root.theme.textFont.family
                        font.pixelSize: root.theme.textFont.pixelSize
                        font.bold: true
                        bottomPadding: 2
                    }
                    Repeater {
                        model: card.modelData.rows
                        Item {
                            id: row
                            required property var modelData
                            objectName: "info-" + modelData[0].replace(/ /g, "-")
                            readonly property string value: modelData[1]
                            width: cardColumn.width
                            implicitHeight: Math.max(rowLabel.implicitHeight, rowValue.implicitHeight)
                            Text {
                                id: rowLabel
                                text: row.modelData[0]
                                color: root.theme.muted
                                font: root.theme.textFont
                            }
                            Text {
                                id: rowValue
                                anchors.right: parent.right
                                width: Math.min(implicitWidth, row.width - rowLabel.implicitWidth - 14)
                                horizontalAlignment: Text.AlignRight
                                wrapMode: Text.WrapAnywhere
                                text: row.value
                                color: root.theme.ink
                                font: root.theme.textFont
                            }
                        }
                    }
                }
            }
        }
        Text {
            objectName: "info-no-capture"
            visible: !root.hasCapture
            width: parent.width
            text: root.shown.length ? "This file carries no camera data." : "No photograph."
            color: root.theme.muted
            font: root.theme.textFont
            wrapMode: Text.WordWrap
        }
    }
}
