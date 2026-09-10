#!/usr/bin/env python3
"""Scene-referred style search.

Builds the look from darktable modules that run before the tone mapper
(exposure, color balance rgb, tone equalizer, sigmoid) plus the spatial
modules, with no display cube, and searches their parameters against the
target renderings by coordinate descent from a neutral start. Steps halve
when a pass gains less than 0.05.

  scene_search.py <style> [--passes 5] [--start neutral|bundled]

Output: work/<style>/scene/style.dtstyle and scene.json (history, state).
The result is a plain darktable style that can be opened in darktable.
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
from common import STYLES, images, style_dirs, score_render  # noqa: E402

# (module, field, step, min, max)
PARAMS = [
    ("exposure", "exposure", 0.3, -3.0, 3.0),
    ("exposure", "black", 0.01, -0.1, 0.1),
    ("colorbalancergb", "contrast", 0.15, -1.0, 1.0),
    ("colorbalancergb", "vibrance", 0.2, -1.0, 1.0),
    ("colorbalancergb", "saturation_global", 0.2, -1.0, 1.0),
    ("colorbalancergb", "chroma_global", 0.2, -1.0, 1.0),
    ("colorbalancergb", "hue_angle", 10.0, -60.0, 60.0),
    ("colorbalancergb", "shadows_Y", 0.1, -1.0, 1.0),
    ("colorbalancergb", "midtones_Y", 0.1, -1.0, 1.0),
    ("colorbalancergb", "highlights_Y", 0.1, -1.0, 1.0),
    ("colorbalancergb", "chroma_shadows", 0.2, -1.0, 1.0),
    ("colorbalancergb", "chroma_highlights", 0.2, -1.0, 1.0),
    ("colorbalancergb", "saturation_shadows", 0.2, -1.0, 1.0),
    ("colorbalancergb", "saturation_highlights", 0.2, -1.0, 1.0),
    ("colorbalancergb", "brilliance_shadows", 0.15, -1.0, 1.0),
    ("colorbalancergb", "brilliance_highlights", 0.15, -1.0, 1.0),
    # 4-way tints: chroma with hue (hue searched only when chroma > 0)
    ("colorbalancergb", "shadows_C", 0.05, 0.0, 0.5),
    ("colorbalancergb", "shadows_H", 30.0, 0.0, 360.0),
    ("colorbalancergb", "midtones_C", 0.05, 0.0, 0.5),
    ("colorbalancergb", "midtones_H", 30.0, 0.0, 360.0),
    ("colorbalancergb", "highlights_C", 0.05, 0.0, 0.5),
    ("colorbalancergb", "highlights_H", 30.0, 0.0, 360.0),
    ("colorbalancergb", "global_C", 0.05, 0.0, 0.5),
    ("colorbalancergb", "global_H", 30.0, 0.0, 360.0),
    ("toneequal", "noise", 0.3, -2.0, 2.0),
    ("toneequal", "deep_blacks", 0.3, -2.0, 2.0),
    ("toneequal", "blacks", 0.3, -2.0, 2.0),
    ("toneequal", "shadows", 0.3, -2.0, 2.0),
    ("toneequal", "midtones", 0.3, -2.0, 2.0),
    ("toneequal", "highlights", 0.3, -2.0, 2.0),
    ("toneequal", "whites", 0.3, -2.0, 2.0),
    ("sigmoid", "middle_grey_contrast", 0.25, 0.5, 3.0),
    ("sigmoid", "contrast_skewness", 0.2, -1.0, 1.0),
    ("sigmoid", "hue_preservation", 25.0, 0.0, 100.0),
    ("bilat", "detail", 0.15, -1.0, 2.0),
    ("shadhi", "shadows", 15.0, -100.0, 100.0),
    ("shadhi", "highlights", 15.0, -100.0, 100.0),
    ("vignette", "brightness", 0.1, -1.0, 0.3),
    ("sharpen", "amount", 0.25, 0.0, 2.0),
    ("grain", "strength", 10.0, 0.0, 100.0),
]
HUE_OF = {"shadows_H": "shadows_C", "midtones_H": "midtones_C", "highlights_H": "highlights_C", "global_H": "global_C"}
ENABLE_WHEN = {"vignette": "brightness", "sharpen": "amount", "grain": "strength", "bilat": "detail"}
ALWAYS_ON = {"exposure", "colorbalancergb", "toneequal", "sigmoid", "shadhi"}
OFF = {"colisa", "lut3d"}
MIN_GAIN = 0.004
NEUTRAL = STYLES / "neutral/style.dtstyle"


class SceneTuner:
    def __init__(self, pid, imgs, start):
        self.pid = pid
        self.pdir = style_dirs()[pid]
        self.w = cube_fit.work_dir(pid) / "scene"
        self.w.mkdir(parents=True, exist_ok=True)
        self.imgs = imgs
        text = NEUTRAL.read_text()
        for op in ALWAYS_ON | set(ENABLE_WHEN):
            params = dict(dtparams.DEFAULTS[op])
            if op in ENABLE_WHEN:
                params[ENABLE_WHEN[op]] = 0.0
            if op == "shadhi":
                params.update(shadows=0.0, highlights=0.0)
            text = dtparams.ensure_module(text, op, params, enabled=op in ALWAYS_ON)
        self.base_text = text
        self.state = dtparams.read_style(text)
        for op in OFF:
            if op in self.state:
                self.state[op]["enabled"] = False
        if start == "bundled":  # take exposure/shadhi from the bundled style
            b = dtparams.read_style(cube_fit.style_text(self.pdir))
            for op in ("exposure", "shadhi"):
                if op in b and b[op]["params"]:
                    self.state[op]["params"].update(b[op]["params"])
        self.n = 0

    def style_path(self, state, name=None):
        st = json.loads(json.dumps(state))
        for op, field in ENABLE_WHEN.items():
            st[op]["enabled"] = abs(st[op]["params"][field]) > 1e-6
        p = self.w / (name or f"trial-{self.n}.dtstyle")
        self.n += 1
        p.write_text(dtparams.write_style(self.base_text, st))
        return p

    def evaluate(self, state, tag="trial"):
        sp = self.style_path(state)
        out = self.w / "renders" / tag
        shutil.rmtree(out, ignore_errors=True)
        cube_fit.render_many([(i["path"], sp, out / (i["id"] + ".jpg"), None) for i in self.imgs])
        score, scores = calib_score(self.pid, self.imgs, out)
        return score, scores

    def coordinate_pass(self, current, scale, log):
        for op, field, step, lo, hi in PARAMS:
            if field in HUE_OF and self.state[op]["params"][HUE_OF[field]] < 1e-6:
                continue
            step = step * scale
            base = self.state[op]["params"][field]
            for direction in (+1, -1):
                moved = False
                while True:
                    val = float(np.clip(base + direction * step, lo, hi))
                    if field.endswith("_H"):
                        val = (base + direction * step) % 360.0
                    if abs(val - base) < 1e-9:
                        break
                    trial = json.loads(json.dumps(self.state))
                    trial[op]["params"][field] = val
                    score, sc = self.evaluate(trial)
                    log(f"  {op}.{field} {base:.3f} -> {val:.3f}: {score:.3f} (current {current:.3f})")
                    if score < current - MIN_GAIN:
                        self.state, current, base, moved = trial, score, val, True
                    else:
                        break
                if moved:
                    break
        return current


def calib_score(pid, imgs, out):
    scores = {i["id"]: score_render(pid, i, out / (i["id"] + ".jpg")) for i in imgs}
    return float(np.mean(list(scores.values()))), scores


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("style")
    ap.add_argument("--passes", type=int, default=5)
    ap.add_argument("--start", default="neutral")
    a = ap.parse_args()
    imgs = images("tuning")
    tn = SceneTuner(a.style, imgs, a.start)
    logf = open(tn.w / "scene.log", "a")

    def log(msg):
        print(msg, flush=True)
        logf.write(msg + "\n")
        logf.flush()

    resume = tn.w / "scene.json"
    if resume.exists():  # continue from the last saved pass
        saved = json.load(open(resume))
        tn.state = saved["state"]
        log(f"[{a.style}] resuming after {len(saved['history']) - 1} passes")
    current, sc = tn.evaluate(tn.state, "start")
    jp = np.mean([v for k, v in sc.items() if k.startswith("J")]); rw = np.mean([v for k, v in sc.items() if k.startswith("R")])
    log(f"[{a.style}] scene start {current:.3f} (jpeg {jp:.2f}, raw {rw:.2f})")
    history = saved["history"] if resume.exists() else [current]
    scale = saved.get("scale", 1.0) if resume.exists() else 1.0
    for p in range(len(history) - 1, a.passes):
        t0 = time.time()
        before = current
        current = tn.coordinate_pass(current, scale, log)
        _, sc = tn.evaluate(tn.state, "pass")
        jp = np.mean([v for k, v in sc.items() if k.startswith("J")]); rw = np.mean([v for k, v in sc.items() if k.startswith("R")])
        log(f"[{a.style}] scene pass {p} (step x{scale:.2f}): {current:.3f} (jpeg {jp:.2f}, raw {rw:.2f}) {time.time()-t0:.0f}s")
        history.append(current)
        tn.style_path(tn.state, "style.dtstyle")
        json.dump(dict(style=a.style, history=history, state=tn.state, scale=scale), open(tn.w / "scene.json", "w"), indent=1)
        if before - current < 0.05:
            scale *= 0.5
            if scale < 0.2:
                break
    log(f"[{a.style}] scene tuned {current:.3f}")


if __name__ == "__main__":
    main()
