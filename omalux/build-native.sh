#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
mkdir -p omalux/build
library="$(realpath "${DARKTABLE_LIBRARY:-/usr/lib/darktable/libdarktable.so}")"
version="$(python3 - "$library" <<'PY'
import ctypes, re, sys
lib = ctypes.CDLL(sys.argv[1], mode=ctypes.RTLD_GLOBAL)
version = ctypes.string_at(ctypes.addressof(ctypes.c_char.in_dll(lib, 'darktable_package_version'))).decode()
if version not in ('5.6.0', '5.6.1'):
    raise SystemExit('Native adapter currently supports darktable 5.6.0/5.6.1; found ' + version)
print(version)
PY
)"
# Internal struct layouts differ even between patch releases: compile against
# the loaded library's exact release, without changing the submodule checkout.
tag="release-$version"
if ! git -C darktable cat-file -e "$tag:src" 2>/dev/null; then
  git -C darktable fetch --depth 1 https://github.com/darktable-org/darktable.git "tag" "$tag"
fi
headers="$PWD/omalux/build/headers-$version"
if [[ ! -d "$headers/src" ]]; then
  mkdir -p "$headers"
  git -C darktable archive "$tag" src | tar -x -C "$headers"
fi
cc -O2 -fPIC -D_RELEASE -DHAVE_OPENCL -DCL_TARGET_OPENCL_VERSION=300 -fopenmp \
  -DOMALUX_DT_VERSION="\"$version\"" \
  -I"$headers/src" -I"$headers/src/external" -Idarktable/src/external/OpenCL \
  $(pkg-config --cflags gtk+-3.0 json-glib-1.0 lcms2 sqlite3 lua librsvg-2.0) \
  -c omalux/native/engine.c -o omalux/build/engine.o
/usr/lib/qt6/moc omalux/native/main.cpp -o omalux/build/main.moc
c++ -std=c++20 -O2 -fPIC -pthread -Iomalux/build \
  -DOMALUX_QML="\"$PWD/omalux/ui/Main.qml\"" \
  $(pkg-config --cflags Qt6Quick Qt6QuickControls2) \
  omalux/native/main.cpp omalux/build/engine.o "$library" \
  $(pkg-config --libs Qt6Quick Qt6QuickControls2 gtk+-3.0 json-glib-1.0) -fopenmp \
  -Wl,-rpath,"$(dirname "$library")" -o omalux/build/omalux
printf '%s\n' "$library" > omalux/build/library-path
