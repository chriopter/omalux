#!/usr/bin/env python3
"""Render an image through darktable-cli with a .dtstyle applied.

The style is converted into an XMP sidecar (history stack), so no darktable
database import is needed and any number of renders can run in parallel.
Every worker gets its own configuration directory because darktable locks
its database per configuration.

    python3 dtrender.py <input> <style.dtstyle> <output.jpg> [max-edge]

Environment:
  OMALUX_CALIBRATION_ROOT  work/dtcfg below it holds the per-worker configs
  OMALUX_PRESETS           LUT root passed to lut3d (default: <repo>/presets)
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
PRESETS = Path(os.environ.get("OMALUX_PRESETS", REPO / "presets"))
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


def dtstyle_to_xmp(style_path, src_name):
    items = parse_style(style_path)
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
        xmp.write_text(dtstyle_to_xmp(style_path, inp.name))
        cmd = ["darktable-cli", str(inp), str(xmp), str(output)]
        if width:
            cmd += ["--width", str(width)]
        if height:
            cmd += ["--height", str(height)]
        cmd += ["--core", "--configdir", str(cfg), "--cachedir", str(cache), "--library", ":memory:",
                "--conf", f"opencl={'TRUE' if opencl else 'FALSE'}",
                "--conf", f"plugins/darkroom/lut3d/def_path={PRESETS}",
                "--conf", f"plugins/imageio/format/jpeg/quality={quality}",
                "--conf", "plugins/lighttable/export/iccprofile=sRGB",
                "--conf", "write_sidecar_files=never"]
        for c in extra_conf:
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


if __name__ == "__main__":
    inp, style, out = sys.argv[1:4]
    edge = int(sys.argv[4]) if len(sys.argv) > 4 else 0
    seconds, err = render(inp, style, out, edge, edge)
    print(f"{seconds:.2f}s")
