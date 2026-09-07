"""Shared plumbing for the calibration tools.

Everything lives under one root directory, named by `OMALUX_CALIBRATION_ROOT`:

    datasets/manifest.json   the images, with `id`, `path` and `split`
    datasets/<path>          the image files the manifest points to
    targets/<preset>/<id>.*  the rendering each image should end up as
    work/<preset>/           what the tools write for one preset
    bin/omalux               the release build the tools render with

`OMALUX_BINARY` overrides the binary. The `split` of an image is `tuning`
or `holdout`; every tool here reads tuning images only, and the holdout is
touched by nothing but a final evaluation.
"""

from __future__ import annotations

import hashlib
import json
import os
import subprocess
import tempfile
import zlib
from pathlib import Path


def _root() -> Path:
    value = os.environ.get("OMALUX_CALIBRATION_ROOT")
    if not value:
        raise SystemExit("OMALUX_CALIBRATION_ROOT is not set")
    return Path(value).expanduser().resolve()


ROOT = _root()
DATASET = ROOT / "datasets"
TARGETS = ROOT / "targets"
WORK = ROOT / "work"
BINARY = Path(os.environ["OMALUX_BINARY"]) if os.environ.get("OMALUX_BINARY") else ROOT / "bin" / "omalux"

PROXY_SIZE = 256
MAX_NORMALIZED_BYTES = PROXY_SIZE * PROXY_SIZE * 3 + 4096
MIN_METRIC_DIMENSION = 32


def manifest() -> dict:
    return json.loads((DATASET / "manifest.json").read_text())


def images(split: str = "tuning") -> list:
    return [item for item in manifest()["items"] if item.get("split") == split]


def target_for(preset: str, image_id: str) -> Path | None:
    return next((TARGETS / preset).glob(f"{image_id}.*"), None)


def source_for(image_id: str) -> Path:
    for item in manifest()["items"]:
        if item["id"] == image_id:
            return DATASET / item["path"]
    raise KeyError(image_id)


def tuning_ids(media: str | None = None) -> list:
    """Ids of the tuning images, optionally only `raw` or only raster ones."""
    return [item["id"] for item in images("tuning")
            if media is None or (item.get("media_type") == "raw") == (media == "raw")]


def skies() -> dict:
    """Frames with a large clear sky and the share of the frame it occupies,
    from `datasets/skies.json` (`{"<id>": 0.3, ...}`). Empty when absent."""
    path = DATASET / "skies.json"
    return json.loads(path.read_text()) if path.exists() else {}


def builtin_preset(identifier: str) -> dict:
    """The built-in preset of that id, as the binary carries it."""
    out = subprocess.run([str(BINARY), "presets", "show", identifier, "--json"],
                         capture_output=True, text=True, check=True).stdout
    return json.loads(out)


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def atomic_bytes(path: Path, data: bytes) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    fd, tmp = tempfile.mkstemp(prefix=".tmp-", dir=path.parent)
    try:
        with os.fdopen(fd, "wb") as handle:
            handle.write(data)
            handle.flush()
            os.fsync(handle.fileno())
        os.replace(tmp, path)
    finally:
        try:
            os.unlink(tmp)
        except FileNotFoundError:
            pass


def run_captured(command: list, timeout: int, env=None):
    """Runs one process tree and terminates the whole group on timeout."""
    process = subprocess.Popen(command, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                               env=env, start_new_session=True)
    try:
        stdout, stderr = process.communicate(timeout=timeout)
    except subprocess.TimeoutExpired:
        try:
            os.killpg(process.pid, 15)
            process.communicate(timeout=2)
        except (ProcessLookupError, subprocess.TimeoutExpired):
            try:
                os.killpg(process.pid, 9)
            except ProcessLookupError:
                pass
            process.communicate()
        return 124, b"", b"timeout"
    return process.returncode, stdout, stderr


def ppm_decode(data: bytes) -> tuple:
    pos, tokens = 0, []
    while len(tokens) < 4:
        while pos < len(data) and data[pos] in b" \t\r\n":
            pos += 1
        if pos < len(data) and data[pos] == 35:
            while pos < len(data) and data[pos] not in b"\r\n":
                pos += 1
            continue
        start = pos
        while pos < len(data) and data[pos] not in b" \t\r\n":
            pos += 1
        if start == pos:
            raise ValueError("invalid PPM header")
        tokens.append(data[start:pos])
    if tokens[0] != b"P6" or tokens[3] != b"255":
        raise ValueError("unsupported PPM")
    width, height = int(tokens[1]), int(tokens[2])
    pixels = data[pos + 1:pos + 1 + width * height * 3]
    if len(pixels) != width * height * 3:
        raise ValueError("truncated PPM")
    return width, height, pixels


def png_encode(width: int, height: int, pixels: bytes) -> bytes:
    def chunk(kind, payload):
        return len(payload).to_bytes(4, "big") + kind + payload + zlib.crc32(kind + payload).to_bytes(4, "big")
    raw = b"".join(b"\0" + pixels[y * width * 3:(y + 1) * width * 3] for y in range(height))
    header = width.to_bytes(4, "big") + height.to_bytes(4, "big") + b"\x08\x02\x00\x00\x00"
    return b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", header) + chunk(b"IDAT", zlib.compress(raw, 9)) + chunk(b"IEND", b"")


def normalize_image(source: Path, cache: Path, timeout: int) -> tuple:
    """The 256x256 proxy every metric is measured on: fitted isotropically,
    centred on mid-grey, sRGB, 8 bit. Cached next to `cache` by content hash."""
    source_hash = sha256_file(source)
    meta = cache.with_suffix(".json")
    raw = cache.with_suffix(".rgb")
    if cache.is_file() and meta.is_file() and raw.is_file():
        old = json.loads(meta.read_text())
        if old.get("source_sha256") == source_hash and raw.stat().st_size == PROXY_SIZE * PROXY_SIZE * 3:
            return raw.read_bytes(), source_hash, old["alignment"]
    probe = ["magick", "identify", "-auto-orient", "-format", "%w\t%h", str(source)]
    code, stdout, stderr = run_captured(probe, timeout)
    if code != 0:
        raise RuntimeError(f"dimension probe failed: {stderr.decode(errors='replace')}")
    source_width, source_height = (int(value) for value in stdout.decode("ascii").split("\t"))
    if min(source_width, source_height) < MIN_METRIC_DIMENSION:
        raise RuntimeError("image too small to measure")
    scale = min(PROXY_SIZE / source_width, PROXY_SIZE / source_height)
    alignment = {"source_width": source_width, "source_height": source_height,
                 "fitted_width": max(1, min(PROXY_SIZE, round(source_width * scale))),
                 "fitted_height": max(1, min(PROXY_SIZE, round(source_height * scale))),
                 "canvas_width": PROXY_SIZE, "canvas_height": PROXY_SIZE,
                 "isotropic_scale": scale, "crop": "none", "gravity": "center"}
    command = ["magick", "-limit", "memory", "512MiB", "-limit", "map", "2GiB",
               "-define", "jpeg:size=2048x2048", str(source), "-auto-orient", "-alpha", "off",
               "-colorspace", "sRGB", "-filter", "Lanczos", "-resize", f"{PROXY_SIZE}x{PROXY_SIZE}",
               "-gravity", "center", "-background", "rgb(127,127,127)",
               "-extent", f"{PROXY_SIZE}x{PROXY_SIZE}", "-depth", "8", "ppm:-"]
    code, stdout, stderr = run_captured(command, timeout)
    if code != 0 or len(stdout) > MAX_NORMALIZED_BYTES:
        raise RuntimeError(f"normalizer failed: {stderr.decode(errors='replace')}")
    width, height, pixels = ppm_decode(stdout)
    if (width, height) != (PROXY_SIZE, PROXY_SIZE):
        raise RuntimeError("normalizer size mismatch")
    atomic_bytes(cache, png_encode(width, height, pixels))
    atomic_bytes(raw, pixels)
    atomic_bytes(meta, (json.dumps({"source_sha256": source_hash, "alignment": alignment}, sort_keys=True) + "\n").encode())
    return pixels, source_hash, alignment
