#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd -- "$script_dir/../.." && pwd)"

if [[ ${1:-} == --help || ${1:-} == -h ]]; then
  cat <<'HELP'
Usage: screenshot.sh [INPUT_PHOTO [OUTPUT_PNG [current|light|dark]]]

Build the current Qt app and capture it at 1440x920 without opening a desktop
window. Defaults: repository beach image → omalux.org/public/app-screenshot.png.
Relative arguments resolve from your current directory. Uses your current app
theme and installed fonts. Requires the GUI build dependencies and GNU timeout.
The previous output is preserved if building or capturing fails.
HELP
  exit 0
fi
if (( $# > 3 )); then
  echo "Usage: $0 [INPUT_PHOTO [OUTPUT_PNG [current|light|dark]]]" >&2
  exit 2
fi

input="$(realpath -- "${1:-$repo_root/reference pictures/main.jpg}")"
output="$(realpath -m -- "${2:-$repo_root/omalux.org/public/app-screenshot.png}")"
theme="${3:-current}"
case "$theme" in current|light|dark) ;; *) echo "Theme must be current, light or dark" >&2; exit 2 ;; esac
[[ -f "$input" ]] || { echo "Photo not found: $input" >&2; exit 2; }
[[ "$output" == *.png ]] || { echo "Output must end in .png" >&2; exit 2; }
[[ "$input" != "$output" ]] || { echo "Input and output must differ" >&2; exit 2; }
command -v timeout >/dev/null
command -v cargo >/dev/null
command -v python3 >/dev/null

cd -- "$repo_root"
cargo build -p omalux-gui --example website_screenshot
target_dir="$(cargo metadata --no-deps --format-version 1 | python3 -c 'import json,sys; print(json.load(sys.stdin)["target_directory"])')"
mkdir -p -- "$(dirname -- "$output")"
temporary="$(mktemp -- "$(dirname -- "$output")/.app-screenshot.XXXXXX.png")"
trap 'rm -f -- "$temporary"' EXIT

QT_QPA_PLATFORM=offscreen QT_QPA_PLATFORMTHEME= QT_QUICK_CONTROLS_STYLE=Basic \
  QT_QUICK_BACKEND=software QT_SCALE_FACTOR=1 \
  QT_FORCE_STDERR_LOGGING=1 QT_LOGGING_RULES='*.warning=true;*.critical=true;qml.debug=true' \
  timeout 60s "$target_dir/debug/examples/website_screenshot" \
  --input "$input" --capture "$temporary" --grain 0 --theme "$theme"
[[ -s "$temporary" ]] || { echo "Screenshot is empty" >&2; exit 1; }
chmod 644 -- "$temporary"
mv -- "$temporary" "$output"
printf 'Screenshot saved: %s\n' "$output"
