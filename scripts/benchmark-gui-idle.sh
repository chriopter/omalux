#!/usr/bin/env bash
set -euo pipefail

binary="${OMALUX_GUI_BIN:-target/release/omalux-gui}"
platform="${OMALUX_QPA_PLATFORM:-offscreen}"
settle_seconds="${SETTLE_SECONDS:-10}"
sample_seconds="${SAMPLE_SECONDS:-30}"
maximum_cpu="${MAXIMUM_CPU_PERCENT:-1.0}"
input=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --binary) binary="$2"; shift 2 ;;
    --platform) platform="$2"; shift 2 ;;
    --input) input="$2"; shift 2 ;;
    --settle) settle_seconds="$2"; shift 2 ;;
    --duration) sample_seconds="$2"; shift 2 ;;
    --max-cpu) maximum_cpu="$2"; shift 2 ;;
    *) echo "unknown option: $1" >&2; exit 2 ;;
  esac
done

if [[ ! -x "$binary" ]]; then
  echo "GUI binary is not executable: $binary" >&2
  echo "Build it first with: cargo build --release -p omalux-gui" >&2
  exit 2
fi

arguments=()
if [[ -n "$input" ]]; then
  arguments+=(--input "$input")
fi

QT_QPA_PLATFORM="$platform" "$binary" "${arguments[@]}" >/dev/null 2>&1 &
pid=$!
cleanup() {
  if kill -0 "$pid" 2>/dev/null; then
    kill -TERM "$pid" 2>/dev/null || true
  fi
  wait "$pid" 2>/dev/null || true
}
trap cleanup EXIT INT TERM

sleep "$settle_seconds"
if ! kill -0 "$pid" 2>/dev/null; then
  echo "GUI exited during the settle period" >&2
  exit 1
fi

read_cpu_ticks() {
  awk '{print $14 + $15}' "/proc/$pid/stat"
}

read_context_switches() {
  awk '
    $1 == "voluntary_ctxt_switches:" { voluntary += $2 }
    $1 == "nonvoluntary_ctxt_switches:" { involuntary += $2 }
    END { print voluntary + 0, involuntary + 0 }
  ' /proc/"$pid"/status
}

start_ticks=$(read_cpu_ticks)
read -r start_voluntary start_involuntary < <(read_context_switches)
start_ns=$(date +%s%N)
sleep "$sample_seconds"
end_ticks=$(read_cpu_ticks)
read -r end_voluntary end_involuntary < <(read_context_switches)
end_ns=$(date +%s%N)

clock_ticks=$(getconf CLK_TCK)
cpu_percent=$(awk \
  -v ticks="$((end_ticks - start_ticks))" \
  -v hz="$clock_ticks" \
  -v nanoseconds="$((end_ns - start_ns))" \
  'BEGIN { printf "%.3f", 100 * ticks / hz / (nanoseconds / 1000000000) }')
set -- /proc/"$pid"/task/[0-9]*
thread_count=$#
child_count=$({ ps -o pid= --ppid "$pid" || true; } \
  | awk 'NF { count++ } END { print count + 0 }')

printf 'idle_cpu_one_core_percent=%s threads=%d children=%d main_thread_voluntary_context_switches=%d main_thread_nonvoluntary_context_switches=%d\n' \
  "$cpu_percent" "$thread_count" "$child_count" \
  "$((end_voluntary - start_voluntary))" \
  "$((end_involuntary - start_involuntary))"

if [[ "$child_count" -ne 0 ]]; then
  echo "FAIL: idle GUI retains child processes" >&2
  exit 1
fi
if ! awk -v actual="$cpu_percent" -v maximum="$maximum_cpu" \
  'BEGIN { exit !(actual < maximum) }'; then
  echo "FAIL: idle CPU ${cpu_percent}% is not below ${maximum_cpu}% of one core" >&2
  exit 1
fi

echo "PASS: idle CPU is below ${maximum_cpu}% of one core"
