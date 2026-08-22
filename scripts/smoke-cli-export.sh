#!/usr/bin/env bash
set -euo pipefail

if [[ $# -lt 1 || $# -gt 2 ]]; then
  echo "usage: $0 INPUT_IMAGE [GRAINROOM_BINARY]" >&2
  exit 2
fi

input=$1
binary=${2:-./target/debug/grainroom}

if [[ ! -f $input ]]; then
  echo "input does not exist: $input" >&2
  exit 2
fi
if [[ ! -x $binary ]]; then
  echo "grainroom binary is not executable: $binary" >&2
  exit 2
fi

test_dir=$(mktemp -d /tmp/grainroom-smoke.XXXXXX)
trap 'rm -rf -- "$test_dir"' EXIT
output=$test_dir/export.heic

expected_geometry=$(magick identify -format '%wx%h' "$input")

"$binary" \
  --headless \
  --input "$input" \
  --output "$output" \
  --format heic \
  --quality 90 \
  --grain 24 \
  --grain-size 4000 \
  --midtones 100

actual_format=$(magick identify -format '%m' "$output")
actual_geometry=$(magick identify -format '%wx%h' "$output")
actual_mean=$(magick identify -format '%[fx:mean]' "$output")

if [[ $actual_format != HEIC ]]; then
  echo "expected HEIC, got $actual_format" >&2
  exit 1
fi
if [[ $actual_geometry != "$expected_geometry" ]]; then
  echo "expected $expected_geometry, got $actual_geometry" >&2
  exit 1
fi
if ! awk -v mean="$actual_mean" 'BEGIN { exit !(mean > 0.001) }'; then
  echo "export appears blank (mean=$actual_mean)" >&2
  exit 1
fi

magick "$output" -resize 8x8 null:
echo "ok: HEIC $actual_geometry ($(stat -c '%s bytes' "$output"))"
