#!/usr/bin/env python3
"""Private per-preset value optimizer (tuning images only).

Starts from the shipped preset values (the ported source values) and runs
coordinate descent over selected parameter groups, rendering with the
current Omalux binary and measuring mean ΔE00 against the frozen reference
proxies. Writes the best candidate to calibration/optimized/<preset>.json
plus a full trace. Never touches holdout, references, or public files.

Usage: preset_optimizer.py <preset-id> [--rounds 2] [--jpeg 6] [--raw 2]
"""

from __future__ import annotations

import argparse
from common import BINARY, DATASET, TARGETS, WORK, builtin_preset, images, normalize_image
from metrics import pair_metrics as _pair_metrics
import copy
import json
import numpy as _np
import os
import statistics
import subprocess
import sys
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path


import subprocess as _subprocess

def registry_bounds() -> dict:
    """Parameter id -> (minimum, maximum) from the live registry."""
    raw = _subprocess.run(
        [str(BINARY), "parameters", "list", "--json"],
        capture_output=True, text=True, check=True).stdout
    rows = json.loads(raw)
    rows = rows if isinstance(rows, list) else rows.get("parameters", rows)
    return {r["id"]: (r["minimum"], r["maximum"]) for r in rows
            if "minimum" in r and "maximum" in r}


# A colour band that is pushed far enough will help the average over a mixed
# catalogue while destroying every picture that actually contains its colour:
# the ten JPEGs hold no saturated red, so rotating the red band 28 degrees cost
# nothing there and turned deep reds orange everywhere else. Bands stay within
# what a person would dial in by hand.
MIXER_LIMITS = {
    "hue_shift_degrees": (-20.0, 20.0),
    "saturation": (-35.0, 35.0),
    "luminance": (-30.0, 30.0),
}


# Mean colour difference is blind to the three faults a person notices first,
# because each of them is small per pixel and consistent across the frame: a
# channel biased over the whole picture reads as a colour cast, a lower spread
# reads as washed out, and highlights sitting too high read as overexposed.
# Averaging colour difference lets an optimiser trade all three away for a
# fractionally better score, which is how a preset ends up measuring well and
# looking wrong. Each fault is priced so that the amount at which it becomes
# visible costs about as much as one unit of colour difference.
CHANNEL_BIAS_WEIGHT = 0.25
CONTRAST_WEIGHT = 8.0
DISTRIBUTION_WEIGHT = 0.05
REGION_GRID = 6
REGION_WEIGHT = 0.12
HUE_FLIP_MARGIN = 3.0
# A flip is priced at the reference's own gap, not the smaller of the two:
# a navy sky that comes out brown is wrong by the whole navy, however faint
# the brown. At the old weight a brown band across a tuning frame's sky cost
# less than half a count and was kept.
HUE_FLIP_WEIGHT = 0.5


def visible_fault_penalty(reference: bytes, ours: bytes) -> float:
    a = _np.frombuffer(reference, dtype=_np.uint8).reshape(-1, 3).astype(_np.float64)
    b = _np.frombuffer(ours, dtype=_np.uint8).reshape(-1, 3).astype(_np.float64)
    bias = float(_np.abs(b.mean(0) - a.mean(0)).max())
    reference_spread = float(a.std())
    contrast = abs(1.0 - float(b.std()) / reference_spread) if reference_spread > 0 else 0.0
    highlights = abs(float(_np.percentile(b, 99)) - float(_np.percentile(a, 99)))
    shadows = abs(float(_np.percentile(b, 1)) - float(_np.percentile(a, 1)))
    # A bias that cancels out over the frame still shows if it is concentrated:
    # one region too blue against another too warm averages to nothing. The
    # worst region is therefore priced on its own.
    side = int(len(a) ** 0.5)
    worst_region = 0.0
    if side * side == len(a):
        grid = a.reshape(side, side, 3)
        ours_grid = b.reshape(side, side, 3)
        step = max(1, side // REGION_GRID)
        for top in range(0, side, step):
            for left in range(0, side, step):
                delta = (ours_grid[top:top + step, left:left + step].reshape(-1, 3).mean(0)
                         - grid[top:top + step, left:left + step].reshape(-1, 3).mean(0))
                worst_region = max(worst_region, float(_np.abs(delta).max()))
    # A region whose colour tips the other way — teal where the reference is
    # blue, or blue where it is teal — reads as wrong however small the
    # numbers are, because the eye judges hue by which channel leads. Nothing
    # above catches that: a sky 8 counts too blue and a sky 8 counts too green
    # score the same. So every region's leading channel order is compared with
    # the reference's, and a flip is priced whether or not the amounts agree.
    hue_flips = 0.0
    if side * side == len(a):
        for top in range(0, side, step):
            for left in range(0, side, step):
                ours = ours_grid[top:top + step, left:left + step].reshape(-1, 3).mean(0)
                theirs = grid[top:top + step, left:left + step].reshape(-1, 3).mean(0)
                for first, second in ((2, 1), (0, 1), (0, 2)):
                    reference_gap = theirs[first] - theirs[second]
                    ours_gap = ours[first] - ours[second]
                    if abs(reference_gap) >= HUE_FLIP_MARGIN and abs(ours_gap) >= HUE_FLIP_MARGIN \
                            and (reference_gap > 0) != (ours_gap > 0):
                        hue_flips += abs(reference_gap)
    return (CHANNEL_BIAS_WEIGHT * bias
            + CONTRAST_WEIGHT * contrast
            + DISTRIBUTION_WEIGHT * (highlights + shadows)
            + REGION_WEIGHT * worst_region
            + HUE_FLIP_WEIGHT * hue_flips)


def clamp_to_registry(path_key: str, value: float, bounds: dict) -> float:
    if path_key.startswith("color_mixer/"):
        limit = MIXER_LIMITS.get(path_key.rsplit("/", 1)[-1])
        if limit is not None:
            value = max(limit[0], min(limit[1], value))
    limits = bounds.get(path_key)
    if limits is None and path_key.startswith("tone_curves/"):
        value = max(-4.0, min(4.0, value))
        # Nodes at non-negative input may not map below zero. A curve that
        # crushes a tone below black gains nothing the renderer can show, and
        # with a warm black lift on the channel curves it painted a brown band
        # across a sky exactly where the master curve crossed zero.
        if int(path_key.split("/")[3]) >= 1:
            value = max(0.0, value)
        return value
    if limits is None:
        return value
    return max(limits[0], min(limits[1], value))



# (path, absolute step sizes for the first round; halved each round)
PARAMETERS = [
    (("basics", "exposure_ev"), [0.15, 0.05]),
    (("basics", "brightness"), [15.0, 5.0]),
    (("basics", "contrast"), [15.0, 5.0]),
    (("basics", "highlights"), [25.0, 8.0]),
    (("basics", "shadows"), [25.0, 8.0]),
    (("basics", "whites"), [15.0, 5.0]),
    (("basics", "blacks"), [15.0, 5.0]),
    (("basics", "saturation"), [10.0, 4.0]),
    (("basics", "vibrance"), [10.0, 4.0]),
    (("basics", "temperature"), [8.0, 3.0]),
    (("basics", "tint"), [8.0, 3.0]),
    (("basics", "clarity"), [10.0, 4.0]),
]
MIXER_BANDS = ["red", "orange", "yellow", "green", "aqua", "blue", "purple", "magenta"]
GRADE_ZONES = ["shadows", "midtones", "highlights"]


def get_path(doc: dict, path: tuple) -> float:
    node = doc["settings"]
    for key in path[:-1]:
        node = node[key]
    return node[path[-1]]


def curve_parameters(doc: dict) -> list[tuple[tuple, list[float]]]:
    """Tunable y-values of every non-identity tone curve's nodes.

    Endpoint nodes are included: the archived curves carry extended domains,
    and endpoint y drives extrapolation strength."""
    params = []
    for channel in ("master", "red", "green", "blue"):
        curve = doc["settings"]["tone_curves"].get(channel)
        if not curve:
            continue
        points = curve["points"]
        if all(abs(pt["y"] - pt["x"]) < 1e-9 for pt in points):
            continue
        for index in range(len(points)):
            params.append(((  "tone_curves", channel, "points", index, "y"), [0.06, 0.02]))
    return params


def set_path(doc: dict, path: tuple, value: float) -> None:
    node = doc["settings"]
    for key in path[:-1]:
        node = node[key]
    node[path[-1]] = value


def active_parameters(doc: dict) -> list[tuple[tuple, list[float]]]:
    params = list(PARAMETERS)
    mixer = doc["settings"]["color_mixer"]
    for band in MIXER_BANDS:
        if any(mixer[band][k] != 0.0 for k in ("hue_shift_degrees", "saturation", "luminance")):
            params.append((("color_mixer", band, "hue_shift_degrees"), [20.0, 7.0]))
            params.append((("color_mixer", band, "saturation"), [15.0, 5.0]))
            params.append((("color_mixer", band, "luminance"), [15.0, 5.0]))
    effects = doc["settings"]["effects"]
    for field in ("bloom", "halation", "fade", "vignette", "sharpness"):
        if effects[field] != 0.0:
            params.append((("effects", field), [10.0, 4.0]))
    if effects["grain"]["amount"] != 0.0:
        params.append((("effects", "grain", "amount"), [10.0, 4.0]))
        params.append((("effects", "grain", "size_iso"), [1000.0, 400.0]))
        params.append((("effects", "grain", "midtone_response"), [20.0, 8.0]))
    grading = doc["settings"]["color_grading"]
    grading_active = any(grading[z]["saturation"] != 0.0 or grading[z]["luminance"] != 0.0
                        for z in GRADE_ZONES)
    for zone in GRADE_ZONES:
        if grading_active:
            params.append((("color_grading", zone, "hue_degrees"), [30.0, 10.0]))
            params.append((("color_grading", zone, "saturation"), [12.0, 4.0]))
    if grading_active:
        params.append((("color_grading", "blending"), [30.0, 10.0]))
        params.append((("color_grading", "balance"), [25.0, 8.0]))
    return params


class Evaluator:
    def __init__(self, pid: str, images: list[dict], out: Path, jobs: int, proxy: int = 0):
        self.proxy = proxy
        # Search-time source proxies: decode the big JPEG originals once.
        self.search_sources = {}
        if proxy:
            (out / "raw-cache").mkdir(parents=True, exist_ok=True)
            proxy_dir = out / "source-proxy"
            proxy_dir.mkdir(parents=True, exist_ok=True)
            for item in images:
                if item["media_type"] != "jpeg":
                    continue
                target = proxy_dir / f"{item['id']}.jpg"
                if not target.is_file():
                    subprocess.run(
                        ["magick", str(DATASET / item["path"]), "-auto-orient",
                         "-resize", "1024x1024>", "-quality", "97", str(target)],
                        check=True, timeout=300)
                self.search_sources[item["id"]] = target
        self.pid = pid
        self.images = images
        self.out = out
        self.jobs = jobs
        self.cache = out / "proxy-cache"
        self.cache.mkdir(parents=True, exist_ok=True)
        (out / "work").mkdir(exist_ok=True)
        self.refs = {}
        for item in images:
            ref = next((TARGETS / pid).glob(f"{item['id']}.*"), None)
            assert ref is not None, f"missing reference for {item['id']}"
            pixels, _, _ = normalize_image(ref, self.cache / f"ref--{ref.name}.px", 120)
            self.refs[item["id"]] = pixels
        self.evaluations = 0
        self.memo = {}

    def render_one(self, arg) -> float | None:
        preset_path, item, tag = arg
        output = self.out / "work" / f"{item['id']}-{tag}.jpeg"
        source = self.search_sources.get(item["id"], DATASET / item["path"])
        result = subprocess.run(
            [str(BINARY), "develop", "--input", str(source),
             "--preset-file", str(preset_path), "--output", str(output), "--overwrite",
             "--format", "jpeg", "--quality", "90", "--unprofiled", "assume-srgb",
             "--metadata", "strip-location", "--alpha", "reject",
             "--max-source-bytes", "67108864", "--max-pixels", "64000000",
             "--max-working-bytes", "8589934592", "--max-output-bytes", "536870912",
             "--progress", "none"]
            + (["--long-edge", str(self.proxy)] if self.proxy else [])
            + (["--raw-decode-cache", str(self.out / "raw-cache")] if self.proxy else []),
            stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=600, check=False)
        if result.returncode:
            return None
        cache_key = self.cache / f"work--{item['id']}-{tag}.px"
        for stale in (cache_key, cache_key.with_suffix(".json"), cache_key.with_suffix(".rgb")):
            stale.unlink(missing_ok=True)
        pixels, _, _ = normalize_image(output, cache_key, 120)
        reference = self.refs[item["id"]]
        delta_e = _pair_metrics(reference, pixels)["delta_e_2000_mean"]
        return delta_e + visible_fault_penalty(reference, pixels)

    def score(self, doc: dict, tag: str = "a") -> float:
        key = json.dumps(doc, sort_keys=True)
        if key in self.memo:
            return self.memo[key]
        preset_path = self.out / "work" / f"candidate-{tag}.json"
        preset_path.write_text(json.dumps(doc))
        values = []
        with ThreadPoolExecutor(max_workers=self.jobs) as pool:
            for value in pool.map(self.render_one, [(preset_path, i, tag) for i in self.images]):
                if value is None:
                    return float("inf")
                values.append(value)
        self.evaluations += 1
        result = statistics.fmean(values)
        self.memo[key] = result
        return result

    def score_pair(self, first: dict, second: dict) -> tuple[float, float]:
        with ThreadPoolExecutor(max_workers=2) as pool:
            a = pool.submit(self.score, first, "a")
            b = pool.submit(self.score, second, "b")
            return a.result(), b.result()


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("preset")
    parser.add_argument("--rounds", type=int, default=2)
    parser.add_argument("--jobs", type=int, default=2)
    parser.add_argument("--jpeg", type=int, default=5)
    parser.add_argument("--raw", type=int, default=2)
    parser.add_argument("--proxy", type=int, default=512,
                        help="search-time --long-edge; 0 renders full resolution")
    parser.add_argument("--pin", action="append", default=[],
                        help="parameter path prefix (slash-separated) to exclude from tuning")
    parser.add_argument("--curves", action="store_true",
                        help="also tune tone-curve node y-values")
    parser.add_argument("--start", type=Path, default=None,
                        help="start from this preset JSON instead of the shipped one")
    args = parser.parse_args()

    tuning = images("tuning")
    images = ([i for i in tuning if i["media_type"] == "jpeg"][:args.jpeg]
              + [i for i in tuning if i["media_type"] == "raw"][:args.raw])
    only = os.environ.get("OMALUX_ONLY_IMAGE")
    if only:
        images = [i for i in tuning if i["id"] in only.split(",")]

    out = (Path(os.environ["OMALUX_OPT_OUT"]) / args.preset
           if os.environ.get("OMALUX_OPT_OUT")
           else WORK / args.preset)
    out.mkdir(parents=True, exist_ok=True)
    best = json.loads(args.start.read_text()) if args.start else builtin_preset(args.preset)
    evaluator = Evaluator(args.preset, images, out, args.jobs, args.proxy)

    global OPTIMIZER_BOUNDS
    OPTIMIZER_BOUNDS = {k.replace(".", "/"): v for k, v in registry_bounds().items()}
    best_score = evaluator.score(best)
    if best_score == float("inf"):
        print("start preset does not render; aborting instead of writing it", flush=True)
        return 1
    trace = [{"step": "start", "score": best_score}]
    print(f"start ΔE={best_score:.4f} on {len(images)} tuning images", flush=True)

    params = active_parameters(best)
    if args.curves:
        params = params + curve_parameters(best)
    if args.pin:
        params = [entry for entry in params
                  if not any("/".join(map(str, entry[0])).startswith(pin)
                             for pin in args.pin)]
    for round_index in range(args.rounds):
        improved = False
        for path, steps in params:
            step = steps[min(round_index, len(steps) - 1)]
            current = get_path(best, path)
            candidates = []
            for candidate_value in (current + step, current - step):
                candidate_value = clamp_to_registry(
                    "/".join(map(str, path[:3] if path[0] != "tone_curves" else path)),
                    candidate_value, OPTIMIZER_BOUNDS)
                candidate = copy.deepcopy(best)
                set_path(candidate, path, candidate_value)
                candidates.append((candidate_value, candidate))
            scores = evaluator.score_pair(candidates[0][1], candidates[1][1])
            for (candidate_value, candidate), score in zip(candidates, scores):
                if score < best_score - 1e-4:
                    best, best_score, improved = candidate, score, True
                    trace.append({"step": "/".join(map(str, path)), "value": candidate_value,
                                  "score": score})
                    (out / "best.json").write_text(json.dumps(best, indent=1))
                    (out / "trace.json").write_text(json.dumps(
                        {"images": [i["id"] for i in images], "final_score": best_score,
                         "evaluations": evaluator.evaluations, "trace": trace,
                         "in_progress": True}, indent=1))
                    print(f"  round {round_index+1}: {'/'.join(map(str, path))} -> "
                          f"{candidate_value:.3f}  ΔE={score:.4f}", flush=True)
        if not improved:
            break

    (out / "best.json").write_text(json.dumps(best, indent=1))
    (out / "trace.json").write_text(json.dumps(
        {"images": [i["id"] for i in images], "final_score": best_score,
         "evaluations": evaluator.evaluations, "trace": trace}, indent=1))
    print(f"final ΔE={best_score:.4f} after {evaluator.evaluations} evaluations")
    return 0


if __name__ == "__main__":
    sys.exit(main())
