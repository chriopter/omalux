#!/usr/bin/env bash
# Copies calibrated presets from the work directory into presets/builtin in
# the canonical form the catalogue requires, then runs the catalogue test.
# Usage: install_presets.sh <preset-id>...
set -euo pipefail
cd "$(git -C "$(dirname "$0")" rev-parse --show-toplevel)"
: "${OMALUX_CALIBRATION_ROOT:?set OMALUX_CALIBRATION_ROOT}"
binary=${OMALUX_BINARY:-$OMALUX_CALIBRATION_ROOT/bin/omalux}
for preset in "$@"; do
  python3 - "$preset" "$OMALUX_CALIBRATION_ROOT" <<'PY'
import json, sys
preset, root = sys.argv[1:3]
best = json.load(open(f"{root}/work/{preset}/best.json"))
import glob
matches = glob.glob(f"presets/builtin/**/{preset}.json", recursive=True)
if not matches:
    raise SystemExit(f"no built-in preset file for {preset}")
target = matches[0]
current = json.load(open(target))
current["settings"] = best["settings"]
open(target, "w").write(json.dumps(current, separators=(",", ":")))
PY
  "$binary" presets canonicalize "$(find presets/builtin -name "$preset.json" | head -1)"
done
cargo test --test develop_catalog 2>&1 | grep 'test result'
