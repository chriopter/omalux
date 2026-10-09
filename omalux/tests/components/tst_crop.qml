import QtQuick
import QtTest
import "../../ui/components"
import "../../ui/components/CropMath.js" as Crop

// The crop frame on the photo with real pointer events: darktable's grab bands, modifiers and
// right-click (src/iop/crop.c), and its aspect list.
//   QT_QPA_PLATFORM=offscreen /usr/lib/qt6/bin/qmltestrunner -input omalux/tests/components/tst_crop.qml
Item {
    width: 700; height: 500
    property var box: ({ x: 0, y: 0, width: 1, height: 1 })
    CropOverlay {
        id: overlay
        width: 600; height: 400
        crop: parent.box
        onCropChangedByUser: rect => parent.box = rect
    }
    TestCase {
        name: "Crop"
        when: windowShown
        function init() { box = { x: 0, y: 0, width: 1, height: 1 }; overlay.aspectRatio = 0 }
        function drag(x0, y0, x1, y1, modifiers, button) {
            mousePress(overlay, x0, y0, button || Qt.LeftButton, modifiers || Qt.NoModifier)
            for (let i = 1; i <= 5; ++i)
                mouseMove(overlay, x0 + (x1 - x0) * i / 5, y0 + (y1 - y0) * i / 5, -1, button || Qt.LeftButton, modifiers || Qt.NoModifier)
            mouseRelease(overlay, x1, y1, button || Qt.LeftButton, modifiers || Qt.NoModifier)
        }
        function near(actual, expected, what) { verify(Math.abs(actual - expected) < 0.004, what + ": " + actual + " ≠ " + expected) }

        function test_01_aspect_list_as_darktable() {
            compare(Crop.aspects.length, 22)
            compare(Crop.aspectName(Crop.aspects[0]), "freehand")
            compare(Crop.aspectName(Crop.aspects[1]), "original image")
            compare(Crop.aspectName(Crop.aspects[2]), "square  1.00")
            compare(Crop.aspectName(Crop.aspects[11]), "3:2, 4x6, 35mm  1.50")
            compare(Crop.aspectName(Crop.aspects[21]), "3:1, panorama  3.00")
            // sorted from most to least square
            for (let i = 3; i < Crop.aspects.length; ++i)
                verify(Crop.aspects[i].d / Crop.aspects[i].n >= Crop.aspects[i - 1].d / Math.max(1, Crop.aspects[i - 1].n))
            // the long side of the frame follows the long side of the photo; a negative ratio_d flips
            compare(Crop.ratio(3, 2, 1.5), 1.5)
            compare(Crop.ratio(-3, 2, 1.5), 2 / 3)
            compare(Crop.ratio(3, 2, 2 / 3), 2 / 3)
            compare(Crop.ratio(-3, 2, 2 / 3), 1.5)
            compare(Crop.ratio(1, 0, 1.5), 1.5)
            compare(Crop.ratio(-1, 0, 1.5), 1 / 1.5)
            compare(Crop.ratio(0, 0, 1.5), 0)
            compare(Crop.aspectIndex(-16, 9), 14)
        }
        function test_02_whole_photo_drags_from_anywhere_near_a_side() {
            // darktable: the bands reach 45 % into an untouched frame
            drag(60, 40, 120, 80)
            near(box.x, 0.1, "left"); near(box.y, 0.1, "top"); near(box.width, 0.9, "width"); near(box.height, 0.9, "height")
        }
        function test_03_edge_bands_resize_along_the_whole_edge() {
            box = { x: 0.2, y: 0.2, width: 0.6, height: 0.6 }
            // right edge, far from its mark, 10 px inside
            drag(470, 150, 410, 150)
            near(box.x, 0.2, "left stays"); near(box.width, 0.5, "width"); near(box.height, 0.6, "height stays")
            // top edge
            drag(200, 90, 200, 130)
            near(box.y, 0.3, "top"); near(box.height, 0.5, "height")
            // a corner moves both
            drag(125, 125, 185, 165)
            near(box.x, 0.3, "corner left"); near(box.y, 0.4, "corner top")
        }
        function test_04_inside_moves_shift_and_ctrl_constrain() {
            box = { x: 0.2, y: 0.2, width: 0.4, height: 0.4 }
            drag(240, 160, 300, 200)
            near(box.x, 0.3, "x"); near(box.y, 0.3, "y")
            drag(300, 200, 240, 160, Qt.ShiftModifier)
            near(box.x, 0.3, "Shift: only up and down"); near(box.y, 0.2, "y")
            drag(300, 160, 240, 200, Qt.ControlModifier)
            near(box.x, 0.2, "Ctrl: only sideways"); near(box.y, 0.2, "y stays")
            near(box.width, 0.4, "size kept"); near(box.height, 0.4, "size kept")
        }
        function test_05_shift_resize_keeps_centre_and_proportions() {
            box = { x: 0.2, y: 0.2, width: 0.6, height: 0.6 }
            drag(125, 200, 155, 200, Qt.ShiftModifier)
            near(box.width, 0.5, "width"); near(box.height, 0.5, "height")
            near(box.x + box.width / 2, 0.5, "centre x"); near(box.y + box.height / 2, 0.5, "centre y")
        }
        function test_06_right_click_resets() {
            box = { x: 0.2, y: 0.2, width: 0.4, height: 0.4 }
            mouseClick(overlay, 300, 200, Qt.RightButton)
            compare(box.width, 1); compare(box.height, 1)
        }
        function test_07_aspect_is_kept() {
            overlay.aspectRatio = 1       // square in photo pixels (600 × 400)
            box = { x: 0.25, y: 0.125, width: 0.5, height: 0.75 }
            drag(445, 200, 415, 200)
            near(box.width * 600, box.height * 400, "square after an edge drag")
            drag(155, 60, 185, 90)
            near(box.width * 600, box.height * 400, "square after a corner drag")
            verify(box.x >= 0 && box.y >= 0 && box.x + box.width <= 1.0001 && box.y + box.height <= 1.0001)
        }
        function test_08_outside_the_frame_nothing_moves_and_minimum_size() {
            box = { x: 0.4, y: 0.4, width: 0.2, height: 0.2 }
            drag(60, 60, 200, 200)
            near(box.x, 0.4, "untouched")
            drag(355, 240, 100, 240)
            verify(box.width >= Crop.MINIMUM - 1e-6, "the frame keeps darktable's minimum size")
        }
        function test_09_margins() {
            // one margin slider (gui_changed): the opposite side stays
            let b = Crop.setEdge({ x: 0, y: 0, width: 1, height: 1 }, Crop.LEFT, 0.25, 0, 3, 2)
            compare(b.x, 0.25); compare(b.width, 0.75)
            b = Crop.setEdge(b, Crop.RIGHT, 0.5, 0, 3, 2)
            compare(b.width, 0.25)
            b = Crop.setEdge(b, Crop.LEFT, 0.9, 0, 3, 2)
            near(b.width, Crop.MINIMUM, "the left margin cannot overlap with the right margin")
        }
    }
}
