import QtQuick
import QtTest
import "../../ui/components"

// The tools drawn on the photo, without an engine: each overlay gets sample engine data and
// must turn the pointer into the gestures documented in engine/canvas.h.
//   QT_QPA_PLATFORM=offscreen /usr/lib/qt6/bin/qmltestrunner -input omalux/tests/components/tst_canvas.qml

Item {
    width: 900; height: 1100
    EditorTheme { id: th }
    property var gestures: []
    property var changes: []
    property int interactions: 0
    function last() { return gestures[gestures.length - 1] }

    CanvasOverlay {
        id: shapes
        width: 400; height: 300
        theme: th
        tool: ({ operation: "retouch", instance: 0, kind: "shapes" })
        overlayJson: JSON.stringify({ operation: "retouch", instance: 0, kind: "shapes", tool: "retouch", algorithm: "heal",
            selected: 7, shapes: [{ id: 7, type: "circle", opacity: 1, closed: true, clone: true, center: [0.5, 0.5],
                outline: circle(0.5, 0.5, 0.1), border: circle(0.5, 0.5, 0.15), source: circle(0.8, 0.3, 0.1),
                sourceCenter: [0.8, 0.3] }] })
        onEdited: g => gestures = gestures.concat([g])
        onInteractionChanged: a => interactions++
        function circle(cx, cy, r) {
            const out = []
            for (let i = 0; i <= 36; ++i) out.push([cx + r * Math.cos(i * Math.PI / 18) * 0.75, cy + r * Math.sin(i * Math.PI / 18)])
            return out
        }
    }
    CanvasOverlay {
        id: line
        y: 310; width: 400; height: 300
        theme: th
        tool: ({ operation: "graduatednd", instance: 0, kind: "line" })
        overlayJson: JSON.stringify({ operation: "graduatednd", instance: 0, kind: "line", line: { a: [0.1, 0.5], b: [0.9, 0.5] } })
        onEdited: g => gestures = gestures.concat([g])
    }
    CanvasOverlay {
        id: vignette
        x: 450; width: 400; height: 300
        theme: th
        tool: ({ operation: "vignette", instance: 0, kind: "vignette" })
        moduleState: ({ values: { "center.x": 0, "center.y": 0, scale: 80, falloff_scale: 50, whratio: 1, autoratio: 0 } })
        onParametersEdited: c => changes = changes.concat([c])
    }
    CanvasOverlay {
        id: structure
        x: 450; y: 310; width: 400; height: 300
        theme: th
        tool: ({ operation: "ashift", instance: 0, kind: "ashift" })
        overlayJson: JSON.stringify({ operation: "ashift", instance: 0, kind: "ashift", lines: [], quad: null })
        onEdited: g => gestures = gestures.concat([g])
    }
    CanvasOverlay {
        id: liquify
        y: 620; width: 400; height: 300
        theme: th
        tool: ({ operation: "liquify", instance: 0, kind: "liquify" })
        overlayJson: JSON.stringify({ operation: "liquify", instance: 0, kind: "liquify", nodes: [
            { index: 0, type: "move", prev: -1, next: -1, warp: "linear", center: [0.5, 0.5], radius: [0.6, 0.5], strength: [0.55, 0.4] }] })
        onEdited: g => gestures = gestures.concat([g])
    }
    CanvasToolbar {
        id: toolbar
        x: 450; y: 620
        theme: th
        title: "retouch"
        tools: [{ key: "shape:circle", label: "circle" }, { key: "algo:blur", label: "blur", separated: true }]
        property var got: []
        onToolClicked: (key, modifiers) => got = got.concat([key])
    }
    CanvasToolRow {
        id: row
        x: 450; y: 680; width: 300
        theme: th
        label: "gradient line"
        hint: "drag the line on the photo"
        property int got: 0
        onActivated: got++
    }

    TestCase {
        name: "canvas"; when: windowShown

        function test_shapes_add_move_scroll_remove() {
            gestures = []
            shapes.toolClicked("shape:circle", 0)
            verify(shapes.capturing)
            mouseClick(shapes, 80, 240)
            compare(last().action, "add")
            compare(last().type, "circle")
            fuzzyCompare(last().at[0], 0.2, 0.01)
            verify(!shapes.capturing)
            // drag the selected circle by its inside
            gestures = []
            mousePress(shapes, 200, 150)
            mouseMove(shapes, 220, 160)
            mouseMove(shapes, 240, 170)
            mouseRelease(shapes, 240, 170)
            const moves = gestures.filter(g => g.action === "move")
            verify(moves.length >= 1)
            compare(moves[0].id, 7)
            fuzzyCompare(moves[moves.length - 1].to[0], 0.6, 0.01)
            // its clone source moves on its own
            gestures = []
            mousePress(shapes, 320, 90); mouseMove(shapes, 300, 100); mouseRelease(shapes, 300, 100)
            compare(gestures[0].action, "move-source")
            // the wheel scales it, Shift the feather, Ctrl the opacity
            gestures = []
            mouseWheel(shapes, 200, 150, 0, 120)
            compare(last().action, "scroll"); compare(last().up, true); compare(last().modifier, "")
            mouseWheel(shapes, 200, 150, 0, -120, Qt.NoButton, Qt.ShiftModifier)
            compare(last().modifier, "shift"); compare(last().up, false)
            mouseWheel(shapes, 200, 150, 0, -120, Qt.NoButton, Qt.ControlModifier)
            compare(last().action, "opacity"); fuzzyCompare(last().value, 0.95, 1e-6)
            // right-click removes, as in darktable
            mouseClick(shapes, 200, 150, Qt.RightButton)
            compare(last().action, "remove"); compare(last().id, 7)
        }
        function test_shapes_path_and_algorithm() {
            gestures = []
            shapes.toolClicked("shape:path", 0)
            mouseClick(shapes, 40, 40); mouseClick(shapes, 120, 40); mouseClick(shapes, 80, 100)
            mouseClick(shapes, 10, 10, Qt.RightButton)
            compare(last().action, "add"); compare(last().type, "path"); compare(last().points.length, 3)
            shapes.toolClicked("algo:blur", 0)
            compare(last().action, "algorithm"); compare(last().value, "blur"); compare(last().id, undefined)
            shapes.toolClicked("algo:clone", Qt.ControlModifier)
            compare(last().id, 7)
            // a clone source placed with Shift before the click travels with the new shape
            shapes.toolClicked("shape:ellipse", 0)
            mouseClick(shapes, 300, 250, Qt.LeftButton, Qt.ShiftModifier)
            mouseClick(shapes, 60, 250)
            compare(last().type, "ellipse"); verify(!!last().source); fuzzyCompare(last().source[0], 0.75, 0.01)
        }
        function test_line() {
            gestures = []
            mousePress(line, 360, 150)       // the end at 0.9
            mouseMove(line, 360, 200); mouseMove(line, 360, 240)
            mouseRelease(line, 360, 240)
            compare(last().action, "line"); fuzzyCompare(last().b[1], 0.8, 0.01); compare(last().keepRotation, false)
            mousePress(line, 50, 60, Qt.RightButton); mouseMove(line, 200, 60); mouseMove(line, 350, 60)
            mouseRelease(line, 350, 60, Qt.RightButton)
            fuzzyCompare(last().a[1], 0.2, 0.01); fuzzyCompare(last().b[0], 0.875, 0.01)
        }
        function test_vignette() {
            changes = []
            mousePress(vignette, 200, 150); mouseMove(vignette, 180, 140); mouseMove(vignette, 160, 120)
            mouseRelease(vignette, 160, 120)
            const c = changes[changes.length - 1]
            fuzzyCompare(c["center.x"], -0.2, 0.001); fuzzyCompare(c["center.y"], -0.2, 0.001)
            // the width handle (vignette.c:560): wider than high sets the size and the ratio
            changes = []
            const g = vignette.children[0].item.geometry(vignette.children[0].item.p)
            mousePress(vignette, g.x + g.w, g.y); mouseMove(vignette, g.x + g.w + 10, g.y); mouseMove(vignette, g.x + g.w + 20, g.y)
            mouseRelease(vignette, g.x + g.w + 20, g.y)
            const d = changes[changes.length - 1]
            fuzzyCompare(d.scale, 90, 0.01); fuzzyCompare(d.whratio, 2 - 160 / 180, 0.001)
        }
        function test_structure() {
            gestures = []
            mousePress(structure, 40, 150, Qt.RightButton); mouseMove(structure, 200, 160); mouseMove(structure, 360, 170)
            mouseRelease(structure, 360, 170, Qt.RightButton)
            compare(last().action, "straighten")
            structure.toolClicked("lines", 0)
            mousePress(structure, 100, 30); mouseMove(structure, 105, 150); mouseMove(structure, 110, 270)
            mouseRelease(structure, 110, 270)
            compare(last().action, "lines"); compare(last().lines.length, 1)
            structure.toolClicked("rectangle", 0)
            mousePress(structure, 100, 50); mouseMove(structure, 200, 150); mouseMove(structure, 300, 250)
            mouseRelease(structure, 300, 250)
            compare(last().action, "quad"); fuzzyCompare(last().bottomRight[0], 0.75, 0.01)
            // short lines are no straightening (darktable needs 25 screen pixels)
            gestures = []
            mousePress(structure, 40, 150, Qt.RightButton); mouseMove(structure, 50, 150); mouseRelease(structure, 50, 150, Qt.RightButton)
            compare(gestures.length, 0)
        }
        function test_liquify() {
            gestures = []
            mousePress(liquify, 200, 150); mouseMove(liquify, 220, 160); mouseMove(liquify, 240, 170); mouseRelease(liquify, 240, 170)
            compare(last().action, "move"); compare(last().part, "center"); compare(last().index, 0)
            // the dragged node is shown where it was dragged until the engine reports it
            mouseClick(liquify, 260, 140, Qt.LeftButton, Qt.ControlModifier)
            compare(last().action, "warp"); compare(last().value, "grow")
            liquify.toolClicked("point", 0)
            mouseClick(liquify, 50, 50)
            compare(last().action, "add-point"); fuzzyCompare(last().at[0], 0.125, 0.01)
            liquify.toolClicked("line", 0)
            mousePress(liquify, 50, 250); mouseMove(liquify, 150, 250); mouseMove(liquify, 250, 250); mouseRelease(liquify, 250, 250)
            compare(last().action, "add-line")
            mouseClick(liquify, 200, 150, Qt.RightButton)
            compare(last().action, "remove")
        }
        function test_toolbar_and_row() {
            mouseClick(toolbar, toolbar.width - 20, 16)
            compare(toolbar.got[toolbar.got.length - 1], "algo:blur")
            mouseClick(row, row.width - 30, 10)
            compare(row.got, 1)
        }
    }
}
