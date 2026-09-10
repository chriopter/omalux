#!/usr/bin/env python3
"""Copy a calibrated style and cube into the bundled style folder.

  install_style.py <style> [...]

Takes work/<style>/tuned/ when slider_tune.py ran, otherwise the fitted
style and best.cube from cube_fit.py. The thumbnail is not regenerated; run
`development/style_preview <style>` afterwards.
"""
import json
import shutil
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from common import WORK, style_dirs  # noqa: E402


def main(pids):
    dirs = style_dirs()
    for pid in pids:
        pdir = dirs[pid]
        w = WORK / pid
        tuned, scene = w / "tuned", w / "scene"
        if (scene / "style.dtstyle").exists():
            shutil.copy(scene / "style.dtstyle", pdir / "style.dtstyle")
            (pdir / "look.cube").unlink(missing_ok=True)
            manifest = pdir / "style.json"
            if manifest.exists():
                d = json.load(open(manifest))
                d["assets"] = []
                manifest.write_text(json.dumps(d, indent=2) + "\n")
            print(f"{pid}: installed scene/style.dtstyle (no cube)")
            continue
        if (tuned / "style.dtstyle").exists():
            style, cube = tuned / "style.dtstyle", tuned / "look.cube"
        else:
            style, cube = w / "style.dtstyle", w / "best.cube"
        shutil.copy(style, pdir / "style.dtstyle")
        shutil.copy(cube, pdir / "look.cube")
        print(f"{pid}: installed {style.parent.name}/{style.name} and {cube.name}")


if __name__ == "__main__":
    main(sys.argv[1:])
