#!/usr/bin/env python3
"""Build the Filters pane layout of every darktable module from the verified inventories.

Inputs
  omalux/design/ui-map/{tone,color,correct,effects}.json   darktable facts, one entry per module
  omalux/design/layout-decisions.json                       our own choices (groups, hiding, primaries)
  omalux/native/engine/controls.h                           curated rows, which stay where they are

Outputs
  omalux/design/layout.json            groups -> modules -> rows, consumed by the QML integration,
                                       and panes -> summary rows and Advanced order (Tone, Color,
                                       Detail, Effects)
  omalux/design/layout-blending.json   the per-module blend section, same row format, for later

Conventions of the row "path":
  "name", "name[i]", "name.member"   a darktable params member the engine can write directly
  "name[@sel]"                         index chosen in the GUI by the row whose field is "@sel"
                                       ("@tab" = the active tab of the module)
  "@name"                              no direct params member: a displayed conversion that needs an
                                       adapter (like "@temperature" in controls.h)
  null                                 GUI-only state or an action (buttons, pickers, notices)

Run from anywhere:  python3 development/tools/darktable/build_layout.py
Standard library only.
"""

import json
import re
import sys
from collections import Counter, OrderedDict
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
DESIGN = ROOT / "omalux" / "design"
UI_MAP = DESIGN / "ui-map"
FAMILIES = ["tone", "color", "correct", "effects"]
CONTROLS_H = ROOT / "omalux" / "native" / "engine" / "controls.h"
GROUP_IDS = ["base", "tone", "color", "correct", "effect", "technical", "deprecated"]
WIDGETS = {"slider", "combobox", "toggle", "button", "curve", "graph", "color", "picker",
           "drawn", "text", "file", "notice", "choice", "patches", "canvas"}
CUSTOM_KINDS = {"curve", "graph", "color", "picker", "drawn", "file", "text", "choice", "patches", "canvas"}
INTERPOLATIONS = {None, "cubic", "catmull", "monotone", "linear"}
TIERS = {"primary", "detail", "advanced"}
COLORS = {"", "light", "saturation", "hue", "temperature", "tint", "green-magenta", "blue-yellow"}
VALUE_WIDGETS = {"slider", "combobox", "toggle", "color", "file", "text", "curve", "drawn"}

# ---------------------------------------------------------------------------------------------
# darktable facts the inventories give only as prose. Each entry cites the pinned source.
# ---------------------------------------------------------------------------------------------

# Irregular inventory fields -> (field id, params path). Keyed by (operation, inventory field).
FIELD_FIXES = {
    ("basecurve", "basecurve[0]"): ("basecurve", "basecurve[0]"),
    ("toneequal", "noise..speculars"): ("@graph", None),
    ("lowlight", "transition_x/transition_y"): ("transition", None),
    ("colorzones", "curve[channel][*]"): ("curve", "curve"),
    ("colorzones", "curve_type[channel]"): ("curve_type", "curve_type[@tab]"),
    ("colorcorrection", "hia/hib/loa/lob"): ("grid", None),
    ("monochrome", "a/b/size"): ("grid", None),
    ("colorchecker", "source_*/target_*"): ("patches", None),
    # The sliders show the target relative to the source patch or absolute, after darktable's
    # "target color" combobox (colorchecker.c:1009-1044); module_values.c converts both.
    ("colorchecker", "target_L[patch]"): ("target_L", "@target_L[@absolute_target][@patch]"),
    ("colorchecker", "target_a[patch]"): ("target_a", "@target_a[@absolute_target][@patch]"),
    ("colorchecker", "target_b[patch]"): ("target_b", "@target_b[@absolute_target][@patch]"),
    ("colorchecker", "@target_C[patch]"): ("@target_C", "@target_C[@absolute_target][@patch]"),
    ("channelmixer", "red[destination]"): ("red", "red[@destination]"),
    ("channelmixer", "green[destination]"): ("green", "green[@destination]"),
    ("channelmixer", "blue[destination]"): ("blue", "blue[@destination]"),
    ("denoiseprofile", "y[0..3][]"): ("y_rgb", "y"),
    ("denoiseprofile", "y[4..5][]"): ("y_yuv", "y"),
    ("denoiseprofile", "a,b"): ("@profile", None),
    ("rawdenoise", "y[0..3][]"): ("y", "y"),
    ("atrous", "y[0][]"): ("y_luma", "y[0]"),
    ("atrous", "y[1][]"): ("y_chroma", "y[1]"),
    ("atrous", "y[2][]"): ("y_edges", "y[2]"),
    ("crop", "ratio_n/ratio_d"): ("aspect", "@aspect"),
    ("clipping", "ratio_n/ratio_d"): ("aspect", "@aspect"),
    ("clipping", "cw/ch sign"): ("flip", "@flip"),
    ("retouch", "num_scales / merge_from_scale / curr_scale"): ("wavelet_decompose", None),
    ("colorharmonizer", "anchor_hue"): ("anchor_hue", "@anchor_hue"),  # RYB<->UCS, colorharmonizer.c:855
    ("negadoctor", "Dmin"): ("Dmin", "Dmin"),
    # blend section (blend.h:180-222)
    ("@blending", "blend_mode & DEVELOP_BLEND_REVERSE"): ("blend_reverse", "@blend_mode.reverse"),
    ("@blending", "mask_combine & DEVELOP_COMBINE_INV (drawn polarity)"): ("drawn_polarity", "@mask_combine.drawn_inv"),
    ("@blending", "raster_mask_source / raster_mask_instance / raster_mask_id"): ("raster_mask", None),
    ("@blending", "blendif_parameters[4*ch+0..3] (output channel ch)"): ("blendif_output", "blendif_parameters"),
    ("@blending", "blendif_parameters[4*ch+0..3] (input channel ch)"): ("blendif_input", "blendif_parameters"),
    ("@blending", "blendif bit (ch+16)"): ("blendif_polarity", "@blendif.polarity"),
    ("@blending", "blendif_boost_factors[ch]"): ("boost_factor", "blendif_boost_factors[@tab]"),
    ("@blending", "mask_combine & (DEVELOP_COMBINE_INV|DEVELOP_COMBINE_INCL)"): ("mask_combine", "@mask_combine"),
}
for _i in range(4):
    FIELD_FIXES[("colorharmonizer", f"custom_hue[{_i}]")] = (f"custom_hue_{_i}", f"@custom_hue[{_i}]")

# Custom widget wiring, keyed by (operation, field id). Struct members from the params typedefs.
CUSTOM = {
    ("basecurve", "basecurve"): dict(kind="curve", fields=["basecurve", "basecurve_nodes", "basecurve_type"],
        nodes_field="basecurve[0]", count_field="basecurve_nodes[0]", interpolation="monotone",
        channels=[], periodic=False),  # basecurve.c:63-65, MAXNODES 20, type from basecurve_type[0]
    ("tonecurve", "tonecurve"): dict(kind="curve", fields=["tonecurve", "tonecurve_nodes", "tonecurve_type"],
        nodes_field="tonecurve", count_field="tonecurve_nodes", interpolation="monotone",
        channels=["L", "a", "b"], periodic=False),  # tonecurve.c:99-101, 20 nodes per channel
    ("rgbcurve", "curve_nodes"): dict(kind="curve", fields=["curve_nodes", "curve_num_nodes", "curve_type"],
        nodes_field="curve_nodes", count_field="curve_num_nodes", interpolation="monotone",
        channels=["R", "G", "B"], periodic=False),  # rgbcurve.c, MAX_ANCHORS 20
    ("colorzones", "curve"): dict(kind="curve", fields=["curve", "curve_num_nodes", "curve_type"],
        nodes_field="curve", count_field="curve_num_nodes", interpolation="catmull",
        channels=["lightness", "chroma", "hue"], periodic=True),  # colorzones.c:74-78, default CATMULL_ROM :834
    ("lowlight", "transition"): dict(kind="curve", fields=["transition_x", "transition_y"],
        nodes_field="transition_y", count_field=None, interpolation="catmull",
        channels=[], periodic=False),  # lowlight.c:791, 6 fixed bands
    ("denoiseprofile", "y_rgb"): dict(kind="curve", fields=["x", "y"], nodes_field="y", count_field=None,
        interpolation="catmull", channels=["all", "R", "G", "B"], periodic=False),  # denoiseprofile.c:3617, 7 bands
    ("denoiseprofile", "y_yuv"): dict(kind="curve", fields=["x", "y"], nodes_field="y", count_field=None,
        interpolation="catmull", channels=["Y0", "U0V0"], periodic=False),
    ("rawdenoise", "y"): dict(kind="curve", fields=["x", "y"], nodes_field="y", count_field=None,
        interpolation="catmull", channels=["all", "R", "G", "B"], periodic=False),  # rawdenoise.c:894, 5 bands
    ("atrous", "y_luma"): dict(kind="curve", fields=["x[0]", "y[0]", "y[3]"], nodes_field="y[0]", count_field=None,
        interpolation="catmull", channels=["luma"], periodic=False),  # atrous.c:1749, 6 bands, y[3] = noise threshold
    ("atrous", "y_chroma"): dict(kind="curve", fields=["x[1]", "y[1]", "y[4]"], nodes_field="y[1]", count_field=None,
        interpolation="catmull", channels=["chroma"], periodic=False),
    ("atrous", "y_edges"): dict(kind="curve", fields=["x[2]", "y[2]"], nodes_field="y[2]", count_field=None,
        interpolation="catmull", channels=["edges"], periodic=False),
    ("toneequal", "@graph"): dict(kind="curve", fields=["noise", "ultra_deep_blacks", "deep_blacks", "blacks",
        "shadows", "midtones", "highlights", "whites", "speculars", "smoothing"], nodes_field=None, count_field=None,
        interpolation=None, channels=[], periodic=False),  # gaussian RBF over the nine bands, toneequal.c
    ("rgblevels", "levels"): dict(kind="graph", fields=["levels"], channels=["R", "G", "B"]),
    ("levels", "levels"): dict(kind="graph", fields=["levels"]),
    ("zonesystem", "zone"): dict(kind="drawn", fields=["size", "zone"]),
    ("zonesystem", "size"): dict(kind="drawn", fields=["size"]),
    ("colorcorrection", "grid"): dict(kind="drawn", fields=["hia", "hib", "loa", "lob", "saturation"]),
    ("monochrome", "grid"): dict(kind="drawn", fields=["a", "b", "size"]),
    ("colorchecker", "patches"): dict(kind="patches", fields=["source_L", "source_a", "source_b",
        "target_L", "target_a", "target_b", "num_patches"]),  # checker_draw/_button_press, colorchecker.c:1293-1479
    ("negadoctor", "Dmin"): dict(kind="color", fields=["Dmin[0]", "Dmin[1]", "Dmin[2]"]),
    ("negadoctor", "wb_low"): dict(kind="color", fields=["wb_low[0]", "wb_low[1]", "wb_low[2]"]),
    ("negadoctor", "wb_high"): dict(kind="color", fields=["wb_high[0]", "wb_high[1]", "wb_high[2]"]),
    ("invert", "color"): dict(kind="color", fields=["color"]),
    ("lut3d", "filepath"): dict(kind="file", fields=["filepath"]),
    ("lut3d", "lutname"): dict(kind="text", fields=["lutname"]),
    ("watermark", "filename"): dict(kind="file", fields=["filename"]),
    ("overlay", "imgid"): dict(kind="file", fields=["imgid", "filename"]),
    ("rasterfile", "file"): dict(kind="file", fields=["path", "file"]),
    ("@blending", "blendif_output"): dict(kind="graph", fields=["blendif_parameters"]),
    ("@blending", "blendif_input"): dict(kind="graph", fields=["blendif_parameters"]),
}

# darktable's own drawn widgets that edit one float over a fixed range: shown as a slider row.
# (operation, field) -> (min, max, digits, reason)
GRADIENT_SLIDERS = {
    # relight.c:254-257: dtgtk gradient slider from black to neutral grey over 0..1 (the
    # gradient slider's own range, dtgtk/gradientslider.c); darktable prints no number.
    ("relight", "center"): (0.0, 1.0, 2, "darktable's black-to-grey gradient slider (relight.c:257)"),
}

# Comboboxes whose entries darktable fills at runtime: shown as a text/file selector instead.
DYNAMIC = {
    ("colorin", "type"): ("text", ["type", "filename"]),        # colorin.c, profiles per image + ICC files
    ("colorin", "type_work"): ("text", ["type_work", "filename_work"]),
    ("colorout", "type"): ("text", ["type", "filename"]),       # colorout.c, out_pos profiles + ICC files
    ("denoiseprofile", "@profile"): ("text", ["a", "b"]),       # noise profile database, denoiseprofile.c:3677
    ("lens", "focal"): ("text", ["focal"]),                     # editable comboboxes, lens.cc:4406
    ("lens", "aperture"): ("text", ["aperture"]),
    ("lens", "distance"): ("text", ["distance"]),
    ("@blending", "mask_id"): ("text", ["mask_id"]),
    ("@blending", "raster_mask"): ("text", ["raster_mask_source", "raster_mask_instance", "raster_mask_id"]),
    ("colorchecker", "@patch"): ("text", ["num_patches"]),      # 'patch #k' for k < num_patches, colorchecker.c
}

# Rows darktable fills from a runtime list or a file dialog, and how Omalux offers them
# (module_choices.c): "list" names the list om_engine_module_choices answers, "browse" the
# "@" path a file from the dialog is written to, "filters" the dialog's name filters.
# Keyed by (operation, field id); the row's own path becomes the field id where it had none.
CHOICES = {
    ("colorin", "type"): dict(list="type"),                       # colorin.c:1919-2028
    ("colorin", "type_work"): dict(list="type_work"),
    ("colorout", "type"): dict(list="type"),                      # colorout.c:860
    ("denoiseprofile", "@profile"): dict(list="@profile"),        # denoiseprofile.c:2719-2733
    ("lens", "camera"): dict(list="camera", search=True),         # lens.cc:3764-3824
    ("lens", "lens"): dict(list="lens", search=True),             # lens.cc:4101-4180
    ("lens", "focal"): dict(list="focal", search=True),           # editable comboboxes, lens.cc:4045-4098
    ("lens", "aperture"): dict(list="aperture", search=True),
    ("lens", "distance"): dict(list="distance", search=True),
    ("lut3d", "filepath"): dict(list="filepath", search=True, browse="@lut_file",   # lut3d.c:1570-1675
                                filters=["LUT files (*.png *.PNG *.cube *.CUBE *.3dl *.3DL *.gmz *.GMZ)", "All files (*)"]),
    ("lut3d", "lutname"): dict(list="lutname", search=True),     # the LUTs of a .gmz file, lut3d.c:1714-1745
    ("watermark", "filename"): dict(list="filename"),             # watermark.c:1142-1207
    ("rasterfile", "file"): dict(list="file", browse="@raster_file",                 # rasterfile.c:322-424
                                 filters=["raster masks (*.pfm *.PFM *.png *.PNG)"]),
    ("overlay", "imgid"): dict(list=None, browse="@overlay_file",                    # overlay.c:989-1047
                               filters=["images (*.jpg *.jpeg *.JPG *.JPEG *.png *.PNG *.tif *.tiff *.TIF *.TIFF "
                                        "*.exr *.EXR *.webp *.WEBP *.avif *.AVIF *.heic *.HEIC *.jxl *.JXL)",
                                        "All files (*)"]),
    ("temperature", "preset"): dict(list="preset"),               # temperature.c:740-806, 1662-1678
}
# Free text darktable edits in an entry: written back as the string parameter.
TEXT_EDIT = {("watermark", "text"), ("watermark", "font")}  # watermark.c _text_callback, _fontsel_callback

# Comboboxes the inventory describes by reference to another one: (operation, field) -> (operation, field).
VALUES_FROM = {
}

# Combobox entries that are positions in a list darktable builds in its GUI, written through an
# "@" conversion (values_clipping.c). The deprecated crop and rotate keeps its own aspect list
# (clipping.c:2124-2143): special entries first, then sorted square to wide (_aspect_ratio_cmp),
# labelled "name  d/n" with two decimals (format_aspect, 2062).
_CLIPPING_ASPECTS = [("freehand", 0, 0), ("original image", 1, 0), ("square", 1, 1),
                     ("10:8 in print", 2445, 2032), ("5:4, 4x5, 8x10", 5, 4), ("11x14", 14, 11),
                     ("8.5x11, letter", 110, 85), ("4:3, VGA, TV", 4, 3), ("5x7", 7, 5),
                     ("ISO 216, DIN 476, A4", 14142136, 10000000), ("3:2, 4x6, 35mm", 3, 2),
                     ("16:10, 8x5", 16, 10), ("golden cut", 16180340, 10000000), ("16:9, HDTV", 16, 9),
                     ("widescreen", 185, 100), ("2:1, univisium", 2, 1), ("cinemascope", 235, 100),
                     ("21:9", 237, 100), ("anamorphic", 239, 100), ("3:1, panorama", 300, 100)]
INDEXED_VALUES = {
    ("clipping", "aspect"): [OrderedDict(value=i, label=name if n == 0 else f"{name}  {d / n:4.2f}")
                             for i, (name, d, n) in enumerate(_CLIPPING_ASPECTS)],
}

# Values the inventory gives as prose. (value, reason)
DEFAULT_FIXES = {
    ("channelmixer", "red"): (1, "red destination is the default selection (channelmixer.c:596)"),
    ("channelmixer", "green"): (0, "red destination is the default selection"),
    ("channelmixer", "blue"): (0, "red destination is the default selection"),
    ("colorchecker", "target_L"): (0, "relative target mode shows the offset, 0 by default"),
    ("@blending", "blend_cst"): (0, "0 = the module's default colorspace"),
    ("denoiseprofile", "compensate_hilite_pres"): (1, "$DEFAULT: TRUE in the params struct"),
    ("ashift", "@auto"): (0, "GUI toggle, off until structure detection runs"),
    ("lens", "@use_latest_algorithm"): (0, "GUI check button, unchecked; only offered for old edits"),
    ("borders", "aspect:aspect_ratio"): (-1.0, "-1 means constant border (DT_IOP_BORDERS_ASPECT_CONSTANT_VALUE, borders.c:49), "
                                        "outside the slider range"),
}

# Clauses of the blend section's free-text conditions (blend.h:93-101 mask_mode bits).
BLEND_MASK_BITS = {"drawn": 2, "parametric": 4, "raster": 8}

# ---------------------------------------------------------------------------------------------


def fail(msg):
    print("error: " + msg, file=sys.stderr)
    sys.exit(1)


def slug(text):
    text = text.strip()
    if text.startswith("(") and text.endswith(")"):
        text = text[1:-1]
    return re.sub(r"[^a-z0-9]+", "_", text.lower()).strip("_")


def ident(text):
    return re.sub(r"[^A-Za-z0-9_@]+", "_", text).strip("_")


def number(value):
    if isinstance(value, bool):
        return int(value)
    if isinstance(value, (int, float)):
        return value
    if isinstance(value, str):
        m = re.match(r"\s*(-?\d+(?:\.\d+)?)", value)
        if m:
            v = float(m.group(1))
            return int(v) if v.is_integer() else v
    return None


def load_inventory():
    modules, blending = OrderedDict(), None
    for fam in FAMILIES:
        data = json.loads((UI_MAP / f"{fam}.json").read_text())
        if data.get("darktable") != "5.6.1":
            fail(f"{fam}.json is for darktable {data.get('darktable')}")
        for m in data["modules"]:
            if m["operation"] == "@blending":
                blending = m
            elif m["operation"] in modules:
                fail("duplicate module " + m["operation"])
            else:
                modules[m["operation"]] = m
    return modules, blending


def load_curated():
    """(module, params member) pairs registered in controls.h, plus the modules it touches."""
    rows = re.findall(r'\{"([^"]+)",\s*"[^"]*",\s*"([^"]+)",\s*"([^"]+)"', CONTROLS_H.read_text())
    pairs, modules = set(), set()
    for _cid, module, param in rows:
        if param == "@enabled":
            continue
        modules.add(module)
        if param.startswith("@int:"):
            param = param[5:]
        if re.fullmatch(r"@curve(2[89]|3\d|4[01])", param):  # denoise Y0/U0V0 ordinates
            pairs.add((module, "@curve_yuv"))
            continue
        pairs.add((module, param))
    return pairs, modules


# ---------------------------------------------------------------------------------------------
# rows

INVENTORY = {}


def base_rows(op, inv_controls, notes):
    """Inventory controls -> rows with field ids, paths and normalised values (order preserved)."""
    rows = []
    for c in inv_controls:
        f = c["field"]
        label = c["label"] or ""
        if (op, f) in FIELD_FIXES:
            field, path = FIELD_FIXES[(op, f)]
        elif f is None:
            field, path = "@" + slug(label), None
        elif f.startswith("@"):
            name = ident(re.sub(r"\s*\(.*$", "", f))
            field, path = name, name
        else:
            field = ident(f)
            path = f if re.fullmatch(r"[A-Za-z_]\w*(\[\d+\])?(\.\w+)?", f) else None
            if path is None and c["widget"] not in ("button", "picker"):
                notes.append(f"{label}: inventory field '{f}' has no direct params path")
        widget = c["widget"]
        values = c["values"]
        # A radio-button row that stores a value is a choice: present it as a combobox.
        if widget == "button" and values and all(isinstance(v["value"], (int, float)) for v in values):
            widget = "combobox"
        if widget in ("button", "picker"):
            path = None  # actions, not values
        if label.startswith("(") and label.endswith(")"):
            notes.append(f"unlabelled in darktable: {label[1:-1]}")
            label = ""
        if (op, field) in VALUES_FROM and not values:
            src_op, src_field = VALUES_FROM[(op, field)]
            src = [x for x in INVENTORY[src_op]["controls"]
                   if FIELD_FIXES.get((src_op, x["field"]), (x["field"],))[0] == src_field]
            values = src[0]["values"]
            notes.append(f"{label}: values taken from {src_op} '{src_field}'")
        rows.append(dict(_inv=c, _f=f, field=field, path=path, label=label, widget=widget, values=values))
    # unique field ids: duplicates get ':<label>' (or ':<condition>') except the first value widget
    counts = Counter(r["field"] for r in rows)
    for field, n in counts.items():
        if n < 2:
            continue
        group = [r for r in rows if r["field"] == field]
        keep = next((r for r in group if r["widget"] in VALUE_WIDGETS), None)
        taken = {field}
        same_label = len({r["_inv"]["label"] for r in group}) == 1
        for r in group:
            if r is keep:
                continue
            for suffix in ("" if same_label else slug(r["_inv"]["label"] or ""), slug(str(r["_inv"]["visible_when"] or "")), str(group.index(r))):
                cand = f"{field}:{suffix}"
                if suffix and cand not in taken:
                    break
            r["field"] = cand
            taken.add(cand)
    return rows


def finish_row(op, r, notes):
    c = r["_inv"]
    widget = r["widget"]
    out = OrderedDict()
    out["field"] = r["field"]
    out["path"] = r["path"]
    out["label"] = r["label"]
    out["tab"] = c["tab"]
    out["section"] = c["section"]
    custom = None
    if (op, r["field"]) in DYNAMIC:
        widget, fields = DYNAMIC[(op, r["field"])]
        custom = dict(kind=widget, fields=fields, dynamic=True)
    gradient = GRADIENT_SLIDERS.get((op, r["field"]))
    if gradient:
        widget = "slider"
        c = dict(c, min=gradient[0], max=gradient[1], soft_min=gradient[0], soft_max=gradient[1],
                 digits=gradient[2], factor=1, offset=0, unit="")
        notes.append(f"{r['label']}: {gradient[3]}")
    out["widget"] = widget
    is_slider = widget == "slider"
    out["unit"] = c["unit"] or ""
    out["factor"] = c["factor"] if c["factor"] is not None else 1
    out["offset"] = c["offset"] if c["offset"] is not None else 0
    out["digits"] = c["digits"] if c["digits"] is not None else 0
    for k in ("min", "max", "soft_min", "soft_max"):
        out[k] = c[k] if is_slider or isinstance(c[k], (int, float)) else None
    if not is_slider:
        out["soft_min"] = out["soft_max"] = None
    default = c["default"]
    if (op, r["field"]) in DEFAULT_FIXES:
        default, why = DEFAULT_FIXES[(op, r["field"])]
        notes.append(f"{r['label'] or r['field']}: default {default} ({why})")
    elif isinstance(default, bool):
        default = int(default)
    elif isinstance(default, str) and widget in ("slider", "toggle", "combobox"):
        n = number(default)
        notes.append(f"{r['label'] or r['field']}: darktable's default is '{default}'")
        if n is None and widget == "toggle":
            n = 1 if "TRUE" in default else 0
        if n is None and widget == "slider":
            n = 0  # introspection zero-initialises a member without $DEFAULT
        if n is None and widget == "combobox" and r["values"]:
            n = r["values"][0]["value"]
        default = n
    if widget == "combobox" and isinstance(default, str) and r["values"]:
        hit = [v["value"] for v in r["values"] if v["label"] == default]
        if hit:
            default = hit[0]
    out["default"] = default
    out["values"] = [OrderedDict(value=v["value"], label=v["label"]) for v in r["values"]] \
        if widget == "combobox" and r["values"] else None
    if (op, r["field"]) in INDEXED_VALUES:
        out["values"] = INDEXED_VALUES[(op, r["field"])]
        out["default"] = 0
    out["visible_when"] = None
    out["tier"] = c["tier"]
    out["colors"] = ""
    if custom is None and widget in CUSTOM_KINDS:
        spec = CUSTOM.get((op, r["field"]))
        if spec:
            custom = dict(spec)
            out["widget"] = widget = spec["kind"] if widget == "drawn" else widget
        else:
            custom = dict(kind=widget, fields=[r["path"]] if r["path"] and not r["path"].startswith("@") else [])
    if custom is not None:
        full = OrderedDict(kind=custom["kind"], fields=custom.get("fields", []))
        if full["kind"] == "curve":
            full["interpolation"] = custom.get("interpolation")
            full["nodes_field"] = custom.get("nodes_field")
            full["count_field"] = custom.get("count_field")
            full["channels"] = custom.get("channels", [])
            full["periodic"] = custom.get("periodic", False)
        elif custom.get("channels"):
            full["channels"] = custom["channels"]
        if custom.get("dynamic"):
            full["dynamic"] = True
        custom = full
    if (op, r["field"]) in CHOICES:
        spec = CHOICES[(op, r["field"])]
        out["widget"] = "choice"
        if out["path"] is None and not r["field"].startswith("@"):
            out["path"] = r["field"]
        out["values"] = None
        custom = OrderedDict(kind="choice", fields=(custom or {}).get("fields", []), list=spec.get("list"),
                             search=spec.get("search", False), browse=spec.get("browse"),
                             filters=spec.get("filters", []))
    if (op, r["field"]) in TEXT_EDIT and custom is not None:
        custom["editable"] = True
    out["custom"] = custom
    out["action"] = c["action"]
    return out


def notice_row(label):
    return OrderedDict(field="@notice", path=None, label=label, tab=None, section=None, widget="notice",
                       unit="", factor=1, offset=0, digits=0, min=None, max=None, soft_min=None,
                       soft_max=None, default=None, values=None, visible_when=None, tier="primary",
                       colors="", custom=None, action=None)


def canvas_row(entry):
    """A row that opens a module's tool on the photograph (layout-decisions "canvas")."""
    return OrderedDict(field="@canvas", path=None, label=entry["label"], tab=None, section=entry.get("section"),
                       widget="canvas", unit="", factor=1, offset=0, digits=0, min=None, max=None,
                       soft_min=None, soft_max=None, default=None, values=None, visible_when=None,
                       tier="detail", colors="", custom=OrderedDict(kind="canvas", tool=entry["tool"],
                                                                    hint=entry["hint"]), action=None)


# ---------------------------------------------------------------------------------------------
# visible_when: darktable prose -> {"field","in"} / {"all": [...]}

LAYOUT_WORDS = ("collapsible", "tab ", "'options' tab", "preference", "conf ", "button bar", "shown",
                "sensitive", "never", "menu behind")
IMAGE_WORDS = ("sensor", "bayer", "x-trans", "raw", "image", "file has", "exif", "monochrome", "camera",
               "g'mic", "4-color", "md_version", "sraw", "gainmaps", "selected camera preset", "shape is selected",
               "selected tool", "rawprepare_supported", "dual method", "built with")


def _row_for(token, rows):
    t = token.strip().lower()
    for pref in (("combobox", "toggle"), ("slider",), None):
        for r in rows:
            if pref and r["widget"] not in pref:
                continue
            if r["label"].lower() == t or r["field"].lower() == t or (r["path"] or "").lower() == t \
                    or (r["_f"] or "").lower() == t:
                return r
    return None


def _match_values(row, words):
    if row["widget"] == "toggle":
        out = []
        for w in words:
            w = w.strip().lower()
            if w in ("on", "true", "yes", "1"):
                out.append(1)
            elif w in ("off", "false", "no", "0"):
                out.append(0)
            else:
                return None
        return out
    values = row["values"] or []
    out = []
    for w in words:
        w = re.sub(r"\s*\(.*?\)\s*$", "", w.strip()).strip().lower()
        hit = [v["value"] for v in values if v["label"].lower() == w] \
            or [v["value"] for v in values if number(w) is not None and v["value"] == number(w)] \
            or [v["value"] for v in values if v["label"].lower().startswith(w)] \
            or [v["value"] for v in values if re.sub(r"\s*\(.*?\)", "", v["label"]).lower() == w]
        if not hit and w in ("lens", "motion", "gaussian"):
            hit = [v["value"] for v in values if v["label"] == w]
        if not hit:
            return None
        out.extend(h for h in hit if h not in out)
    return out


def _clause(text, rows, op):
    t = text.strip()
    t = re.sub(r"\s*\((?:[a-z_]+\.c[c]?)?:?\d[^)]*\)\s*", " ", t).strip()  # source refs
    t = re.sub(r"\s*\(widget made insensitive.*\)$", "", t).strip()
    low = t.lower()
    if op == "@blending":
        m = re.match(r"mask_mode (has|!=) (\w+)", low)
        if m:
            modes = [v["value"] for v in _row_for("mask_mode", rows)["values"]]
            if m.group(1) == "!=":
                return {"field": "mask_mode", "in": [v for v in modes if v != 0]}
            bits = [b for name, b in BLEND_MASK_BITS.items() if name in low[m.start(2):]]
            if bits:
                return {"field": "mask_mode", "in": [v for v in modes if any(v & b for b in bits)]}
        if low.startswith("refine box shown"):
            return {"field": "mask_mode", "in": [v for v in (3, 5, 7, 9)]}
    m = re.match(r"(.+?)\s*(==|!=)\s*(.+)$", t)
    m_in = re.match(r"(.+?)\s+(not in|in)\s*[({](.+)[)}]$", t)
    m_gt = re.match(r"(.+?)\s*>\s*(\d+)$", t)
    if m_in:
        row = _row_for(m_in.group(1), rows)
        if row:
            vals = _match_values(row, re.split(r",\s*", m_in.group(3)))
            if vals is not None:
                if m_in.group(2) == "not in":
                    vals = [v["value"] for v in row["values"] if v["value"] not in vals]
                return {"field": row["field"], "in": vals}
    if m and not m_in:
        row = _row_for(m.group(1), rows)
        rhs = m.group(3).strip()
        if row:
            words = [w for w in re.split(r",\s*", rhs)] if row["widget"] == "combobox" and \
                _match_values(row, [rhs]) is None else [rhs]
            vals = _match_values(row, words)
            if vals is not None:
                if m.group(2) == "!=":
                    pool = [0, 1] if row["widget"] == "toggle" else [v["value"] for v in row["values"]]
                    vals = [v for v in pool if v not in vals]
                return {"field": row["field"], "in": vals}
    if m_gt:
        row = _row_for(m_gt.group(1), rows)
        if row and row["widget"] == "slider" and row["_inv"]["digits"] == 0:
            c = row["_inv"]
            lo, hi = int(c["min"]), int(c["max"])
            return {"field": row["field"], "in": list(range(max(lo, int(m_gt.group(2)) + 1), hi + 1))}
    m_not = re.match(r"(?:not |!)(.+)$", t)
    if m_not:
        row = _row_for(m_not.group(1), rows)
        if row and row["widget"] == "toggle":
            return {"field": row["field"], "in": [0]}
    row = _row_for(t, rows)
    if row and row["widget"] == "toggle":
        return {"field": row["field"], "in": [1]}
    m_on = re.match(r"(.+?)\s+(on|off)$", t)
    if m_on:
        row = _row_for(m_on.group(1), rows)
        if row and row["widget"] == "toggle":
            return {"field": row["field"], "in": [1 if m_on.group(2) == "on" else 0]}
    m_mode = re.match(r"mode (not auto)$", t)
    if m_mode:
        row = _row_for("mode", rows)
        if row:
            return {"field": row["field"], "in": [v["value"] for v in row["values"] if "auto" not in v["label"]]}
    m_wav = re.match(r"(.+?) in (wavelet modes)$", t)
    if m_wav:
        row = _row_for(m_wav.group(1), rows)
        if row:
            return {"field": row["field"], "in": [v["value"] for v in row["values"] if "wavelets" in v["label"]]}
    return None


def convert_visible(op, text, rows, layout_notes, dropped):
    if not text:
        return None
    text = re.sub(r"\s*\([^()]*\.cc?[:)][^()]*\)", "", text)
    text = re.sub(r"\s*\(:\d[^()]*\)", "", text)
    if "||" in text:
        dropped.append(text.strip())
        return None
    clauses = re.split(r"\s*(?:&&|;| and | AND )\s*", text)
    out = []
    for cl in clauses:
        if not cl.strip():
            continue
        conv = _clause(cl, rows, op)
        if conv:
            out.append(conv)
            continue
        low = cl.lower()
        if any(w in low for w in LAYOUT_WORDS):
            layout_notes.append(cl.strip())
        else:
            dropped.append(cl.strip())
    if not out:
        return None
    return out[0] if len(out) == 1 else {"all": out}


# ---------------------------------------------------------------------------------------------
# colours

def derive_color(row):
    if row["widget"] != "slider":
        return ""
    label = row["label"].lower()
    unit = row["unit"].strip()
    lo = (row["min"] or 0) * row["factor"]
    hi = (row["max"] or 0) * row["factor"]
    if unit == "K":
        return "temperature"
    if label == "tint":
        return "tint"
    if unit == "°" and "hue" in label and min(lo, hi) >= -0.5 and max(lo, hi) >= 300:
        return "hue"
    if re.search(r"\b(saturation|chroma|vibrance|colorfulness|purity)\b", label) and "threshold" not in label:
        return "saturation"
    if re.search(r"^(exposure|brightness|lightness|luminance|black level correction|print exposure adjustment)$",
                 label):
        return "light"
    return ""


# ---------------------------------------------------------------------------------------------

def build_module(op, inv, dec, curated_pairs, curated_modules, report):
    notes = []
    curated = op in curated_modules
    rows = base_rows(op, inv["controls"], notes)
    # GUI order. The inventory lists rows in the order darktable packs them, which is not always
    # source order (exposure packs its mode combobox above a stack built earlier). Rows are only
    # grouped by tab here; a row without a tab stays next to its neighbours.
    tabs = inv["tabs"] or []
    tab_index = {t: i for i, t in enumerate(tabs)}
    current = 0
    for i, r in enumerate(rows):
        t = r["_inv"]["tab"]
        if t in tab_index:
            current = tab_index[t]
        r["_key"] = (current, i)
    before = [r["field"] for r in rows]
    rows.sort(key=lambda r: r["_key"])
    if [r["field"] for r in rows] != before:
        report["reordered"].append(op)

    # omitted rows (our choice) and curated rows (already in controls.h)
    omit = dec["omit_rows"].get(op, {})
    for f in omit:
        if not any(r["field"] == f for r in rows):
            fail(f"layout-decisions: omit_rows {op}.{f} matches no row")
    kept, curated_rows = [], []
    for r in rows:
        if r["field"] in omit:
            continue
        if curated:
            key = r["path"] or r["field"]
            if (op, key) in curated_pairs or (op, r["field"]) in curated_pairs or \
                    (op == "denoiseprofile" and r["field"] == "y_yuv" and (op, "@curve_yuv") in curated_pairs):
                curated_rows.append(r)
                continue
        kept.append(r)
    rows = kept

    # notices replace drawn/on-canvas rows
    notice = dec["notices"].get(op)
    if notice:
        for f in notice["replaces"]:
            if not any(r["field"] == f for r in rows):
                fail(f"layout-decisions: notice of {op} replaces unknown row {f}")
        first = next((i for i, r in enumerate(rows) if r["field"] in notice["replaces"]), 0)
        rows = [r for r in rows if r["field"] not in notice["replaces"]]
        rows.insert(min(first, len(rows)), {"_notice": notice["label"]})
        report["notices"].append(op)

    # on-canvas tools replace the rows they draw (and take the place of the first of them)
    canvas = dec.get("canvas", {}).get(op)
    if canvas:
        for f in canvas["replaces"]:
            if not any(r.get("field") == f for r in rows):
                fail(f"layout-decisions: canvas of {op} replaces unknown row {f}")
        first = next((i for i, r in enumerate(rows) if r.get("field") in canvas["replaces"]), 0)
        rows = [r for r in rows if r.get("field") not in canvas["replaces"]]
        rows.insert(min(first, len(rows)), {"_canvas": canvas})
        report["canvas"].append(op)

    # finish rows
    layout_notes, dropped = [], []
    vw_dec = dec["visible_when"].get(op)
    # conditions may name a curated parameter: it is referred to by its params member
    live = [r for r in rows if "_notice" not in r and "_canvas" not in r] + \
        [dict(r, field=r["path"] or r["field"]) for r in curated_rows]
    out_rows = []
    for r in rows:
        if "_notice" in r:
            out_rows.append(notice_row(r["_notice"]))
            continue
        if "_canvas" in r:
            out_rows.append(canvas_row(r["_canvas"]))
            continue
        row = finish_row(op, r, notes)
        text = r["_inv"]["visible_when"]
        if vw_dec and text and vw_dec["match"] in text:
            row["visible_when"] = vw_dec["value"]
        else:
            drop_before = len(dropped)
            lay_before = len(layout_notes)
            row["visible_when"] = convert_visible(op, text, live, layout_notes, dropped)
            if len(dropped) > drop_before:
                report["vw_partial" if row["visible_when"] else "vw_unconverted"].append(f"{op}.{row['field']}: {text}")
                notes.append(f"{row['label'] or row['field']}: shown when {'; '.join(dropped[drop_before:])}"
                             " (not machine-readable)")
            if len(layout_notes) > lay_before:
                report["vw_layout"] += 1
        out_rows.append(row)

    # colours
    for row in out_rows:
        row["colors"] = derive_color(row)
    for f, col in dec["colors"].get(op, {}).items():
        hit = [row for row in out_rows if row["field"] == f]
        if not hit:
            fail(f"layout-decisions: colors {op}.{f} matches no row")
        hit[0]["colors"] = col

    # tiers and primaries
    for f, tier in dec["tier"].get(op, {}).items():
        hit = [row for row in out_rows if row["field"] == f]
        if not hit:
            fail(f"layout-decisions: tier {op}.{f} matches no row")
        hit[0]["tier"] = tier
    if curated:
        primary = []
    elif op in dec["primary"]:
        primary = list(dec["primary"][op])
    else:
        primary = [row["field"] for row in out_rows if row["tier"] == "primary"
                   and row["widget"] in ("slider", "combobox", "toggle")][:2]
    for row in out_rows:
        if row["widget"] == "notice":
            row["tier"] = "primary"
        elif row["widget"] == "canvas":
            row["tier"] = "detail"
        elif row["field"] in primary:
            row["tier"] = "primary"
        elif row["tier"] == "primary":
            row["tier"] = "detail"

    # conditions on a params member that is not a row (layout-decisions "visible_params"): retouch
    # shows the fill and blur rows by its `algorithm`, which the tool on the photo sets
    param_refs = set()
    for f, cond in dec.get("visible_params", {}).get(op, {}).items():
        hit = [row for row in out_rows if row["field"] == f]
        if not hit:
            fail(f"layout-decisions: visible_params {op}.{f} matches no row")
        hit[0]["visible_when"] = cond
        param_refs.update(c["field"] for c in cond.get("all", [cond]))
        label = hit[0]["label"] or hit[0]["field"]
        notes[:] = [n for n in notes if not (n.startswith(label + ": shown when") and "not machine-readable" in n)]

    if not out_rows and not curated:
        fail(f"{op} has no rows")
    used_tabs = [t for t in tabs if any(row["tab"] == t for row in out_rows)]
    for row in out_rows:
        if row["tab"] is not None and row["tab"] not in used_tabs:
            row["tab"] = None  # channel tabs of a single curve are carried in custom.channels
    if layout_notes:
        notes.append("darktable shows some rows only on a tab, in a collapsible section or with a preference;"
                     " that layout state is not part of visible_when")
    extra = dec["notes"].get(op)
    if extra:
        notes.insert(0, extra)

    deprecated = bool(inv["deprecated"])
    module = OrderedDict()
    module["operation"] = op
    module["name"] = inv["name"]
    module["purpose"] = inv["purpose"]
    module["curated"] = curated
    module["deprecated"] = deprecated
    module["show"] = "if-used" if deprecated and not curated else "always"
    module["tabs"] = used_tabs
    module["primary"] = primary
    module["rows"] = out_rows
    module["notes"] = " ".join(n if n.endswith(".") else n + "." for n in dict.fromkeys(notes))
    module["_curated_refs"] = {r["path"] or r["field"] for r in curated_rows} | param_refs
    if curated:
        # A further instance of a curated module is not edited through controls.h: it gets
        # every row, as an uncurated module would (instance_rows, instance_primary, instance_tabs).
        full = build_module(op, inv, dec, curated_pairs, curated_modules - {op},
                            dict(reordered=[], notices=[], canvas=[], vw_partial=[], vw_unconverted=[], vw_layout=0))
        full.pop("_curated_refs")
        module["instance_rows"] = full["rows"]
        module["instance_primary"] = full["primary"]
        module["instance_tabs"] = full["tabs"]
    return module


def validate_rows(where, rows, primary=(), curated_refs=()):
    errors = []
    fields = Counter(r["field"] for r in rows)
    for f, n in fields.items():
        if n > 1:
            errors.append(f"{where}: duplicate field {f}")
    for p in primary:
        if p not in fields:
            errors.append(f"{where}: primary {p} is not a row")
    if len(primary) > 2:
        errors.append(f"{where}: more than two primaries")
    for r in rows:
        tag = f"{where}.{r['field']}"
        if r["widget"] not in WIDGETS:
            errors.append(f"{tag}: unknown widget {r['widget']}")
        if r["tier"] not in TIERS:
            errors.append(f"{tag}: unknown tier {r['tier']}")
        if r["colors"] not in COLORS:
            errors.append(f"{tag}: unknown colors {r['colors']}")
        if r["widget"] == "slider":
            for k in ("min", "max", "default"):
                if not isinstance(r[k], (int, float)) or isinstance(r[k], bool):
                    errors.append(f"{tag}: slider {k} is not numeric ({r[k]!r})")
        if r["widget"] == "combobox":
            if not r["values"]:
                errors.append(f"{tag}: combobox without values")
            elif r["default"] not in [v["value"] for v in r["values"]]:
                errors.append(f"{tag}: default {r['default']!r} is not one of the values")
        c = r["custom"]
        if c is not None:
            if c["kind"] not in CUSTOM_KINDS:
                errors.append(f"{tag}: unknown custom kind {c['kind']}")
            if c.get("interpolation") not in INTERPOLATIONS:
                errors.append(f"{tag}: unknown interpolation {c.get('interpolation')}")
        vw = r["visible_when"]
        if vw is not None:
            for cond in vw.get("all", [vw]):
                if (cond.get("field") not in fields and cond.get("field") not in curated_refs) \
                        or not isinstance(cond.get("in"), list):
                    errors.append(f"{tag}: visible_when refers to {cond.get('field')}")
    return errors


def build_panes(dec, layout, errors):
    """The summary and the Advanced order of the Tone, Color, Detail and Effects panes.

    A summary entry is a slider row of the generated layout ({"module", "field"}). The Filters
    pane is the home of the curated modules: none of them may appear in another pane, and no
    summary label may repeat a Filters name ("filters_labels") or another summary label (the
    alternatives of one "pick" aside), so one name never means two controls. Entries with the
    same "pick" key belong to alternative modules (the tone mappers): the pane shows the rows of
    the first of them the image uses. The display labels of rows ("display_labels") are applied
    here too.
    """
    modules = {m["operation"]: m for g in layout["groups"] for m in g["modules"]}
    for op, labels in dec.get("display_labels", {}).items():
        if op == "note":
            continue
        for field, text in labels.items():
            hit = [r for r in modules[op]["rows"] if r["field"] == field] if op in modules else []
            if not hit:
                errors.append(f"display_labels: {op}.{field} is not a row")
                continue
            hit[0]["display"] = text
    panes = OrderedDict()
    taken = {text.lower(): "Filters" for text in dec.get("panes", {}).get("filters_labels", [])}
    for pane, spec in dec.get("panes", {}).items():
        if pane in ("note", "filters_labels"):
            continue
        summary = []
        for e in spec["summary"]:
            if not e.get("label"):
                errors.append(f"panes.{pane}: summary entry without a label: {e}")
            m = modules.get(e.get("module"))
            if m and m["curated"]:
                errors.append(f"panes.{pane}: {e['module']} is curated; its home is the Filters pane")
            where = f"{pane}.{e.get('module')}.{e.get('field')}"
            key = str(e.get("label", "")).lower()
            if key in taken and taken[key] != where:
                errors.append(f"panes.{pane}: the label '{e.get('label')}' is already used by {taken[key]}")
            taken[key] = where
            row = next((r for r in m["rows"] if r["field"] == e.get("field")), None) if m else None
            if not row or row["widget"] != "slider" or not row["path"]:
                errors.append(f"panes.{pane}: {e.get('module')}.{e.get('field')} is not a slider row")
                continue
            colors = e.get("colors", row["colors"])
            if colors not in COLORS:
                errors.append(f"panes.{pane}: unknown colors {colors}")
            out = OrderedDict(module=e["module"], field=e["field"], label=e["label"], colors=colors)
            if e.get("pick"):
                out["pick"] = e["pick"]
            summary.append(out)
        for op in spec["advanced"]:
            if op not in modules:
                errors.append(f"panes.{pane}: advanced lists unknown module {op}")
            elif modules[op]["curated"]:
                errors.append(f"panes.{pane}: advanced lists the curated module {op}")
        if len(set(spec["advanced"])) != len(spec["advanced"]):
            errors.append(f"panes.{pane}: advanced lists a module twice")
        panes[pane] = OrderedDict(summary=summary, advanced=spec["advanced"])
    panes["filters_labels"] = dec.get("panes", {}).get("filters_labels", [])
    return panes


def main():
    modules, blending = load_inventory()
    INVENTORY.update(modules)
    dec = json.loads((DESIGN / "layout-decisions.json").read_text())
    curated_pairs, registry_modules = load_curated()
    curated_modules = set(dec["curated"]["filters"]) | set(dec["curated"]["geometry"])
    missing = (registry_modules - curated_modules)
    if missing:
        fail("controls.h registers modules not marked curated: " + ", ".join(sorted(missing)))

    errors = []
    placed = Counter()
    for g in dec["groups"]:
        if g["id"] not in GROUP_IDS:
            errors.append("unknown group id " + g["id"])
        placed.update(g["modules"])
    for op in modules:
        if op in dec["hidden"]:
            if placed[op]:
                errors.append(f"{op} is hidden and placed")
            continue
        if placed[op] != 1:
            errors.append(f"{op} is placed {placed[op]} times")
    for op in placed:
        if op not in modules:
            errors.append(f"decisions place unknown module {op}")
    if [g["id"] for g in dec["groups"]] != GROUP_IDS:
        errors.append("groups must appear in the order " + ", ".join(GROUP_IDS))
    if errors:
        fail("\n".join(errors))

    report = dict(reordered=[], notices=[], canvas=[], vw_partial=[], vw_unconverted=[], vw_layout=0)
    layout = OrderedDict(darktable="5.6.1", groups=[])
    for g in dec["groups"]:
        mods = []
        for op in g["modules"]:
            m = build_module(op, modules[op], dec, curated_pairs, curated_modules, report)
            errors += validate_rows(op, m["rows"], m["primary"], m.pop("_curated_refs"))
            mods.append(m)
        layout["groups"].append(OrderedDict(id=g["id"], label=g["label"], modules=mods))

    # blend section
    bnotes = []
    brows = base_rows("@blending", blending["controls"], bnotes)
    blive = list(brows)
    blend_rows, blay, bdrop = [], [], []
    for r in brows:
        if r["field"] == "@drawn_mask_shapes":
            blend_rows.append(notice_row("drawn mask shapes are drawn on the image — not available yet"))
            continue
        row = finish_row("@blending", r, bnotes)
        before = len(bdrop)
        row["visible_when"] = convert_visible("@blending", r["_inv"]["visible_when"], blive, blay, bdrop)
        if len(bdrop) > before:
            bnotes.append(f"{row['label']}: shown when {'; '.join(bdrop[before:])} (not machine-readable)")
        row["colors"] = derive_color(row)
        blend_rows.append(row)
    errors += validate_rows("@blending", blend_rows)
    blend = OrderedDict(darktable="5.6.1", name=blending["name"], rows=blend_rows,
                        notes=" ".join(n if n.endswith(".") else n + "." for n in dict.fromkeys(bnotes)))

    layout["panes"] = build_panes(dec, layout, errors)
    if errors:
        fail("\n".join(errors))

    (DESIGN / "layout.json").write_text(json.dumps(layout, indent=1, ensure_ascii=False) + "\n")
    (DESIGN / "layout-blending.json").write_text(json.dumps(blend, indent=1, ensure_ascii=False) + "\n")

    # summary
    total_rows = 0
    kinds, widgets = Counter(), Counter()
    print("layout.json (darktable 5.6.1)")
    for g in layout["groups"]:
        n = sum(len(m["rows"]) for m in g["modules"])
        total_rows += n
        cur = sum(m["curated"] for m in g["modules"])
        print(f"  {g['id']:<11} {len(g['modules']):>3} modules ({cur} curated) {n:>4} rows")
        for m in g["modules"]:
            for r in m["rows"]:
                widgets[r["widget"]] += 1
                if r["custom"]:
                    kinds[r["custom"]["kind"]] += 1
    nmod = sum(len(g["modules"]) for g in layout["groups"])
    print(f"  total       {nmod:>3} modules            {total_rows:>4} rows; hidden: {len(dec['hidden'])}")
    print("  widgets: " + ", ".join(f"{k} {v}" for k, v in widgets.most_common()))
    print("  custom widgets: " + ", ".join(f"{k} {v}" for k, v in kinds.most_common()))
    print("  notices: " + ", ".join(report["notices"]))
    print("  on-canvas tools: " + ", ".join(report["canvas"]))
    print(f"  visible_when: {len(report['vw_partial'])} partly converted, {len(report['vw_unconverted'])} left null "
          f"(image/sensor conditions), {report['vw_layout']} layout-only clauses dropped")
    print("  rows regrouped by tab: " + (", ".join(report["reordered"]) or "none"))
    print(f"layout-blending.json: {len(blend_rows)} rows")
    if "-v" in sys.argv:
        for line in report["vw_partial"] + report["vw_unconverted"]:
            print("    " + line)


if __name__ == "__main__":
    main()
