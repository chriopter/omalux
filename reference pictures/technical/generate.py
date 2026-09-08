#!/usr/bin/env python3
"""Regenerate the fixed sRGB 16-bit RGB PNG fixtures using Python's stdlib."""

from array import array
import colorsys
import hashlib
import json
from pathlib import Path
import struct
import sys
import zlib

ROOT = Path(__file__).resolve().parent
MAX = 65535


def chunk(kind, data):
    return (struct.pack(">I", len(data)) + kind + data
            + struct.pack(">I", zlib.crc32(kind + data)))


def write_png(name, width, height, pixel, description):
    raw = bytearray()
    for y in range(height):
        row = array("H")
        for x in range(width):
            rgb = pixel(x, y)
            if isinstance(rgb, (int, float)):
                rgb = (rgb,) * 3
            row.extend(max(0, min(MAX, int(v * MAX + 0.5))) for v in rgb)
        if sys.byteorder == "little":
            row.byteswap()
        raw.append(0)  # PNG filter: None; keep generation straightforward.
        raw.extend(row.tobytes())
    data = b"\x89PNG\r\n\x1a\n"
    data += chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 16, 2, 0, 0, 0))
    data += chunk(b"sRGB", b"\x00")
    data += chunk(b"IDAT", zlib.compress(raw, 9))
    data += chunk(b"IEND", b"")
    (ROOT / name).write_bytes(data)
    return dict(file=name, width=width, height=height, bit_depth=16,
                color_space="sRGB", description=description,
                scanlines_sha256=hashlib.sha256(raw).hexdigest(),
                file_sha256=hashlib.sha256(data).hexdigest())


def gray(x, y):
    return x / 1023 if y < 320 else (x // 32) / 31


def extremes(x, y):
    t = x / 1023 if y % 320 < 160 else (x // 32) / 31
    return t * 0.08 if y < 320 else 0.92 + t * 0.08


PRIMARIES = [(1, 0, 0), (0, 1, 0), (0, 0, 1),
             (0, 1, 1), (1, 0, 1), (1, 1, 0)]


def channels(x, y):
    color = PRIMARIES[y // 128]
    t = x / 1023
    return tuple(c * t if y % 128 < 64 else c + (1 - c) * t for c in color)


def hue_saturation(x, y):
    return colorsys.hsv_to_rgb(x / 1024, (y % 256) / 255, (1, 0.5, 0.125)[y // 256])


PATCHES = (
    [(v, v, v) for v in (0, 8, 32, 64, 128, 192, 247, 255)]
    + [(255, 0, 0), (255, 128, 0), (255, 255, 0), (0, 255, 0),
       (0, 255, 255), (0, 0, 255), (128, 0, 255), (255, 0, 255)]
    + [(255, 192, 192), (255, 224, 192), (255, 255, 192), (192, 255, 192),
       (192, 255, 255), (192, 192, 255), (224, 192, 255), (255, 192, 255)]
    + [(96, 64, 48), (176, 120, 88), (224, 176, 144), (208, 184, 128),
       (64, 96, 48), (48, 112, 96), (64, 112, 176), (112, 64, 128)]
)


def patches(x, y):
    return tuple(c / 255 for c in PATCHES[(y // 128) * 8 + x // 128])


def detail(x, y):
    col, row = x // 384, y // 384
    u, v = x % 384, y % 384
    if row == 0:
        if col == 0:  # Vertical edge, no clipping headroom lost.
            return 0.1 if u < 192 else 0.9
        if col == 1:  # Slanted edge.
            return 0.1 if u < 96 + v / 2 else 0.9
        # Six bands with 1, 2, 4, 8, 16, 32-pixel vertical bars.
        return 0.1 if (u // (1 << (v // 64))) % 2 == 0 else 0.9
    if col == 0:
        return 0.1 if (u // 8 + v // 8) % 2 == 0 else 0.9
    if col == 1:
        return 0.5
    # Fixed coordinate hash, independent of Python's random implementation.
    n = ((u + v * 384 + 20260908) * 747796405 + 2891336453) & 0xFFFFFFFF
    n = (((n >> ((n >> 28) + 4)) ^ n) * 277803737) & 0xFFFFFFFF
    n = (n >> 22) ^ n
    return 0.5 + ((n & 65535) / MAX - 0.5) * 0.1


def main():
    specs = [
        ("01-grayscale.png", 1024, 640, gray,
         "Top: continuous encoded sRGB 0–1 ramp. Bottom: 32 equal-width levels 0–1."),
        ("02-shadows-highlights.png", 1024, 640, extremes,
         "Four strips: smooth 0–0.08; 32 levels 0–0.08; smooth 0.92–1; 32 levels 0.92–1."),
        ("03-channel-ramps.png", 1024, 768, channels,
         "Six pairs, red/green/blue/cyan/magenta/yellow: black to color, then color to white."),
        ("04-hue-saturation.png", 1024, 768, hue_saturation,
         "HSV hue left to right [0,1), saturation top to bottom in each band [0,1]; V=1,0.5,0.125. HSV computed in encoded sRGB, not perceptual lightness."),
        ("05-color-patches.png", 1024, 512, patches,
         "8 columns × 4 rows: grays, saturated colors, pastels, earthy colors. Exact 8-bit-equivalent RGB triplets in manifest; not a calibrated physical chart."),
        ("06-spatial-detail.png", 1152, 768, detail,
         "2 rows × 3 columns. Top: vertical edge, slanted edge, bars at six widths. Bottom: 8px checkerboard, uniform 0.5, fixed monochrome noise in [0.45,0.55]."),
    ]
    manifest = dict(version=1, encoding="16-bit RGB PNG; sRGB chunk; round-half-up; no dithering",
                    scope="Display-referred inputs, not linear HDR or RAW decoder fixtures.",
                    patches_rgb8=PATCHES,
                    images=[write_png(*spec) for spec in specs])
    (ROOT / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
    print(f"Generated {len(specs)} fixtures in {ROOT}")


if __name__ == "__main__":
    main()
