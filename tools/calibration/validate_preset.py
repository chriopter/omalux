#!/usr/bin/env python3
"""Full-resolution validation of an optimized preset on all tuning images."""

from __future__ import annotations

import argparse
from common import BINARY, DATASET, TARGETS, WORK, images, normalize_image
from metrics import pair_metrics
import json
import statistics
import subprocess
import sys
from concurrent.futures import ThreadPoolExecutor





def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("preset")
    parser.add_argument("--jobs", type=int, default=6)
    args = parser.parse_args()

    best = WORK / args.preset / "best.json"
    out = WORK / args.preset / "validate"
    out.mkdir(parents=True, exist_ok=True)
    cache = out / "cache"
    cache.mkdir(exist_ok=True)
    tuning = images("tuning")

    def one(item):
        output = out / f"{item['id']}.jpeg"
        result = subprocess.run(
            [str(BINARY), "develop", "--input", str(DATASET / item["path"]),
             "--preset-file", str(best), "--output", str(output), "--overwrite",
             "--format", "jpeg", "--quality", "90", "--unprofiled", "assume-srgb",
             "--metadata", "strip-location", "--alpha", "reject",
             "--max-source-bytes", "67108864", "--max-pixels", "64000000",
             "--max-working-bytes", "8589934592", "--max-output-bytes", "536870912",
             "--progress", "none"],
            stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=600, check=False)
        if result.returncode:
            return item["id"], None
        ref = next((TARGETS / args.preset).glob(f"{item['id']}.*"))
        a, _, _ = normalize_image(ref, cache / f"ref-{ref.name}.px", 120)
        for stale_suffix in (".px", ".json", ".rgb"):
            (cache / f"ren-{item['id']}{stale_suffix}").unlink(missing_ok=True)
        b, _, _ = normalize_image(output, cache / f"ren-{item['id']}.px", 120)
        m = pair_metrics(a, b)
        return item["id"], m

    rows = []
    with ThreadPoolExecutor(max_workers=args.jobs) as pool:
        for iid, m in pool.map(one, tuning):
            rows.append((iid, m))
    ok = [m for _, m in rows if m]
    result = {
        "preset": args.preset,
        "n": len(ok),
        "delta_e_mean": statistics.fmean(m["delta_e_2000_mean"] for m in ok),
        "ssim_mean": statistics.fmean(m["ssim_global"] for m in ok),
        "per_image": {iid: ({"delta_e": m["delta_e_2000_mean"], "ssim": m["ssim_global"]}
                            if m else None) for iid, m in rows},
    }
    (WORK / args.preset / "validation.json").write_text(
        json.dumps(result, indent=1))
    print(f"{args.preset}: mean ΔE {result['delta_e_mean']:.3f}  SSIM {result['ssim_mean']:.4f}  n={result['n']}")
    for iid, m in sorted(rows, key=lambda r: -(r[1]["delta_e_2000_mean"] if r[1] else 99)):
        print(f"  {iid} ΔE={m['delta_e_2000_mean']:6.2f} ssim={m['ssim_global']:.3f}" if m else f"  {iid} FAILED")
    return 0


if __name__ == "__main__":
    sys.exit(main())
