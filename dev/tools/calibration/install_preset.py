#!/usr/bin/env python3
"""Copy a calibrated style and cube into the bundled preset folder.

  install_preset.py <preset> [...]

Takes work/<preset>/tuned/ when slider_tune.py ran, otherwise the fitted
style and best.cube from cube_fit.py. The thumbnail is not regenerated; run
`dev/scripts/preset_preview <preset>` afterwards.
"""
import json
import shutil
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from common import WORK, preset_dirs  # noqa: E402


def main(pids):
    dirs = preset_dirs()
    for pid in pids:
        pdir = dirs[pid]
        w = WORK / pid
        tuned, scene = w / "tuned", w / "scene"
        if (scene / "preset.dtstyle").exists():
            shutil.copy(scene / "preset.dtstyle", pdir / "preset.dtstyle")
            (pdir / "look.cube").unlink(missing_ok=True)
            manifest = pdir / "preset.json"
            if manifest.exists():
                d = json.load(open(manifest))
                d["assets"] = []
                manifest.write_text(json.dumps(d, indent=2) + "\n")
            print(f"{pid}: installed scene/preset.dtstyle (no cube)")
            continue
        if (tuned / "preset.dtstyle").exists():
            style, cube = tuned / "preset.dtstyle", tuned / "look.cube"
        else:
            style, cube = w / "style.dtstyle", w / "best.cube"
        shutil.copy(style, pdir / "preset.dtstyle")
        shutil.copy(cube, pdir / "look.cube")
        print(f"{pid}: installed {style.parent.name}/{style.name} and {cube.name}")


if __name__ == "__main__":
    main(sys.argv[1:])
