#!/usr/bin/env python3
"""Derives a preset's colour table from the reference corpus.

The colour mixer moves eight hue bands as wholes, so it cannot express a
rendering that treats a colour differently according to how light it is —
measured on this corpus, a deep sky is carried through as blue while a hazy one
of the same hue is taken towards cyan. A table over the whole colour space can
hold both.

The table is solved on its own grid rather than fitted as a polynomial. That
choice is the point of this file and is explained at `solve_smooth_grid`.

The fit uses the tuning images only; the holdout is never read.
"""

from __future__ import annotations

import json
import os
import subprocess
import sys
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

import numpy as np

from common import BINARY, TARGETS, WORK, source_for, tuning_ids


TUNING = tuning_ids("raster")
RAW_TUNING = tuning_ids("raw")
# Tuning frames left out of the fit, for checking within the tuning split
# whether a table generalises. The holdout is never read here.
_EXCLUDE = {key for key in os.environ.get("TABLE_EXCLUDE", "").split(",") if key}
TUNING = [key for key in TUNING if key not in _EXCLUDE]
RAW_TUNING = [key for key in RAW_TUNING if key not in _EXCLUDE]
# The raw frames carry colours no raster frame contains, and the table is short
# of coverage far more than of precision. Their own colour rendering differs
# from the reference's for a reason no preset can fix — each is a different
# camera — so they count for less than a raster frame rather than not at all.
RAW_WEIGHT = 0.35
SAMPLE = 700
# How many samples it takes for an entry to overrule its neighbours.
STIFFNESS = float(os.environ.get("TABLE_STIFFNESS", "60"))
# How far a near-neutral cell's own measurements count against its neighbours,
# and the chroma at which a colour counts as fully saturated for that purpose.
NEUTRAL_TRUST = float(os.environ.get("TABLE_NEUTRAL_TRUST", "0.12"))
NEUTRAL_CHROMA = float(os.environ.get("TABLE_NEUTRAL_CHROMA", "0.30"))
# Spread, in the 0..1 units the table works in, at which a colour is trusted
# half as far. Four counts out of 255.
DISAGREEMENT_SCALE = float(os.environ.get("TABLE_DISAGREEMENT", "0.016"))
# Widest hue rotation, in degrees, a table entry may apply. Near black every
# hue sits within a few cells of every other, and the neighbour smoothing can
# carry a warm ground's correction into a dark sky: measured on one look, a
# navy sky came out with a brown band across it. No photographic rendering
# turns blue into brown, so a rotation beyond this is pulled back to it.
HUE_GUARD_DEGREES = float(os.environ.get("TABLE_HUE_GUARD", "30"))


def render(preset: Path, source: Path, output: Path) -> None:
    subprocess.run(
        [str(BINARY), "develop", "--input", str(source), "--preset-file", str(preset),
         "--output", str(output), "--overwrite", "--format", "jpeg", "--quality", "95",
         "--unprofiled", "assume-srgb", "--metadata", "strip-location", "--alpha", "reject",
         "--max-source-bytes", "67108864", "--max-pixels", "64000000",
         "--max-working-bytes", "8589934592", "--max-output-bytes", "536870912",
         "--progress", "none", "--long-edge", str(SAMPLE)],
        stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=600, check=True)


def pixels(path: Path, width: int, height: int) -> np.ndarray:
    raw = subprocess.run(
        ["magick", str(path), "-resize", f"{width}x{height}!", "-depth", "8", "rgb:-"],
        capture_output=True, check=True).stdout
    return np.frombuffer(raw, dtype=np.uint8).reshape(-1, 3).astype(np.float64) / 255.0


def solve_smooth_grid(delta: np.ndarray, weight: np.ndarray, size: int,
                      passes: int = 3000) -> np.ndarray:
    """Solves for a correction that matches the measurements and stays smooth.

    Fitting a polynomial through the measurements looked like the careful
    choice and was not. A polynomial is defined everywhere, including the large
    part of the colour cube no photograph reaches, and nothing out there holds
    it down — so it curves freely, and that curvature appeared as a broad arc
    across a clear sky. Raising its order made the arc worse and lowering it
    made the fit useless, because neither addresses where the freedom comes
    from.

    Solving on the grid removes it instead. Every entry answers to two pulls at
    once: towards what was measured there, in proportion to how much was
    measured, and towards the average of its neighbours. Where the corpus is
    dense the measurements win and the table follows them closely. Where it is
    thin the neighbours take over and the surface is carried across smoothly,
    with no freedom to invent a shape of its own. An arc cannot form, because
    nothing in the system rewards one.
    """
    correction = np.zeros_like(delta)
    # Near the grey axis the measurements are trusted less against the
    # neighbours. A clear sky is a slow ramp through exactly that region, and
    # the eye reads any short-range variation of the correction along it as a
    # band or an arc, while what a pale sky actually needs from the table is a
    # broad, even shift. Colours with real chroma keep the full pull.
    axis = np.arange(size) / (size - 1)
    grid = np.stack(np.meshgrid(axis, axis, axis, indexing="ij"), axis=-1)
    chroma = grid.max(axis=-1) - grid.min(axis=-1)
    saturation_trust = NEUTRAL_TRUST + (1.0 - NEUTRAL_TRUST) * np.clip(chroma / NEUTRAL_CHROMA, 0.0, 1.0) ** 2
    pull = (weight * saturation_trust / STIFFNESS)[..., None]
    for _ in range(passes):
        total = np.zeros_like(correction)
        count = np.zeros(correction.shape[:3] + (1,))
        for axis in range(3):
            for shift in (1, -1):
                # Gathered without wrapping: the far corner of the colour cube
                # is not next to the near one, and joining them would drag one
                # end of the space towards the other.
                valid = np.ones(correction.shape[:3] + (1,))
                edge = [slice(None)] * 3
                edge[axis] = 0 if shift == 1 else size - 1
                valid[tuple(edge)] = 0.0
                total += np.roll(correction, shift, axis=axis) * valid
                count += valid
        correction = (pull * delta + total) / (pull + count)
    return correction


def guard_hue(identity: np.ndarray, table: np.ndarray) -> np.ndarray:
    """Limits how far each entry rotates the hue of the colour it stands for.

    Hue is taken around the entry's own grey level, so the guard acts on the
    direction of the colour, not on its lightness or chroma. Where the
    rotation exceeds the limit, the corrected colour's chroma is turned back
    to the limit on the same side, keeping its lightness and its chroma.
    """
    def polar(rgb):
        grey = rgb.mean(axis=-1, keepdims=True)
        offset = rgb - grey
        # Two orthogonal axes in the plane perpendicular to grey.
        a = offset @ np.array([1.0, -0.5, -0.5]) / np.sqrt(1.5)
        b = offset @ np.array([0.0, np.sqrt(0.75), -np.sqrt(0.75)]) / np.sqrt(1.5)
        return grey[..., 0], np.hypot(a, b), np.arctan2(b, a)

    _, chroma_in, hue_in = polar(identity)
    grey_out, chroma_out, hue_out = polar(table)
    turn = (hue_out - hue_in + np.pi) % (2 * np.pi) - np.pi
    limit = np.radians(HUE_GUARD_DEGREES)
    # Colours that were nearly grey to begin with have no hue to guard.
    active = (np.abs(turn) > limit) & (chroma_in > 0.02)
    if not active.any():
        return table
    hue_new = np.where(active, hue_in + np.clip(turn, -limit, limit), hue_out)
    a = chroma_out * np.cos(hue_new)
    b = chroma_out * np.sin(hue_new)
    axis_a = np.array([1.0, -0.5, -0.5]) / np.sqrt(1.5)
    axis_b = np.array([0.0, np.sqrt(0.75), -np.sqrt(0.75)]) / np.sqrt(1.5)
    rebuilt = grey_out[..., None] + a[..., None] * axis_a + b[..., None] * axis_b
    print(f"hue guard turned back {int(active.sum())} of {active.size} entries", flush=True)
    return np.clip(np.where(active[..., None], rebuilt, table), 0.0, 1.0)


def accumulate(pairs: list, weights: list, size: int) -> tuple:
    """Averages the change each colour needs, how much was seen of it, and how
    far the images disagree about it.

    The disagreement matters as much as the average. Where every image asks for
    the same change, the table can apply it in full and be right for all of
    them. Where they ask for different things — and measured on this corpus
    they often do, by more than ten counts — any single answer is wrong for
    someone, and applying it in full makes the frame that disagrees visibly
    worse. A slow drift of exactly that kind is what drew an arc across one
    clear sky.
    """
    total = np.zeros((size, size, size, 3))
    square = np.zeros((size, size, size, 3))
    weight = np.zeros((size, size, size))
    for (ours, target), sample_weight in zip(pairs, weights):
        index = np.clip((ours * (size - 1)).round().astype(int), 0, size - 1)
        change = (target - ours) * sample_weight
        np.add.at(total, (index[:, 0], index[:, 1], index[:, 2]), change)
        np.add.at(square, (index[:, 0], index[:, 1], index[:, 2]),
                  change * (target - ours))
        np.add.at(weight, (index[:, 0], index[:, 1], index[:, 2]), sample_weight)
    safe = np.maximum(weight, 1e-9)[..., None]
    delta = np.where(weight[..., None] > 0, total / safe, 0.0)
    spread = np.sqrt(np.maximum(square / safe - delta * delta, 0.0)).mean(axis=-1)
    return delta, weight, spread


def main() -> int:
    preset_name = sys.argv[1]
    size = int(sys.argv[2]) if len(sys.argv) > 2 and sys.argv[2].isdigit() else 17
    with_raws = "--with-raws" in sys.argv
    best_path = WORK / preset_name / "best.json"
    work = WORK / preset_name / "table"
    work.mkdir(parents=True, exist_ok=True)

    document = json.loads(best_path.read_text())
    document["settings"].pop("color_table", None)
    staged = work / "without-table.json"
    staged.write_text(json.dumps(document))

    def measure(preset: Path, keys: list, folder: str = "") -> list:
        def one(key):
            source = source_for(key)
            output = work / f"{key}.jpeg"
            render(preset, source, output)
            probe = subprocess.run(["magick", "identify", "-format", "%w %h", str(output)],
                                   capture_output=True, text=True, check=True).stdout.split()
            reference = next((TARGETS / preset_name).glob(f"{key}.*"))
            return (pixels(output, int(probe[0]), int(probe[1])),
                    pixels(reference, int(probe[0]), int(probe[1])))
        with ThreadPoolExecutor(max_workers=9) as pool:
            return list(pool.map(one, keys))

    pairs = measure(staged, TUNING, "jpeg")
    weights = [1.0] * len(pairs)
    raws = []
    if with_raws:
        raws = measure(staged, RAW_TUNING, "raw")
        pairs = pairs + raws
        weights += [RAW_WEIGHT] * len(raws)

    delta, weight, spread = accumulate(pairs, weights, size)
    # The spread is measured and reported but deliberately not used to weight
    # the correction. Scaling the table by a confidence that itself varies from
    # cell to cell puts the confidence field's own shape into the picture, and
    # tried here it turned a faint arc into a pronounced one.
    print(f"images disagree by {spread[weight >= 40].mean() * 255:.1f} counts "
          f"on average where they overlap", flush=True)
    print(f"{int((weight >= 40).sum())} of {size ** 3} entries carry measurements",
          flush=True)

    axis = np.arange(size) / (size - 1)
    identity = np.stack(np.meshgrid(axis, axis, axis, indexing="ij"), axis=-1)

    def install(field: np.ndarray) -> dict:
        table = guard_hue(identity, np.clip(identity + field, 0.0, 1.0))
        result = json.loads(json.dumps(document))
        result["settings"]["color_table"] = {
            "size": size,
            "entries": [round(float(value), 6) for value in table.reshape(-1)],
            "strength": 100.0,
        }
        return result

    def residual(candidate: dict) -> tuple:
        path = work / "candidate.json"
        path.write_text(json.dumps(candidate))
        got = measure(path, TUNING, "jpeg")
        error = float(np.mean([np.abs(target - ours).mean()
                               for ours, target in got])) * 255.0
        return error, got

    # The correction is then measured through the real renderer and re-solved
    # from what is actually left, because the first solve assumes the table is
    # the only thing between our rendering and the reference, and it is not.
    best_field = solve_smooth_grid(delta, weight, size)
    candidate = best_field
    best_error = None
    for attempt in range(4):
        error, got = residual(install(candidate))
        if best_error is None or error < best_error - 1e-3:
            print(f"  pass {attempt}: {error:.2f} counts", flush=True)
            best_error, best_field = error, candidate
        else:
            print(f"  pass {attempt}: {error:.2f} counts, worse than "
                  f"{best_error:.2f} — keeping the previous table", flush=True)
            break
        remaining, remaining_weight, remaining_spread = accumulate(
            got + raws, [1.0] * len(got) + [RAW_WEIGHT] * len(raws), size)
        candidate = best_field + solve_smooth_grid(remaining, remaining_weight, size)

    output = Path(os.environ["TABLE_OUT"]) if os.environ.get("TABLE_OUT") else best_path
    output.write_text(json.dumps(install(best_field), indent=1))
    print(f"final: {best_error:.2f} counts")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
