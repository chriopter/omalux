#!/usr/bin/env python3
"""Camera presets: darktable module presets applied automatically by camera.

    python3 tools/darktable/camera_presets.py generate   # write camera/<maker>/<model>.dtpreset from TABLE
    python3 tools/darktable/camera_presets.py list       # show what the files declare

A camera preset is a `.dtpreset` file as darktable exports it from
preferences > presets, with `autoapply` set and `maker`/`model` patterns
(SQL LIKE syntax, so `Pixel%` matches a family) and `format` 2 for RAW files.
darktable imports the file from preferences > presets > import, and from then
on applies the module settings when a matching RAW is opened, before any look.

The calibration renderer reads the same files and applies the matching
presets to its RAW renders, so the looks are fitted on the same base.
"""
import json
import struct
import sys
from pathlib import Path
from xml.etree import ElementTree as ET

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "calibration"))
import dtparams  # noqa: E402

REPO = Path(__file__).resolve().parents[2]
CAMERA = REPO / "camera"
FLOAT_MAX = "340282346638528859811704183484516925440"
FOR_LDR, FOR_RAW, FOR_HDR = 1, 2, 4

# (maker pattern, model pattern, file name, description, operation, params)
# Embedded lens metadata: distortion, vignetting and chromatic aberration data the camera
# writes into the file. Enabled for makers whose files carry it and whose target renderings
# show it applied; not for Ricoh or Panasonic, where the same correction made things worse.
LENS_EMBEDDED = dict(method=0, modify_flags=7)
TABLE = [
    ("FUJIFILM", "%", "fujifilm/all", "Fujifilm: embedded lens correction", "lens", LENS_EMBEDDED),
    ("OLYMPUS%", "%", "olympus/all", "Olympus: embedded lens correction", "lens", LENS_EMBEDDED),
    ("OM Digital%", "%", "om-system/all", "OM System: embedded lens correction", "lens", LENS_EMBEDDED),
    ("DJI", "%", "dji/all", "DJI: embedded lens correction", "lens", LENS_EMBEDDED),
    ("Google", "Pixel%", "google/pixel", "Google Pixel: embedded lens correction", "lens", LENS_EMBEDDED),
    ("Apple", "iPhone%", "apple/iphone", "Apple iPhone: embedded lens correction", "lens", LENS_EMBEDDED),
]


def preset_xml(name, description, operation, params_hex, version, maker, model, fmt=FOR_RAW):
    root = ET.Element("darktable_preset", version="1.0")
    p = ET.SubElement(root, "preset")
    fields = [("name", name), ("description", description), ("operation", operation), ("op_params", params_hex),
              ("op_version", str(version)), ("enabled", "1"), ("autoapply", "1"), ("model", model), ("maker", maker),
              ("lens", "%"), ("iso_min", "0"), ("iso_max", FLOAT_MAX), ("exposure_min", "0"), ("exposure_max", FLOAT_MAX),
              ("aperture_min", "0"), ("aperture_max", FLOAT_MAX), ("focal_length_min", "0"), ("focal_length_max", "1000"),
              ("blendop_params", ""), ("blendop_version", "0"), ("multi_priority", "0"), ("multi_name", ""),
              ("multi_name_hand_edited", "0"), ("filter", "0"), ("def", "0"), ("format", str(fmt))]
    for tag, value in fields:
        ET.SubElement(p, tag).text = value
    ET.indent(root)
    return '<?xml version="1.0" encoding="UTF-8"?>\n' + ET.tostring(root, encoding="unicode") + "\n"


def generate():
    for maker, model, file, description, op, params in TABLE:
        full = dict(dtparams.DEFAULTS[op])
        full.update(params)
        text = preset_xml(f"Omalux camera preset", description, op, dtparams.encode(op, full),
                          dtparams.VERSIONS[op], maker, model)
        dest = CAMERA / (file + ".dtpreset")
        dest.parent.mkdir(parents=True, exist_ok=True)
        dest.write_text(text)
        print(dest.relative_to(REPO))


def load():
    """Parse camera/**/*.dtpreset -> list of dicts with maker, model, operation, params(hex), version, format."""
    out = []
    for path in sorted(CAMERA.rglob("*.dtpreset")):
        p = ET.parse(path).getroot().find("preset")
        get = lambda tag: (p.findtext(tag) or "")
        out.append(dict(path=path, maker=get("maker"), model=get("model"), operation=get("operation"),
                        params=get("op_params"), version=int(get("op_version") or 0), enabled=get("enabled") == "1",
                        autoapply=get("autoapply") == "1", format=int(get("format") or 0), description=get("description")))
    return out


def like(pattern, value):
    """SQL LIKE with % and _ against a string, case-insensitive like darktable's matching."""
    import re
    rx = "^" + "".join(".*" if c == "%" else "." if c == "_" else re.escape(c) for c in pattern) + "$"
    return re.match(rx, value, re.IGNORECASE) is not None


def matching(presets, maker, model, is_raw=True):
    return [p for p in presets if p["autoapply"] and like(p["maker"], maker) and like(p["model"], model)
            and (p["format"] & (FOR_RAW if is_raw else FOR_LDR))]


if __name__ == "__main__":
    cmd = sys.argv[1] if len(sys.argv) > 1 else "list"
    if cmd == "generate":
        generate()
    else:
        for p in load():
            print(f"{p['path'].relative_to(REPO)}: {p['maker']} / {p['model']} -> {p['operation']} ({p['description']})")
