"""Shared paths and image helpers for the calibration tools.

`OMALUX_CALIBRATION_ROOT` names one directory with:

  datasets/manifest.json   items: id, path (relative to datasets/), split
                           (tuning|holdout), media_type
  datasets/proxies/        optional smaller versions of dataset files, used
                           instead of the original when present (same file name)
  targets/<style>/<id>.jpg  the rendering each style should reproduce
  work/                    everything the tools write

Scores are the mean CIEDE2000 between proxies of 256 pixels on the long edge.
"""
import json
import os
import subprocess
from pathlib import Path

import numpy as np

from metrics import _delta_e_2000, _rgb_to_lab

ROOT = Path(os.environ.get("OMALUX_CALIBRATION_ROOT", "")).expanduser()
if not ROOT.is_dir():
    raise SystemExit("OMALUX_CALIBRATION_ROOT must name the calibration data directory")
DATASETS = ROOT / "datasets"
TARGETS = ROOT / "targets"
WORK = ROOT / "work"
REPO = Path(__file__).resolve().parents[2]
STYLES = Path(os.environ.get("OMALUX_STYLES", REPO / "catalog/styles"))
PROXY = 256
RENDER = 1024
JOBS = int(os.environ.get("DT_JOBS", "6"))


def manifest():
    m = json.load(open(DATASETS / "manifest.json"))
    items = []
    for i in m["items"]:
        proxy = DATASETS / "proxies" / Path(i["path"]).name
        items.append(dict(id=i["id"], path=proxy if proxy.exists() else DATASETS / i["path"],
                          split=i["split"], kind=i.get("media_type", "")))
    return items


def images(split):
    kind = os.environ.get("DT_KIND")  # optional filter: raw | jpeg
    return [i for i in manifest() if (split == "all" or i["split"] == split)
            and (not kind or (i["kind"] != "jpeg") == (kind == "raw"))]


def style_dirs():
    """Bundled styles that have target renderings, by id."""
    out = {}
    for p in sorted(STYLES.rglob("style.dtstyle")):
        if (TARGETS / p.parent.name).is_dir():
            out[p.parent.name] = p.parent
    return out


def im_size(path):
    o = subprocess.run(["magick", "identify", "-format", "%w %h", str(path)],
                       capture_output=True, text=True).stdout
    return tuple(int(x) for x in o.split()[:2])


def read_rgb(path, w, h):
    o = subprocess.run(["magick", str(path), "-colorspace", "sRGB", "-filter", "Triangle", "-resize",
                        f"{w}x{h}!", "-depth", "8", "rgb:-"], capture_output=True).stdout
    a = np.frombuffer(o, dtype=np.uint8)
    assert a.size == w * h * 3, (path, a.size, w, h)
    return a.reshape(h, w, 3)


def box(w, h, size):
    s = min(size / w, size / h)
    return max(1, round(w * s)), max(1, round(h * s))


_ref_cache = {}


def target(pid, img, size):
    key = (pid, img["id"], size)
    if key not in _ref_cache:
        path = TARGETS / pid / (img["id"] + ".jpg")
        w, h = im_size(path)
        pw, ph = box(w, h, size)
        _ref_cache[key] = read_rgb(path, pw, ph)
    return _ref_cache[key]


def delta_e(a, b):
    return float(_delta_e_2000(_rgb_to_lab(a.reshape(-1, 3)), _rgb_to_lab(b.reshape(-1, 3))).mean())


def score_render(pid, img, render_path):
    ref = target(pid, img, PROXY)
    got = read_rgb(render_path, ref.shape[1], ref.shape[0])
    return delta_e(got, ref)
