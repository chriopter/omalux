#!/usr/bin/env bash
set -euo pipefail
script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd -- "$script_dir/../.." && pwd)"
if [[ ${1:-} == --help || ${1:-} == -h ]]; then
  echo "Usage: screenshots.sh [INPUT_PHOTO] — capture filters and expanded presets in light and dark"
  exit 0
fi
if (( $# > 1 )); then
  echo "Usage: $0 [INPUT_PHOTO]" >&2
  exit 2
fi
input="${1:-$repo_root/reference pictures/main.jpg}"
"$script_dir/screenshot.sh" "$input" "$repo_root/omalux.org/public/app-screenshot.png" light
"$script_dir/screenshot.sh" "$input" "$repo_root/omalux.org/public/app-screenshot-dark.png" dark
"$script_dir/screenshot.sh" "$input" "$repo_root/omalux.org/public/app-presets.png" light presets
"$script_dir/screenshot.sh" "$input" "$repo_root/omalux.org/public/app-presets-dark.png" dark presets
