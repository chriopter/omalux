#!/usr/bin/env python3
"""Search the spatial module parameters on top of a fitted cube.

  slider_tune.py <preset> [--passes 3]

Starts from work/<preset>/best.cube and style.dtstyle (see cube_fit.py).
Only parameters after lut3d with a spatial effect are searched: shadows and
highlights, vignette, sharpening, grain. Global tone and colour are left to
the cube, which already absorbs them. Because the cube was fitted for the
current sliders, every trial gets one cube update and a second render before
it is judged; otherwise "no change" always wins. After each coordinate pass
the cube is refitted for three iterations.

Writes work/<preset>/tuned/preset.dtstyle, look.cube and tuned.json.
"""
import argparse
import json
import shutil
import sys
import time
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parent))
import cube_fit  # noqa: E402
import dtparams  # noqa: E402
from common import WORK, images, preset_dirs  # noqa: E402

# (module, field, step, min, max)
PARAMS = [
    ("bilat", "detail", 0.15, -1.0, 2.0),
    ("shadhi", "shadows", 15.0, -100.0, 100.0),
    ("shadhi", "highlights", 15.0, -100.0, 100.0),
    ("shadhi", "radius", 40.0, 10.0, 300.0),
    ("vignette", "brightness", 0.1, -1.0, 0.3),
    ("vignette", "scale", 15.0, 0.0, 150.0),
    ("vignette", "falloff_scale", 15.0, 0.0, 150.0),
    ("sharpen", "amount", 0.25, 0.0, 2.0),
    ("grain", "strength", 10.0, 0.0, 100.0),
]
ENABLE_WHEN = {"vignette": "brightness", "sharpen": "amount", "grain": "strength", "bilat": "detail"}
MIN_GAIN = 0.005


class Tuner:
    def __init__(self, pid, imgs):
        self.pid = pid
        self.pdir = preset_dirs()[pid]
        self.w = cube_fit.work_dir(pid)
        self.t = self.w / "tuned"
        self.t.mkdir(parents=True, exist_ok=True)
        self.imgs = imgs
        self.base_text = cube_fit.base_style(pid, self.pdir).read_text()
        self.state = dtparams.read_style(self.base_text)
        for op, field in ENABLE_WHEN.items():
            if self.state.get(op, {}).get("params") is None:
                params = dict(dtparams.DEFAULTS[op])
                params[field] = 0.0  # a module absent from the style starts switched off
                self.state[op] = dict(params=params, enabled=False)
                self.base_text = dtparams.ensure_module(self.base_text, op, params, enabled=False)
        self.rel = cube_fit.lut_relpath(self.pdir)
        self.lut_dir = self.t / "lut"
        shutil.copy(self.w / "best.cube", self.cube_path())
        self.n = 0
        self.trial_cube = None
        self.A, self.B = cube_fit.lut_inputs(pid, self.w / "style.dtstyle", imgs, self.w / "input")

    def cube_path(self):
        p = self.lut_dir / self.rel
        p.parent.mkdir(parents=True, exist_ok=True)
        return p

    def style_path(self, state):
        st = json.loads(json.dumps(state))
        for op, field in ENABLE_WHEN.items():
            st[op]["enabled"] = abs(st[op]["params"][field]) > 1e-6
        p = self.t / f"trial-{self.n}.dtstyle"
        self.n += 1
        p.write_text(dtparams.write_style(self.base_text, st))
        return p

    def render(self, sp, tag, lut_dir):
        out = self.t / "renders" / tag
        shutil.rmtree(out, ignore_errors=True)
        cube_fit.render_many([(i["path"], sp, out / (i["id"] + ".jpg"), lut_dir) for i in self.imgs])
        return out

    def evaluate(self, state, tag, refit=True):
        sp = self.style_path(state)
        out = self.render(sp, tag, self.lut_dir)
        score, scores = cube_fit.score_dir(self.pid, self.imgs, out)
        self.trial_cube = None
        if refit:
            cube = cube_fit.cube_update(cube_fit.read_cube(self.cube_path()), self.imgs, self.A, self.B,
                                        out, self.w / "input")
            tl = self.t / "trial-lut"
            cube_fit.write_cube(tl / self.rel, cube, "trial")
            out2 = self.render(sp, tag + "-refit", tl)
            s2, sc2 = cube_fit.score_dir(self.pid, self.imgs, out2)
            if s2 < score:
                score, scores, self.trial_cube = s2, sc2, cube
        return score, scores

    def accept(self):
        if self.trial_cube is not None:
            cube_fit.write_cube(self.cube_path(), self.trial_cube, f"{self.pid} tuned")
            self.trial_cube = None

    def coordinate_pass(self, current, log):
        for op, field, step, lo, hi in PARAMS:
            base = self.state[op]["params"][field]
            for direction in (+1, -1):
                moved = False
                while True:
                    val = float(np.clip(base + direction * step, lo, hi))
                    if abs(val - base) < 1e-9:
                        break
                    trial = json.loads(json.dumps(self.state))
                    trial[op]["params"][field] = val
                    score, sc = self.evaluate(trial, "trial")
                    head = " ".join(f"{k}:{v:.2f}" for k, v in list(sc.items())[:4])
                    log(f"  {op}.{field} {base:.3f} -> {val:.3f}: {score:.3f} (current {current:.3f}) [{head}]")
                    if score < current - MIN_GAIN:
                        self.state, current, base, moved = trial, score, val, True
                        self.accept()
                    else:
                        break
                if moved:
                    break
        return current

    def cube_refit(self, current, iters, log):
        sp = self.style_path(self.state)
        cube = cube_fit.read_cube(self.cube_path())
        best_cube, best = cube.copy(), current
        for k in range(iters):
            out = self.render(sp, f"refit-{k}", self.lut_dir)
            mean, _ = cube_fit.score_dir(self.pid, self.imgs, out)
            log(f"  cube refit iter {k}: {mean:.3f}")
            if mean < best:
                best, best_cube = mean, cube.copy()
            cube = cube_fit.cube_update(cube, self.imgs, self.A, self.B, out, self.w / "input")
            cube_fit.write_cube(self.cube_path(), cube, f"{self.pid} refit")
        cube_fit.write_cube(self.cube_path(), best_cube, f"{self.pid} tuned")
        return best


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("preset")
    ap.add_argument("--passes", type=int, default=3)
    a = ap.parse_args()
    tn = Tuner(a.preset, images("tuning"))
    logf = open(tn.t / "tune.log", "a")

    def log(msg):
        print(msg, flush=True)
        logf.write(msg + "\n")
        logf.flush()

    current, _ = tn.evaluate(tn.state, "start", refit=False)
    log(f"[{a.preset}] start {current:.3f}")
    history = [current]
    for p in range(a.passes):
        t0 = time.time()
        current = tn.coordinate_pass(current, log)
        log(f"[{a.preset}] pass {p} sliders: {current:.3f}")
        current = tn.cube_refit(current, 3, log)
        log(f"[{a.preset}] pass {p} cube: {current:.3f} ({time.time() - t0:.0f}s)")
        history.append(current)
        shutil.copy(tn.style_path(tn.state), tn.t / "preset.dtstyle")
        shutil.copy(tn.cube_path(), tn.t / "look.cube")
        json.dump(dict(preset=a.preset, history=history, state=tn.state),
                  open(tn.t / "tuned.json", "w"), indent=1)
        if history[-2] - history[-1] < 0.02:
            break
    log(f"[{a.preset}] tuned {current:.3f}")


if __name__ == "__main__":
    main()
