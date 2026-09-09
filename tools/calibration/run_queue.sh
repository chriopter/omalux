#!/usr/bin/env bash
# Run the full chain (cube fit, one slider pass, final scoring, report) for
# several presets, a few at a time.
#
#   tools/calibration/run_queue.sh [-j 3] [preset ...]
#
# Without preset ids every bundled preset that has target renderings is
# queued; presets whose work/<preset>/final.json already exists are skipped.
# Each job runs with DT_JOBS render workers (default 5); with -j 3 that is
# 15 darktable-cli processes at a time, about right for 16 cores.
# Logs: work/queue/<preset>.log, progress in work/queue/progress.log.
set -euo pipefail
: "${OMALUX_CALIBRATION_ROOT:?set OMALUX_CALIBRATION_ROOT}"
here="$(cd "$(dirname "$0")" && pwd)"
parallel=3
if [ "${1:-}" = "-j" ]; then parallel="$2"; shift 2; fi
export DT_JOBS="${DT_JOBS:-5}"
queue="$OMALUX_CALIBRATION_ROOT/work/queue"
mkdir -p "$queue"

if [ $# -gt 0 ]; then
  printf '%s\n' "$@" > "$queue/queue.txt"
else
  python3 - "$here" > "$queue/queue.txt" <<'EOF'
import sys
sys.path.insert(0, sys.argv[1])
from common import WORK, preset_dirs
for pid in sorted(preset_dirs()):
    if not (WORK / pid / "final.json").exists():
        print(pid)
EOF
fi
echo "queued: $(wc -l < "$queue/queue.txt") presets, $parallel at a time"

one() {
  p="$1"
  {
    python3 "$here/cube_fit.py" fit "$p" --iters 15
    python3 "$here/slider_tune.py" "$p" --passes 1
    python3 "$here/cube_fit.py" final "$p"
    flock "$queue/report.lock" python3 "$here/report.py"
  } > "$queue/$p.log" 2>&1 || echo "$p FAILED $(date +%H:%M)" >> "$queue/progress.log"
  echo "$p done $(date +%H:%M)" >> "$queue/progress.log"
}
export -f one
export here queue
xargs -P "$parallel" -I{} bash -c 'one {}' < "$queue/queue.txt"
echo "ALLDONE $(date +%H:%M)" >> "$queue/progress.log"
