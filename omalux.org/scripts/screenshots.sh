#!/usr/bin/env bash
set -euo pipefail
script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd -- "$script_dir/../.." && pwd)"
if [[ ${1:-} == --help || ${1:-} == -h ]]; then
  echo 'Usage: screenshots.sh [INPUT_PHOTO] — capture the current filters and presets views'
  exit 0
fi
(( $# <= 1 )) || { echo 'Expected at most one input photo' >&2; exit 2; }
input="${1:-$repo_root/assets/images/beach-volleyball.jpg}"
"$script_dir/screenshot.sh" "$input" "$repo_root/omalux.org/public/app-screenshot-dark.png" filters
"$script_dir/screenshot.sh" "$input" "$repo_root/omalux.org/public/app-presets-dark.png" presets
