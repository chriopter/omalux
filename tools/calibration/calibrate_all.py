#!/usr/bin/env python3
"""Runs the full calibration recipe over every preset that has references.

The recipe is the one that worked on the first preset, in the order that
matters:

  1. Tune everything against the extended objective. Colour difference alone is
     blind to a channel biased across the whole frame, to a picture that is
     flatter than the reference, and to highlights sitting too high — each is
     small per pixel and consistent, so an optimiser will trade all three away
     for a fractionally better score.
  2. Derive the colour table from what is left. The hue-band mixer cannot
     express a rendering that treats a colour differently according to how
     light it is, and every preset in this corpus does exactly that somewhere.
  3. Tune once more with the table held fixed, so the sliders settle around it
     rather than fighting it.
  4. Validate and audit at full resolution, and record both.

Each preset is checkpointed as it finishes, so the run can be stopped and
resumed without losing work.
"""

from __future__ import annotations

import json

import numpy as np

from common import BINARY, TARGETS, WORK, builtin_preset, source_for

OPTIMIZED = WORK
LEDGER = WORK / "ledger.json"
import subprocess
import sys
import time
from pathlib import Path


# Table strengths to try, strongest first. A shared table can leave a drift
# across a smooth sky, and the strength at which that becomes visible differs
# by preset: a look the sliders express well tolerates little, one they cannot
# express at all (a dark matte look lived almost entirely in its table) loses
# a third of its accuracy at seventy. So each preset takes the strongest
# setting whose skies stay as smooth as the reference's, measured the way
# `sky_smoothness.py` measures them.
TABLE_STRENGTHS = (100.0, 85.0, 70.0, 50.0)
# The arc is measured on what the table itself adds — the rendering at a
# given strength against the rendering with no table — so that a vignette,
# grain, or a look that departs from the reference in some other way cannot
# hide it. It is measured on the channel differences (R−G, R−B, G−B), not on
# RGB: a dark look's table legitimately moves the whole sky's brightness by a
# smooth but non-planar amount, while the arc that matters is a hue that
# drifts across the sky. Measured this way the one table with a visible arc
# left twelve counts at full strength and eight where the arc was gone; looks
# without an arc stay under four. Nine is the line.
ARC_LIMIT = 9.0
EXPECTED_IMAGES = 19


def run(arguments: list, minutes: int = 90, environment: dict | None = None) -> str:
    import os
    env = dict(os.environ)
    env.update(environment or {})
    result = subprocess.run([sys.executable] + arguments, cwd=HERE, env=env,
                            capture_output=True, text=True, timeout=minutes * 60)
    return (result.stdout or "") + (result.stderr or "")


def last_number(text: str, marker: str) -> float | None:
    for line in reversed(text.splitlines()):
        if marker in line:
            for piece in line.replace(",", " ").split():
                try:
                    return float(piece)
                except ValueError:
                    continue
    return None


def render_sky_frame(preset_path: Path, key: str, output: Path) -> None:
    source = source_for(key)
    subprocess.run(
        [str(BINARY), "develop", "--input", str(source), "--preset-file", str(preset_path),
         "--output", str(output), "--overwrite", "--format", "jpeg", "--quality", "95",
         "--unprofiled", "assume-srgb", "--metadata", "strip-location", "--alpha", "reject",
         "--max-source-bytes", "67108864", "--max-pixels", "64000000",
         "--max-working-bytes", "8589934592", "--max-output-bytes", "536870912",
         "--progress", "none", "--long-edge", "900"],
        stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=600, check=True)


def choose_table_strength(preset: str, document: dict, work: Path) -> tuple:
    """Returns the strongest table strength whose skies carry no arc, and the
    arc measure the table leaves at that strength."""
    import sky_smoothness
    work.mkdir(parents=True, exist_ok=True)
    keys = [key for key in sky_smoothness.SKIES
            if next((TARGETS / preset).glob(f"{key}.*"), None) is not None]

    def render_all(strength: float) -> dict:
        candidate = json.loads(json.dumps(document))
        candidate["settings"]["color_table"]["strength"] = strength
        candidate_path = work / f"strength-{int(strength)}.json"
        candidate_path.write_text(json.dumps(candidate))
        rendered = {}
        for key in keys:
            output = work / f"strength-{int(strength)}-{key}.jpeg"
            render_sky_frame(candidate_path, key, output)
            rendered[key] = sky_smoothness.load(output)
        return rendered

    without = render_all(0.0)
    chosen = (TABLE_STRENGTHS[-1], None)
    for strength in TABLE_STRENGTHS:
        rendered = render_all(strength)
        worst = 0.0
        for key in keys:
            rows = int(sky_smoothness.HEIGHT * sky_smoothness.SKIES[key])
            difference = rendered[key][:rows] - without[key][:rows]
            chroma = np.stack([difference[..., 0] - difference[..., 1],
                               difference[..., 0] - difference[..., 2],
                               difference[..., 1] - difference[..., 2]], axis=-1)
            worst = max(worst, sky_smoothness.departure_from_plane(chroma))
        print(f"  Tabellenstaerke {int(strength)}: Bogenmass {worst:.2f}", flush=True)
        chosen = (strength, round(worst, 2))
        if worst <= ARC_LIMIT:
            return chosen
    return chosen


def calibrate(preset: str) -> dict:
    started = time.time()
    record: dict = {"preset": preset}

    # Two starting points, the archived values and the neutral preset, and
    # the better result is kept. For most looks the archive is the shorter
    # way; for the crush looks it was a trap, and tuning from neutral
    # reached in ten rounds what the archive start never did.
    best = OPTIMIZED / preset / "best.json"
    scores = {}
    for start in ("archive", "neutral"):
        out = OPTIMIZED / preset / f"start-{start}"
        arguments = ["preset_optimizer.py", preset, "--rounds", "10", "--jobs", "12",
                     "--jpeg", "10", "--raw", "10", "--curves", "--proxy", "512"]
        if start == "neutral":
            neutral = builtin_preset("neutral")
            neutral["id"] = preset
            neutral["name"] = preset
            out.mkdir(parents=True, exist_ok=True)
            (out / "start.json").write_text(json.dumps(neutral))
            arguments += ["--start", str(out / "start.json")]
        run(arguments, minutes=180, environment={"OMALUX_OPT_OUT": str(OPTIMIZED / preset / "starts" / start)})
        trace = OPTIMIZED / preset / "starts" / start / preset / "trace.json"
        if trace.exists():
            scores[start] = json.loads(trace.read_text())["final_score"]
    if not scores:
        record["status"] = "no optimizer output"
        return record
    chosen = min(scores, key=scores.get)
    record["start_scores"] = {k: round(v, 3) for k, v in scores.items()}
    record["start"] = chosen
    best.parent.mkdir(parents=True, exist_ok=True)
    best.write_text((OPTIMIZED / preset / "starts" / chosen / preset / "best.json").read_text())
    print(f"  Startpunkt {chosen}: {scores}", flush=True)

    table_log = run(["fit_color_table.py", preset, "17", "--with-raws"], minutes=180)
    record["table_counts"] = last_number(table_log, "final:")
    if record["table_counts"] is None:
        record["table_log_tail"] = table_log[-400:]

    document = json.loads(best.read_text())
    if "color_table" in document["settings"]:
        strength, arc = choose_table_strength(preset, document, OPTIMIZED / preset / "strength")
        record["table_strength"] = strength
        record["sky_arc"] = arc
        document["settings"]["color_table"]["strength"] = strength
        best.write_text(json.dumps(document, indent=1))

    # The polish keeps the raw frames in the objective. Tuned on raster
    # frames alone, a high-key look once pushed brightness and highlights to
    # where every raw frame washed out, because nothing in the objective
    # could see it.
    run(["preset_optimizer.py", preset, "--start", str(best), "--rounds", "8",
         "--jobs", "12", "--jpeg", "10", "--raw", "10", "--curves", "--proxy", "512",
         "--pin", "color_table"], minutes=180)

    run(["validate_preset.py", preset], minutes=120)
    # The numbers are read from the validation file rather than scraped from
    # the console, and a run that did not render every image is a failed run,
    # not a partial success: a mean over eight frames says nothing about the
    # other eleven, and a rendering that fails silently for one class of input
    # would otherwise be reported as an improvement.
    validation_path = OPTIMIZED / preset / "validation.json"
    if not validation_path.exists():
        record["status"] = "validation missing"
        return record
    validation = json.loads(validation_path.read_text())
    rendered = sum(1 for value in validation["per_image"].values() if value)
    record["images_rendered"] = rendered
    record["delta_e"] = round(validation["delta_e_mean"], 3)
    record["ssim"] = round(validation["ssim_mean"], 4)
    if rendered < EXPECTED_IMAGES:
        record["status"] = f"only {rendered} of {EXPECTED_IMAGES} images rendered"
        return record
    record["table"] = "color_table" in json.loads(best.read_text())["settings"]

    audit = run(["audit_preset.py", preset], minutes=120)
    failing = last_number(audit, "images have findings")
    record["images_with_findings"] = failing
    record["minutes"] = round((time.time() - started) / 60.0, 1)
    record["status"] = "done"
    return record


def main() -> int:
    wanted = sys.argv[1:] or sorted(
        path.name for path in TARGETS.iterdir() if path.is_dir())
    ledger = json.loads(LEDGER.read_text()) if LEDGER.exists() else {}

    for preset in wanted:
        if ledger.get(preset, {}).get("status") == "done":
            print(f"{preset}: bereits fertig, uebersprungen", flush=True)
            continue
        print(f"\n=== {preset}", flush=True)
        try:
            ledger[preset] = calibrate(preset)
        except Exception as error:  # noqa: BLE001 - the run must outlive one preset
            ledger[preset] = {"preset": preset, "status": f"failed: {error}"}
        print(f"{preset}: {ledger[preset]}", flush=True)
        LEDGER.write_text(json.dumps(ledger, indent=1))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
