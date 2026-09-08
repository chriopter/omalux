#!/usr/bin/env python3
"""Measures whether a sky is as smooth as the reference's.

A clear sky is very nearly a plane: brightness varies steadily with height and
a little with direction, and nothing else. That makes it the one place in a
photograph where a gentle, broad deviation is unmistakable — the arc a fitted
colour table drew across one of these frames was invisible to every average,
every tile, and every colour difference, because within any small area it is
tiny. Against a fitted plane it is obvious.

So each sky is compared with the plane that best describes it, and the leftover
is compared with the reference's own leftover. A picture that departs from its
plane much more than the reference does has something in it that the reference
does not.
"""

from __future__ import annotations

import subprocess
import sys
from pathlib import Path

import numpy as np

from common import TARGETS, WORK, skies


WIDTH, HEIGHT = 600, 400
# Frames with a large clear sky, and the fraction of the frame it occupies:
# `datasets/skies.json` in the calibration root.
SKIES = skies()


def load(path: Path) -> np.ndarray:
    raw = subprocess.run(
        ["magick", str(path), "-resize", f"{WIDTH}x{HEIGHT}!", "-depth", "8", "rgb:-"],
        capture_output=True, check=True).stdout
    return np.frombuffer(raw, dtype=np.uint8).reshape(HEIGHT, WIDTH, 3).astype(np.float64)


def departure_from_plane(sky: np.ndarray) -> float:
    """Root mean square of what a plane through the sky cannot account for."""
    rows, columns, _ = sky.shape
    y, x = np.mgrid[0:rows, 0:columns]
    design = np.column_stack([np.ones(rows * columns), x.ravel() / columns, y.ravel() / rows])
    total = 0.0
    for channel in range(3):
        observed = sky[..., channel].ravel()
        coefficients, *_ = np.linalg.lstsq(design, observed, rcond=None)
        total += float(np.sqrt(np.mean((observed - design @ coefficients) ** 2)))
    return total / 3.0


def main() -> int:
    preset = sys.argv[1]
    rendered = WORK / preset / "validate"
    worst = 0.0
    print("Bild   Bogenmass")
    for key, share in SKIES.items():
        ours_path = rendered / f"{key}.jpeg"
        reference_path = next((TARGETS / preset).glob(f"{key}.*"), None)
        if not ours_path.exists() or reference_path is None:
            continue
        rows = int(HEIGHT * share)
        # The difference between the two skies is measured, not either sky on
        # its own. Whatever the reference itself contains — clouds, haze — is
        # in both and cancels. A preset that shifts the whole sky leaves a
        # plane behind, which is subtracted too. What survives is structure
        # that only one of the two has, which is exactly the fault being
        # looked for and nothing else.
        difference = load(ours_path)[:rows] - load(reference_path)[:rows]
        value = departure_from_plane(difference)
        worst = max(worst, value)
        mark = "  <== Bogen" if value > 2.0 else ""
        print(f"{key:5s}  {value:9.2f}{mark}")
    print(f"\nhoechstes Bogenmass {worst:.2f} (ab etwa 2.0 sichtbar)")
    return 1 if worst > 2.0 else 0


if __name__ == "__main__":
    raise SystemExit(main())
