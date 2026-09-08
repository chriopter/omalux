#!/usr/bin/env python3
"""Full-image acceptance audit for one preset.

The metric average hides exactly the faults that are obvious to a person: a
red channel sitting too high across every saturated area, a picture that is
uniformly flatter than the reference, a black point that never reaches black.
A single sampled strip hides them too — a strip through the top of a sky can
match while the rest of that same sky is off by twice as much.

So every image is measured whole, in tiles, per channel, and the worst tile is
what counts. The audit fails loudly rather than reporting an average that
looks acceptable.
"""

from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path

import numpy as np

from common import TARGETS, WORK


SIZE = 600
TILE = 100

# A tile is called out when a single channel is off by more than this many
# 8-bit counts. Twelve is roughly where a side-by-side comparison starts to
# show a colour cast rather than a difference in exposure.
CHANNEL_TOLERANCE = 12.0
# Contrast is compared as a ratio so it does not depend on the image's own
# brightness. Below this the picture reads as washed out.
CONTRAST_TOLERANCE = 0.90


def load(path: Path) -> np.ndarray:
    raw = subprocess.run(
        ["magick", str(path), "-resize", f"{SIZE}x{SIZE}!", "-depth", "8", "rgb:-"],
        capture_output=True, check=True).stdout
    return np.frombuffer(raw, dtype=np.uint8).reshape(SIZE, SIZE, 3).astype(np.float32)


def remove_global_offset(reference: np.ndarray, ours: np.ndarray) -> np.ndarray:
    """Rescales each channel so the frame's overall level matches.

    A single preset cannot follow a per-image decision. Where the reference
    chose half a stop darker for one frame, every region of ours is off by the
    same amount, and that one decision then dominates every regional figure and
    hides whatever else is wrong. Taking the per-channel level out separates
    what a preset controls — colour relationships, contrast, local behaviour —
    from what only a per-image adjustment could fix.
    """
    scale = reference.reshape(-1, 3).mean(0) / np.maximum(ours.reshape(-1, 3).mean(0), 1e-6)
    return np.clip(ours * scale, 0.0, 255.0)


def audit_pair(reference: np.ndarray, ours: np.ndarray) -> dict:
    findings = []
    levelled = remove_global_offset(reference, ours)
    levelled_worst = 0.0
    for top in range(0, SIZE, TILE):
        for left in range(0, SIZE, TILE):
            delta = (levelled[top:top + TILE, left:left + TILE].reshape(-1, 3).mean(0)
                     - reference[top:top + TILE, left:left + TILE].reshape(-1, 3).mean(0))
            levelled_worst = max(levelled_worst, float(np.abs(delta).max()))

    # Per-channel bias over the whole frame, and the worst single tile.
    bias = (ours - reference).reshape(-1, 3).mean(0)
    worst_tile = None
    for top in range(0, SIZE, TILE):
        for left in range(0, SIZE, TILE):
            tile_reference = reference[top:top + TILE, left:left + TILE].reshape(-1, 3)
            tile_ours = ours[top:top + TILE, left:left + TILE].reshape(-1, 3)
            delta = tile_ours.mean(0) - tile_reference.mean(0)
            magnitude = float(np.abs(delta).max())
            if worst_tile is None or magnitude > worst_tile["magnitude"]:
                worst_tile = {
                    "magnitude": magnitude,
                    "at": (top, left),
                    "delta": delta.round(1).tolist(),
                    "reference": tile_reference.mean(0).round(1).tolist(),
                    "ours": tile_ours.mean(0).round(1).tolist(),
                }
    if worst_tile["magnitude"] > CHANNEL_TOLERANCE:
        findings.append(
            f"tile at {worst_tile['at']} off by {worst_tile['delta']} "
            f"(reference {worst_tile['reference']}, ours {worst_tile['ours']})")

    # Overall contrast, and separately whether the darks and lights land.
    contrast = float(ours.std() / reference.std())
    if contrast < CONTRAST_TOLERANCE:
        findings.append(f"flat: contrast {contrast:.2f} of the reference")

    reference_black = float(np.percentile(reference, 1))
    ours_black = float(np.percentile(ours, 1))
    reference_white = float(np.percentile(reference, 99))
    ours_white = float(np.percentile(ours, 99))
    if abs(ours_black - reference_black) > CHANNEL_TOLERANCE:
        findings.append(f"black point {ours_black:.0f} against {reference_black:.0f}")
    if abs(ours_white - reference_white) > CHANNEL_TOLERANCE:
        findings.append(f"white point {ours_white:.0f} against {reference_white:.0f}")

    # Saturated colour is where a raised weakest channel shows first.
    chroma = reference.max(axis=2) - reference.min(axis=2)
    saturated = chroma >= np.quantile(chroma, 0.90)
    if saturated.any():
        delta = ours[saturated].mean(0) - reference[saturated].mean(0)
        if float(np.abs(delta).max()) > CHANNEL_TOLERANCE:
            findings.append(
                f"saturated areas off by {delta.round(1).tolist()} "
                f"(reference {reference[saturated].mean(0).round(1).tolist()}, "
                f"ours {ours[saturated].mean(0).round(1).tolist()})")

    return {
        "bias": bias.round(1).tolist(),
        "contrast": round(contrast, 3),
        "worst_tile": round(worst_tile["magnitude"], 1),
        "worst_tile_levelled": round(levelled_worst, 1),
        "exposure_stops": round(float(np.log2(
            max(reference.mean(), 1e-6) / max(ours.mean(), 1e-6))), 2),
        "findings": findings,
    }


def main() -> int:
    preset = sys.argv[1]
    rendered = WORK / preset / "validate"
    references = TARGETS / preset
    results = {}
    for path in sorted(rendered.glob("*.jpeg")):
        identifier = path.stem
        reference_path = next(references.glob(f"{identifier}.*"), None)
        if reference_path is None:
            continue
        results[identifier] = audit_pair(load(reference_path), load(path))

    failing = {k: v for k, v in results.items() if v["findings"]}
    for identifier, result in sorted(
            results.items(), key=lambda item: -item[1]["worst_tile"]):
        mark = "FAIL" if result["findings"] else "ok  "
        print(f"{mark} {identifier}  bias {result['bias']}  "
              f"contrast {result['contrast']}  worst tile {result['worst_tile']}"
              f"  levelled {result['worst_tile_levelled']}"
              f"  exposure {result['exposure_stops']:+.2f}")
        for finding in result["findings"]:
            print(f"       {finding}")

    print(f"\n{len(failing)} of {len(results)} images have findings")
    (WORK / preset / "audit.json").write_text(
        json.dumps(results, indent=1))
    return 1 if failing else 0


if __name__ == "__main__":
    raise SystemExit(main())
