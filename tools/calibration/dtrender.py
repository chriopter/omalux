#!/usr/bin/env python3
"""Render an image through darktable-cli with a .dtstyle applied.

The style is converted into an XMP sidecar (history stack), so no darktable
database import is needed. `render_batch_style` renders many inputs with one
style in a single darktable-cli process (folder input): process start-up is
paid once instead of per image, and with `hq=False` darktable scales early,
which makes RAW renders several times faster at a cost of a few tenths of
ΔE against full-quality processing. Fits use the fast path; final scoring
uses full quality. Every worker gets its own configuration directory
because darktable locks its database per configuration.

    python3 dtrender.py <input> <style.dtstyle> <output.jpg> [max-edge]

Environment:
  OMALUX_CALIBRATION_ROOT  work/dtcfg below it holds the per-worker configs
  OMALUX_STYLES           LUT root passed to lut3d (default: <repo>/styles)
  RAW_EXPOSURE_OFFSET      EV added to the style's exposure for RAW inputs only
                           (darktable's own RAW default is +0.7 EV, which a
                           style's absolute exposure value replaces)
  DT_EXTRA_CONF            extra `--conf key=value` pairs, separated by ';'
                           (for example plugins/darkroom/workflow=none)
  RAW_EXTRA_ITEMS          JSON list of {"op", "params"} history items appended
                           for RAW inputs only (a RAW base rendition experiment)
  DT_OPENCL=1              use OpenCL. Off by default: with several parallel
                           processes a GPU reset silently corrupts the output
                           of the other processes on some drivers, and the
                           pixelpipe is a small part of the per-process time.
"""
import os
import subprocess
import sys
import tempfile
import threading
import time
from pathlib import Path

from common import ROOT, WORK

REPO = Path(__file__).resolve().parents[2]
STYLES = Path(os.environ.get("OMALUX_STYLES", REPO / "styles"))
CFG = WORK / "dtcfg"

XMP_HEAD = """<?xpacket begin="﻿" id="W5M0MpCehiHzreSzNTczkc9d"?>
<x:xmpmeta xmlns:x="adobe:ns:meta/" x:xmptk="XMP Core 4.4.0-Exiv2">
 <rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">
  <rdf:Description rdf:about=""
    xmlns:xmp="http://ns.adobe.com/xap/1.0/"
    xmlns:xmpMM="http://ns.adobe.com/xap/1.0/mm/"
    xmlns:darktable="http://darktable.sf.net/"
   xmp:Rating="1"
   xmpMM:DerivedFrom="{src}"
   darktable:xmp_version="5"
   darktable:import_timestamp="0"
   darktable:history_end="{n}"
   darktable:iop_order_version="2">
   <darktable:history>
    <rdf:Seq>
"""
XMP_ITEM = """     <rdf:li
      darktable:num="{num}"
      darktable:operation="{op}"
      darktable:enabled="{enabled}"
      darktable:modversion="{ver}"
      darktable:params="{params}"
      darktable:multi_name="{mname}"
      darktable:multi_name_hand_edited="{mhand}"
      darktable:multi_priority="{mprio}"
      darktable:blendop_version="{bver}"{bparams}/>
"""
XMP_TAIL = """    </rdf:Seq>
   </darktable:history>
  </rdf:Description>
 </rdf:RDF>
</x:xmpmeta>
<?xpacket end="w"?>
"""

FALLBACKS = []
_slots = []
_slot_lock = threading.Lock()
_slot_count = 0


def _acquire_slot():
    global _slot_count
    with _slot_lock:
        if _slots:
            return _slots.pop()
        _slot_count += 1
        return _slot_count


def _release_slot(n):
    with _slot_lock:
        _slots.append(n)


def parse_style(path):
    import re
    s = Path(path).read_text()
    items = []
    for block in re.findall(r"<plugin>(.*?)</plugin>", s, re.S):
        def g(tag, default=""):
            m = re.search(rf"<{tag}>(.*?)</{tag}>", block, re.S)
            return m.group(1) if m else default
        items.append(dict(
            num=int(g("num", "0")), ver=int(g("module", "0")), op=g("operation"),
            params=g("op_params"), enabled=int(g("enabled", "1")),
            bparams=g("blendop_params"), bver=int(g("blendop_version", "0")),
            mprio=int(g("multi_priority", "0")), mname=g("multi_name"),
            mhand=int(g("multi_name_hand_edited", "0"))))
    items.sort(key=lambda i: i["num"])
    return items


RAW_SUFFIXES = {".cr2", ".cr3", ".nef", ".raf", ".orf", ".dng", ".rw2", ".arw", ".pef", ".srw", ".3fr", ".iiq"}


_camera_cache = {}


def camera_of(path):
    """(maker, model) from EXIF via exiv2, cached per file."""
    key = str(path)
    if key not in _camera_cache:
        maker = model = ""
        try:
            out = subprocess.run(["exiv2", "-g", "Exif.Image.Make", "-g", "Exif.Image.Model", "-Pkv", key],
                                 capture_output=True, text=True).stdout
            for line in out.splitlines():
                if line.startswith("Exif.Image.Make"):
                    maker = line.split(None, 1)[1].strip()
                elif line.startswith("Exif.Image.Model"):
                    model = line.split(None, 1)[1].strip()
        except OSError:
            pass
        _camera_cache[key] = (maker, model)
    return _camera_cache[key]


def camera_items(path):
    """History items of the camera presets (camera/*.dtpreset) that darktable would auto-apply."""
    if os.environ.get("DT_CAMERA_PRESETS", "1") != "1":
        return []
    try:
        sys.path.insert(0, str(REPO / "tools/darktable"))
        import camera_presets
    except ImportError:
        return []
    styles = camera_presets.load()
    if not styles:
        return []
    maker, model = camera_of(path)
    is_raw = Path(path).suffix.lower() in RAW_SUFFIXES
    return [dict(op=p["operation"], ver=p["version"], params=p["params"], enabled=int(p["enabled"]), bparams="",
                 bver=0, mprio=0, mname="", mhand=0) for p in camera_presets.matching(styles, maker, model, is_raw)]


def dtstyle_to_xmp(style_path, src_name, camera=()):
    items = parse_style(style_path)
    for it in camera:  # camera presets sit before the style, as darktable applies them at import
        items.insert(0, dict(it))
    offset = float(os.environ.get("RAW_EXPOSURE_OFFSET", "0"))
    if offset and Path(src_name).suffix.lower() in RAW_SUFFIXES:
        import dtparams
        for it in items:
            if it["op"] == "exposure" and it["params"]:
                d = dtparams.decode("exposure", it["params"])
                d["exposure"] += offset
                it["params"] = dtparams.encode("exposure", d)
    extra = os.environ.get("RAW_EXTRA_ITEMS")
    if extra and Path(src_name).suffix.lower() in RAW_SUFFIXES:
        import json
        import dtparams
        for e in json.loads(extra):
            d = dict(dtparams.DEFAULTS[e["op"]])
            d.update(e.get("params", {}))
            items.append(dict(num=len(items), ver=dtparams.VERSIONS[e["op"]], op=e["op"],
                              params=dtparams.encode(e["op"], d), enabled=int(e.get("enabled", 1)),
                              bparams="", bver=0, mprio=0, mname="", mhand=0))
    seen = {}
    for it in items:  # a style item for the same module wins over the camera preset
        seen[it["op"]] = it
    items = [it for it in items if seen[it["op"]] is it]
    out = [XMP_HEAD.format(src=src_name, n=len(items))]
    for i, it in enumerate(items):
        bparams = f'\n      darktable:blendop_params="{it["bparams"]}"' if it["bparams"] else ""
        out.append(XMP_ITEM.format(num=i, op=it["op"], enabled=it["enabled"], ver=it["ver"],
                                   params=it["params"], mname=it["mname"], mhand=it["mhand"],
                                   mprio=it["mprio"], bver=it["bver"], bparams=bparams))
    out.append(XMP_TAIL)
    return "".join(out)


def render(inp, style_path, output, width=0, height=0, opencl=None, quality=90, extra_conf=()):
    """Run darktable-cli. Returns (seconds, stderr). Raises RuntimeError on failure."""
    if opencl is None:
        opencl = os.environ.get("DT_OPENCL", "0") == "1"
    inp = Path(inp)
    output = Path(output)
    output.parent.mkdir(parents=True, exist_ok=True)
    slot = _acquire_slot()
    cfg = CFG / f"p{os.getpid()}-w{slot}"
    cache = CFG / "cache"
    cfg.mkdir(parents=True, exist_ok=True)
    cache.mkdir(exist_ok=True)
    try:
        try:
            return _render(inp, style_path, output, width, height, opencl, quality, extra_conf, cfg, cache)
        except RuntimeError:
            if not opencl:
                raise
            FALLBACKS.append(str(inp))
            sys.stderr.write(f"[dtrender] GPU render failed for {inp.name}, retrying on CPU\n")
            return _render(inp, style_path, output, width, height, False, quality, extra_conf, cfg, cache)
    finally:
        _release_slot(slot)


def _render(inp, style_path, output, width, height, opencl, quality, extra_conf, cfg, cache):
    with tempfile.TemporaryDirectory(dir=os.environ.get("TMPDIR", "/tmp")) as td:
        xmp = Path(td) / (inp.name + ".xmp")
        xmp.write_text(dtstyle_to_xmp(style_path, inp.name, camera_items(inp)))
        cmd = ["darktable-cli", str(inp), str(xmp), str(output)]
        if width:
            cmd += ["--width", str(width)]
        if height:
            cmd += ["--height", str(height)]
        cmd += ["--core", "--configdir", str(cfg), "--cachedir", str(cache), "--library", ":memory:",
                "--conf", f"opencl={'TRUE' if opencl else 'FALSE'}",
                "--conf", f"plugins/darkroom/lut3d/def_path={STYLES}",
                "--conf", f"plugins/imageio/format/jpeg/quality={quality}",
                "--conf", "plugins/lighttable/export/iccprofile=sRGB",
                "--conf", "write_sidecar_files=never"]
        extra = list(extra_conf) + [c for c in os.environ.get("DT_EXTRA_CONF", "").split(";") if c]
        for c in extra:
            cmd += ["--conf", c]
        if output.exists():
            output.unlink()
        t = time.time()
        p = subprocess.run(cmd, capture_output=True, text=True)
        dt = time.time() - t
        if p.returncode != 0 or not output.exists():
            raise RuntimeError(f"darktable-cli failed ({p.returncode}) for {inp} / {style_path}:\n"
                               f"{p.stderr[-2000:]}\n{p.stdout[-1000:]}")
        return dt, p.stderr


def render_batch_style(style_path, jobs, width=1024, height=1024, hq=False, quality=90, extra_conf=(), threads=None):
    """Batch render (one darktable-cli process) of `jobs` = [(input, output)] with one style.
    With RAW_EXPOSURE_OFFSET set, RAW and non-RAW inputs go in separate batches."""
    if not jobs:
        return []
    split = float(os.environ.get("RAW_EXPOSURE_OFFSET", "0")) or os.environ.get("RAW_EXTRA_ITEMS")
    groups = {}
    for inp, out in jobs:
        key = (Path(inp).suffix.lower() in RAW_SUFFIXES if split else False,
               tuple(sorted(str(it["params"]) for it in camera_items(inp))))
        groups.setdefault(key, []).append((inp, out))
    failed = []
    for g in groups.values():
        failed += _render_batch(g, width, height, hq, quality, extra_conf, threads, style_path)
    return failed


def _render_batch(jobs, width, height, hq, quality, extra_conf, threads, style_path=None):
    import shutil
    opencl = os.environ.get("DT_OPENCL", "0") == "1"
    slot = _acquire_slot()
    cfg = CFG / f"p{os.getpid()}-w{slot}"; cache = CFG / "cache"
    cfg.mkdir(parents=True, exist_ok=True); cache.mkdir(exist_ok=True)
    try:
        with tempfile.TemporaryDirectory(dir=os.environ.get("TMPDIR", "/tmp")) as td:
            td = Path(td); inp_dir = td / "in"; out_dir = td / "out"
            inp_dir.mkdir(); out_dir.mkdir()
            names = {}
            for inp, out in jobs:
                inp = Path(inp); link = inp_dir / inp.name
                if link.exists():
                    raise RuntimeError(f"duplicate input name in batch: {inp.name}")
                link.symlink_to(inp.resolve())
                names[inp.stem] = Path(out)
            xmp = td / "style.xmp"
            xmp.write_text(dtstyle_to_xmp(style_path, "x" + Path(jobs[0][0]).suffix, camera_items(jobs[0][0])))
            ext = Path(jobs[0][1]).suffix or ".jpg"
            cmd = ["darktable-cli", str(inp_dir), str(xmp), str(out_dir / ("$(FILE_NAME)" + ext)),
                   "--width", str(width), "--height", str(height), "--hq", "true" if hq else "false",
                   "--core", "--configdir", str(cfg), "--cachedir", str(cache), "--library", ":memory:",
                   "--conf", f"opencl={'TRUE' if opencl else 'FALSE'}",
                   "--conf", f"plugins/darkroom/lut3d/def_path={STYLES}",
                   "--conf", f"plugins/imageio/format/jpeg/quality={quality}",
                   "--conf", "plugins/lighttable/export/iccprofile=sRGB",
                   "--conf", "write_sidecar_files=never"]
            for c in list(extra_conf) + [c for c in os.environ.get("DT_EXTRA_CONF", "").split(";") if c]:
                cmd += ["--conf", c]
            env = dict(os.environ)
            if threads:
                env["OMP_NUM_THREADS"] = str(threads)
            p = subprocess.run(cmd, capture_output=True, text=True, env=env)
            failed = []
            for stem, out in names.items():
                got = out_dir / (stem + ext)
                if got.exists():
                    out.parent.mkdir(parents=True, exist_ok=True)
                    if out.exists():
                        out.unlink()
                    shutil.move(str(got), str(out))
                else:
                    failed.append(out)
            if failed:
                sys.stderr.write(f"[dtrender] batch: {len(failed)} of {len(jobs)} missing (rc {p.returncode})\n{p.stderr[-1500:]}\n")
            return failed
    finally:
        _release_slot(slot)


if __name__ == "__main__":
    inp, style, out = sys.argv[1:4]
    edge = int(sys.argv[4]) if len(sys.argv) > 4 else 0
    seconds, err = render(inp, style, out, edge, edge)
    print(f"{seconds:.2f}s")
