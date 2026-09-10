#!/usr/bin/env python3
"""Build a darktable distribution bundle from the bundled styles.

    python3 tools/darktable/build_bundle.py [--out dist/darktable] [--embed-luts]

Output layout:

    dist/darktable/
      styles/<group>/<name>.dtstyle   one style per look, named "Omalux|<Group>|<Name>"
      luts/<catalogue path>.cube      the LUT files the styles reference (unless embedded)
      camera/<maker>/<model>.dtpreset automatically applied camera presets
      camera/<maker>/<model>.icc      input profiles those presets select, if any
      README.md                       how to install in darktable

Styles keep the catalogue-relative LUT path, so darktable's LUT root
(preferences > processing > 3D LUT root folder) must point at `luts/`.
With --embed-luts the cube is compressed into G'MIC keypoints and stored
inside the style (darktable's lut3d "gmz" path), which needs `gmic` on the
path and a darktable built with G'MIC support; then no LUT folder is needed.

Camera presets are read from `camera/` in the repository if present; see
docs/reference/darktable-bundle.md for the file format.
"""
import argparse
import json
import re
import shutil
import struct
import subprocess
import sys
import tempfile
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
STYLES = REPO / "catalog/styles"
CAMERA = REPO / "catalog/camera"
GROUP_NAMES = {"film": "Film", "experimental": "Experimental", "monochrome": "Monochrome",
               "series/movie": "Movie", "series/late-summer": "Late Summer"}
LUT3D_KEYPOINT_BYTES = 2048 * 2 * 3
MAX_KEYPOINTS = 2048


def group_of(bundle: Path):
    rel = bundle.relative_to(STYLES).parent.as_posix()
    return GROUP_NAMES.get(rel, rel.replace("/", " ").title() if rel != "." else "")


def style_name(bundle: Path, original: str):
    group = group_of(bundle)
    return f"Omalux|{group}|{original}" if group else f"Omalux|{original}"


def lut3d_params(hexstr):
    b = bytes.fromhex(hexstr)
    path = b[:512].split(b"\0")[0].decode()
    colorspace, interpolation, nb_keypoints = struct.unpack("<iii", b[512:524])
    return dict(path=path, colorspace=colorspace, interpolation=interpolation,
                nb_keypoints=nb_keypoints, keypoints=b[524:524 + LUT3D_KEYPOINT_BYTES], size=len(b))


def lut3d_encode(path, colorspace, interpolation, nb_keypoints, keypoints, size):
    body = path.encode().ljust(512, b"\0") + struct.pack("<iii", colorspace, interpolation, nb_keypoints)
    body += keypoints.ljust(LUT3D_KEYPOINT_BYTES, b"\0")
    return body.ljust(size, b"\0").hex()


def compress_cube(cube: Path, error: float, work: Path):
    """Return G'MIC keypoints (x,y,z,r,g,b as bytes, N*6) for a .cube file."""
    gmz = work / (cube.stem + ".gmz")
    raw = work / (cube.stem + ".raw")
    env = dict(TMP=str(work), TEMP=str(work), TMPDIR=str(work))
    subprocess.run(["gmic", "-v", "-1", "input_cube", str(cube), "compress_clut", f"{error},{error}",
                    "-o", str(gmz)], check=True, env={**dict(__import__("os").environ), **env})
    # keypoints come as an image of width 1, height N, 6 channels; export channel-interleaved uint8
    subprocess.run(["gmic", "-v", "-1", str(gmz), "permute", "cxyz", "-o", f"{raw},uint8"], check=True,
                   env={**dict(__import__("os").environ), **env})
    data = raw.read_bytes()
    n = len(data) // 6
    if n > MAX_KEYPOINTS:
        raise RuntimeError(f"{cube}: {n} keypoints exceed darktable's limit of {MAX_KEYPOINTS}; raise --error")
    return n, data


def convert_style(bundle: Path, out_styles: Path, out_luts: Path, embed: bool, error: float, work: Path):
    text = (bundle / "style.dtstyle").read_text()
    original = re.search(r"<name>(.*?)</name>", text).group(1)
    name = style_name(bundle, original)
    text = text.replace(f"<name>{original}</name>", f"<name>{name}</name>", 1)
    m = re.search(r"(<operation>lut3d</operation>\s*<op_params>)([0-9a-f]*)(</op_params>\s*<enabled>)(\d)", text)
    lut_note = ""
    if m and m.group(4) == "1":
        p = lut3d_params(m.group(2))
        cube = STYLES / p["path"]
        if embed:
            n, keypoints = compress_cube(cube, error, work)
            new = lut3d_encode(p["path"], p["colorspace"], p["interpolation"], n, keypoints, p["size"])
            text = text[:m.start(2)] + new + text[m.end(2):]
            lut_note = f" (LUT embedded, {n} keypoints)"
        else:
            dest = out_luts / p["path"]
            dest.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy(cube, dest)
            lut_note = f" (LUT {p['path']})"
    group = group_of(bundle)
    dest = out_styles / (group.lower().replace(" ", "-") if group else ".") / f"{original}.dtstyle"
    dest.parent.mkdir(parents=True, exist_ok=True)
    dest.write_text(text)
    return name, lut_note


def copy_camera(out: Path):
    if not CAMERA.is_dir():
        return 0
    count = 0
    for style in sorted(CAMERA.rglob("*.dtpreset")):
        rel = style.relative_to(CAMERA)
        (out / rel).parent.mkdir(parents=True, exist_ok=True)
        shutil.copy(style, out / rel)
        icc = style.with_suffix(".icc")
        if icc.exists():
            shutil.copy(icc, (out / rel).with_suffix(".icc"))
        count += 1
    return count


README = """# Omalux looks and camera presets for darktable

## Looks (styles)

1. darktable → lighttable → *styles* module → *import…* → select the `.dtstyle` files in `styles/`.
   They appear as `Omalux|<group>|<name>` and can be applied in lighttable or darkroom.
2. {lut_step}

## Camera presets

1. If the bundle contains `.icc` files under `camera/`, copy them into darktable's configuration folder
   under `color/in/` (Linux: `~/.config/darktable/color/in/`) and restart darktable.
2. darktable → preferences → *presets* → *import…* → select the `.dtpreset` files in `camera/`.
   Each preset is applied automatically to RAW files of its camera model and selects that profile
   (for the current set: embedded lens correction). Looks stay independent of it.

{camera_note}
"""


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--out", default=str(REPO / "dist/darktable"))
    ap.add_argument("--embed-luts", action="store_true")
    ap.add_argument("--error", type=float, default=4.0, help="G'MIC compression error (8-bit units)")
    a = ap.parse_args()
    out = Path(a.out)
    shutil.rmtree(out, ignore_errors=True)
    styles, luts, camera = out / "styles", out / "luts", out / "camera"
    styles.mkdir(parents=True)
    with tempfile.TemporaryDirectory() as td:
        for bundle in sorted(p.parent for p in STYLES.rglob("style.dtstyle")):
            name, note = convert_style(bundle, styles, luts, a.embed_luts, a.error, Path(td))
            print(f"{name}{note}")
    n = copy_camera(camera)
    lut_step = ("Nothing else: the LUT data is stored inside each style." if a.embed_luts else
                "darktable → preferences → *processing* → *3D LUT root folder*: choose the `luts/` folder from this bundle. "
                "The styles reference their LUT files relative to it.")
    camera_note = (f"{n} camera preset(s) included; presets marked auto-apply take effect for their "
                   "camera as soon as they are imported, the others can be chosen in the module." if n else
                   "No camera presets are included in this bundle yet; darktable's own colour matrices are used.")
    (out / "README.md").write_text(README.format(lut_step=lut_step, camera_note=camera_note))
    print(f"bundle written to {out} ({n} camera presets)")


if __name__ == "__main__":
    main()
