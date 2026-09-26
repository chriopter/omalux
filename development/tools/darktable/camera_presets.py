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

REPO = Path(__file__).resolve().parents[3]
CAMERA = REPO / "catalog/camera"
FLOAT_MAX = "340282346638528859811704183484516925440"
# darktable's default blending parameters (develop_blend_params_t v14), taken from a preset it
# migrated itself. A preset without valid blending data is skipped when it is applied, so the
# file has to carry them; darktable only fills them in on its next start.
BLEND_VERSION = 14
BLEND_PARAMS = ("000000000000000018000000000000000000c84200000000000000000000000000000000050000000000000000000000000000000000000001000000000000000000000000000000000000000000803f0000803f00000000000000000000803f0000803f00000000000000000000803f0000803f00000000000000000000803f0000803f00000000000000000000803f0000803f00000000000000000000803f0000803f00000000000000000000803f0000803f00000000000000000000803f0000803f00000000000000000000803f0000803f00000000000000000000803f0000803f00000000000000000000803f0000803f00000000000000000000803f0000803f00000000000000000000803f0000803f00000000000000000000803f0000803f00000000000000000000803f0000803f00000000000000000000803f0000803f00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000ffffffff00000000")
FOR_LDR, FOR_RAW, FOR_HDR = 1, 2, 4

# (maker pattern, model pattern, file name, description, operation, params)
# Embedded lens metadata: distortion, vignetting and chromatic aberration data the camera
# writes into the file. Enabled for makers whose files carry it and whose target renderings
# show it applied; not for Ricoh, where the same correction made things worse. Panasonic was
# once left out on the same grounds, judged while the camera presets were silently not applied
# at all; with them applied, the TZ60 frame loses its dark corners and matches the target.
LENS_EMBEDDED = dict(method=0, modify_flags=7)
# Colour noise only. A preset that removed all sensor noise (nlmeans, luma and chroma 1,
# strength 100, radius 4) turned grass into mush and skies into blotches at full size, while
# the target renderings keep their fine luminance grain. What they do not keep is colour
# mottle: with luma left alone and chroma at 0.5, a flat sky loses its blotches and a textured
# fabric keeps its grain. Measured on six RAW files under three styles, 0.5 brings flat-area
# roughness and detail closest to the targets together (1.0 over-smooths, 0.3 leaves mottle).
RAW_CHROMA_DENOISE = dict(radius=2.0, strength=50.0, luma=0.0, chroma=0.5)
# Capture sharpening, for every RAW file. darktable sharpens nothing by itself, and the target
# renderings are sharper than ours on RAW files only (JPEG sources already match). Measured at
# full size on six RAW files under four styles, against the targets' detail and flat-area
# roughness together: amount 1.5 with threshold 2 comes closest to both; the threshold keeps
# low-contrast noise out, which plain sharpening and darktable's demosaicing sharpening
# (diffuse or sharpen) both amplify.
RAW_SHARPEN = dict(radius=2.0, amount=1.5, threshold=2.0)
# The Hasselblad L1D (the camera of a DJI drone) writes its vignetting correction into the DNG as
# a gain map in OpcodeList3, which the DNG specification marks mandatory and darktable 5.6 skips
# ("unsupported mandatory opcode 9"): corners come out up to 2.4 times too dark. darktable's
# manual vignette correction is fitted to that gain map (taken from the file, not from any
# rendering): strength 0.625, radius 0.1, steepness 0.8 match it within 3 % out to 85 % of the
# half diagonal and fall short only in the extreme corners (1.94 against 2.26).
# has_been_set: without it darktable replaces every lens parameter by its own autodetected
# defaults at render time, keeping only the method, and the vignette strength falls back to 0.
L1D_LENS = dict(LENS_EMBEDDED, v_strength=0.625, v_radius=0.1, v_steepness=0.8, has_been_set=1)
# Exposure per camera model, as RAW developers apply a baseline exposure by model and darktable
# does not. Measured on the reference set: the EV that fits all looks at once for that camera's
# image (one photograph per camera, so treat the values as a first estimate). It is a second
# exposure instance with a hand-edited name: a look's own exposure item matches instances by name,
# or by priority only where the name was not set by hand, so the look adds its exposure on top
# instead of replacing this one. Without any look it replaces darktable's default +0.7 EV for
# these models.
CAMERA_EXPOSURE = "camera exposure"
def camera_ev(ev):
    return dict(mode=0, black=0.0, exposure=ev, compensate_exposure_bias=0, compensate_hilite_pres=0,
                instance=CAMERA_EXPOSURE)
TABLE = [
    ("%", "%", "all/sharpen", "Every camera: capture sharpening", "sharpen", RAW_SHARPEN),
    ("%", "%", "all/colour-noise", "Every camera: colour noise reduction", "nlmeans", RAW_CHROMA_DENOISE),
    ("FUJIFILM", "%", "fujifilm/all", "Fujifilm: embedded lens correction", "lens", LENS_EMBEDDED),
    ("OLYMPUS%", "%", "olympus/all", "Olympus: embedded lens correction", "lens", LENS_EMBEDDED),
    ("OM Digital%", "%", "om-system/all", "OM System: embedded lens correction", "lens", LENS_EMBEDDED),
    ("DJI", "%", "dji/all", "DJI: embedded lens correction", "lens", LENS_EMBEDDED),
    ("Google", "Pixel%", "google/pixel", "Google Pixel: embedded lens correction", "lens", LENS_EMBEDDED),
    ("Apple", "iPhone%", "apple/iphone", "Apple iPhone: embedded lens correction", "lens", LENS_EMBEDDED),
    ("Panasonic", "%", "panasonic/all", "Panasonic: embedded lens correction", "lens", LENS_EMBEDDED),
    ("SIGMA%", "%fp%", "sigma/fp", "Sigma fp: embedded lens correction", "lens", LENS_EMBEDDED),
    ("Hasselblad", "L1D%", "hasselblad/l1d", "Hasselblad L1D: embedded lens correction and vignetting",
     "lens", L1D_LENS),
    ("Canon%", "%EOS 6D", "canon/eos-6d", "Canon EOS 6D: exposure", "exposure", camera_ev(0.3)),
    ("FUJIFILM%", "X-T10", "fujifilm/x-t10-exposure", "Fujifilm X-T10: exposure", "exposure", camera_ev(0.2)),
    ("OLYMPUS%", "E-M1", "olympus/e-m1", "Olympus E-M1: exposure", "exposure", camera_ev(0.6)),
    ("DJI", "FC220", "dji/fc220", "DJI FC220: exposure", "exposure", camera_ev(0.2)),
    ("RICOH%", "%GR III", "ricoh/gr-iii", "Ricoh GR III: exposure", "exposure", camera_ev(-0.2)),
    ("Canon%", "%SX100 IS", "canon/powershot-sx100-is", "Canon PowerShot SX100 IS: exposure", "exposure", camera_ev(-0.3)),
    ("Apple", "iPhone XS", "apple/iphone-xs", "Apple iPhone XS: exposure", "exposure", camera_ev(-0.1)),
]


def preset_xml(name, description, operation, params_hex, version, maker, model, fmt=FOR_RAW,
               autoapply=True, multi_name=""):
    root = ET.Element("darktable_preset", version="1.0")
    p = ET.SubElement(root, "preset")
    fields = [("name", name), ("description", description), ("operation", operation), ("op_params", params_hex),
              ("op_version", str(version)), ("enabled", "1"), ("autoapply", "1" if autoapply else "0"), ("model", model), ("maker", maker),
              ("lens", "%"), ("iso_min", "0"), ("iso_max", FLOAT_MAX), ("exposure_min", "0"), ("exposure_max", FLOAT_MAX),
              ("aperture_min", "0"), ("aperture_max", FLOAT_MAX), ("focal_length_min", "0"), ("focal_length_max", "1000"),
              ("blendop_params", BLEND_PARAMS), ("blendop_version", str(BLEND_VERSION)), ("multi_priority", "0"), ("multi_name", multi_name),
              ("multi_name_hand_edited", "1" if multi_name else "0"), ("filter", "0"), ("def", "0"), ("format", str(fmt))]
    for tag, value in fields:
        ET.SubElement(p, tag).text = value
    ET.indent(root)
    return '<?xml version="1.0" encoding="UTF-8"?>\n' + ET.tostring(root, encoding="unicode") + "\n"


def generate():
    for maker, model, file, description, op, params in TABLE:
        full = dict(dtparams.DEFAULTS[op])
        full.update(params)
        instance = full.pop("instance", "") if "instance" in full else ""
        # darktable replaces a preset with the same name and operation, so each file needs its own name.
        text = preset_xml(f"Omalux {description}", description, op, dtparams.encode(op, full),
                          dtparams.VERSIONS[op], maker, model, multi_name=instance)
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
                        autoapply=get("autoapply") == "1", format=int(get("format") or 0), description=get("description"),
                        multi_name=get("multi_name")))
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
