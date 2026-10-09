.pragma library
// darktable's crop maths (src/iop/crop.c), shared by the frame on the photo (CropOverlay) and
// the Crop pane (GeometryPanel). Boxes are { x, y, width, height } in fractions of the photo.

// _grab_region_t as bits.
var LEFT = 1, TOP = 2, RIGHT = 4, BOTTOM = 8
// crop.c:46 MIN_CROP_SIZE
var MINIMUM = 0.01

// The aspect list of gui_init (crop.c:1267), sorted from most to least square as darktable
// sorts it; d = 1, n = 0 is the photo's own ratio.
var aspects = [
    ["freehand", 0, 0], ["original image", 1, 0], ["square", 1, 1], ["10:8 in print", 2445, 2032],
    ["5:4, 4x5, 8x10", 5, 4], ["11x14", 14, 11], ["45x35, portrait", 45, 35], ["8.5x11, letter", 110, 85],
    ["4:3, VGA, TV", 4, 3], ["5x7", 7, 5], ["ISO 216, DIN 476, A4", 14142136, 10000000],
    ["3:2, 4x6, 35mm", 3, 2], ["16:10, 8x5", 16, 10], ["golden cut", 16180340, 10000000],
    ["16:9, HDTV", 16, 9], ["widescreen", 185, 100], ["2:1, Univisium", 2, 1], ["CinemaScope", 235, 100],
    ["21:9", 237, 100], ["anamorphic", 239, 100], ["65:24, XPan", 65, 24], ["3:1, panorama", 300, 100]
].map(function (a) { return { name: a[0], d: a[1], n: a[2] } })

// _aspect_format (crop.c:1224): the name and, for a real ratio, its value.
function aspectName(a) {
    return a.n === 0 ? a.name : a.name + "  " + (a.d / a.n).toFixed(2)
}
function aspectIndex(d, n) {
    for (var i = 0; i < aspects.length; ++i)
        if (aspects[i].d === Math.abs(d) && aspects[i].n === n) return i
    return -1
}
// _aspect_ratio_get and the start of _aspect_apply: the wanted width over height in photo
// pixels (0: freehand). ratio_d's sign is the orientation: positive keeps the long side of the
// frame along the long side of the photo. imageAspect is the photo's width over height.
function ratio(d, n, imageAspect) {
    if (d === 0 && n === 0) return 0
    if (n < 0 || !(imageAspect > 0)) return 0
    var longOverShort = n === 0 ? Math.max(imageAspect, 1 / imageAspect) : Math.max(Math.abs(d), n) / Math.min(Math.abs(d), n)
    var landscape = (imageAspect >= 1) === (d > 0)
    return landscape ? longOverShort : 1 / longOverShort
}

// _aspect_apply (crop.c:771): bring a box to `aspect` (width over height in pixels of a photo
// iwd × iht) after the sides in `grab` moved.
function aspectApply(c, grab, aspect, iwd, iht) {
    if (!(aspect > 0) || !(iwd > 0) || !(iht > 0)) return c
    var x = Math.max(c.x, 0), y = Math.max(c.y, 0), w = Math.min(c.width, 1), h = Math.min(c.height, 1)
    var targetH = iwd * c.width / (iht * aspect), targetW = iht * c.height * aspect / iwd
    var off, ph, pw
    if (grab === (TOP | LEFT)) {
        x = x + w - (targetW + w) * .5; y = y + h - (targetH + h) * .5
        w = (targetW + w) * .5; h = (targetH + h) * .5
    } else if (grab === (TOP | RIGHT)) {
        y = y + h - (targetH + h) * .5
        w = (targetW + w) * .5; h = (targetH + h) * .5
    } else if (grab === (BOTTOM | RIGHT)) {
        w = (targetW + w) * .5; h = (targetH + h) * .5
    } else if (grab === (BOTTOM | LEFT)) {
        h = (targetH + h) * .5
        x = x + w - (targetW + w) * .5
        w = (targetW + w) * .5
    } else if (grab & (LEFT | RIGHT)) {
        off = targetH - h
        h += off; y -= .5 * off
    } else if (grab & (TOP | BOTTOM)) {
        off = targetW - w
        w += off; x -= .5 * off
    }
    if (x < 0) { ph = h; h *= (w + x) / w; w = w + x; x = 0; if (grab & TOP) y += ph - h }
    if (y < 0) { pw = w; w *= (h + y) / h; h = h + y; y = 0; if (grab & LEFT) x += pw - w }
    if (x + w > 1) { ph = h; h *= (1 - x) / w; w = 1 - x; if (grab & TOP) y += ph - h }
    if (y + h > 1) { pw = w; w *= (1 - y) / h; h = 1 - y; if (grab & LEFT) x += pw - w }
    x = Math.max(0, Math.min(1, x)); y = Math.max(0, Math.min(1, y))
    return { x: x, y: y, width: Math.max(0, Math.min(w, 1 - x)), height: Math.max(0, Math.min(h, 1 - y)) }
}

// The largest centred box of `aspect` inside c (choosing a ratio keeps the frame's centre).
function fit(c, aspect, iwd, iht) {
    if (!(aspect > 0) || !(iwd > 0) || !(iht > 0)) return c
    var r = aspect * iht / iwd
    var w = Math.min(c.width, c.height * r), h = w / r
    return { x: c.x + (c.width - w) / 2, y: c.y + (c.height - h) / 2, width: w, height: h }
}

// gui_changed (crop.c:1086): one margin slider moved; side is LEFT, TOP, RIGHT or BOTTOM and
// value the new edge position as a fraction from the left or top.
function setEdge(c, side, value, aspect, iwd, iht) {
    var x = c.x, y = c.y, w = c.width, h = c.height
    if (side === LEFT) { value = Math.max(0, Math.min(x + w - MINIMUM, value)); w = x + w - value; x = value }
    else if (side === RIGHT) { value = Math.max(x + MINIMUM, Math.min(1, value)); w = value - x }
    else if (side === TOP) { value = Math.max(0, Math.min(y + h - MINIMUM, value)); h = y + h - value; y = value }
    else if (side === BOTTOM) { value = Math.max(y + MINIMUM, Math.min(1, value)); h = value - y }
    return aspectApply({ x: x, y: y, width: w, height: h }, side, aspect, iwd, iht)
}
