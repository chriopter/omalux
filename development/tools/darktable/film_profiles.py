#!/usr/bin/env python3
"""Film profiles: lookup tables for darktable's LUT 3D module, one module preset per table.

    film_profiles.py install <source dir> [camera dir] [--cube]  # converted films -> catalogue layout
    film_profiles.py generate [camera dir]                       # film.json + tables -> .dtpreset
    film_profiles.py check [camera dir]                          # presets, tables and names agree
    film_profiles.py placeholders <camera dir>                   # three made-up films for tests

Layout below the camera catalogue (catalog/camera/; development/docs/reference/darktable-bundle.md):

    <group>/<film>/film.json        {"version": 1, "group": "DHH", "brand": "Kodak", "name": "Portra 800⁺¹",
                                     "colorspace": "srgb", "interpolation": "tetrahedral",
                                     "variants": [{"key": "C", "name": "Portra 800⁺¹ - C", "lut": "c.png",
                                                   "id": "film-portra-800-plus-1-c"}, ...]}
    <group>/<film>/<lut>            the lookup table of each variant: a HALD .png, a .cube or a .3dl
    <group>/<film>/<stem>.dtpreset  written by `generate`: the LUT 3D preset that names the table

A preset is called "Omalux <group>: <variant name>", labels its module instance with the variant
name and describes itself as "<group> · <brand> · <film> · <key> · <id>", which is what Omalux
reads the grouping from. It names its table relative to darktable's LUT root folder as
camera/<group>/<film>/<lut>: the root holds the camera catalogue as `camera`.

`install` takes a folder of converted films, one sub-folder each with `look.cube`, `style.json`
({"name", "group", "family": the variant key, "kind": "film"}) and optionally `style.dtstyle`
(its lut3d item gives colour space and interpolation). The sub-folder's name becomes the
variant's id, by which a look's style.json can name its film. Tables are stored as 16-bit HALD
images of level 6 (36 nodes), resampled the way LUT 3D reads a cube (tetrahedral); --cube keeps
the cube files.
"""
import base64
import json
import re
import shutil
import struct
import sys
import zlib
from pathlib import Path
from xml.etree import ElementTree as ET

sys.path.insert(0, str(Path(__file__).resolve().parent))
import camera_presets  # noqa: E402

REPO = camera_presets.REPO
CAMERA = camera_presets.CAMERA
# dt_iop_lut3d_params_t version 3 (iop/lut3d.c): filepath[512], colorspace, interpolation,
# nb_keypoints, c_clut[2048 * 2 * 3], lutname[128].
LUT3D_VERSION = 3
LUT3D_LAYOUT = "<512siii12288s128s"
COLORSPACES = ["srgb", "adobergb", "rec709", "linear-rec709", "linear-rec2020", "linear-prophoto"]
INTERPOLATIONS = ["tetrahedral", "trilinear", "pyramid"]
LUT_SUFFIXES = (".png", ".cube", ".3dl")
SEPARATOR = " · "
HALD_LEVEL = 6
# The maker of a film whose name does not start with it, by the name's first words.
BRANDS = [("Kodak", ("Kodak", "Ektachrome", "Ektar", "Elite", "E100", "Portra", "Tri-X", "TRI-X", "Plus-X")),
          ("Fuji", ("Fuji", "160S", "400H", "Provia", "Sensia", "Velvia")),
          ("Agfa", ("Agfa", "Optima", "Portrait XPS", "Precisa", "RSX", "Ultra")),
          ("Ilford", ("Ilford", "HP5", "Pan F")),
          ("Polaroid", ("Polaroid", "PX-", "Time-Zero"))]


def encode(data):
    """darktable's compressed parameter text (common/exif.cc dt_exif_xmp_encode_internal): "gz",
    the two-digit compression factor, base64 of the zlib stream. The parameters are 13 kB of
    mostly zeros; as plain hex each preset would be 26 kB."""
    packed = zlib.compress(data, 9)
    return "gz%02d" % min(len(data) // len(packed) + 1, 99) + base64.b64encode(packed).decode()


def decode(text):
    return zlib.decompress(base64.b64decode(text[4:])) if text.startswith("gz") else bytes.fromhex(text)


def brand_of(name):
    for brand, starts in BRANDS:
        if name.startswith(starts):
            return brand
    return "Other"


def slug(name):
    text = name.lower().replace("⁺", " plus ").replace("⁻", " minus ").replace("+", " plus ")
    text = text.translate(str.maketrans("⁰¹²³⁴⁵⁶⁷⁸⁹", "0123456789"))
    return re.sub(r"[^a-z0-9]+", "-", text).strip("-")


# ---- catalogue: film.json -> presets -------------------------------------------------------

def films(root):
    return sorted(root.rglob("film.json"))


def read(manifest):
    film = json.loads(manifest.read_text())
    if film.get("version") != 1:
        raise SystemExit(f"{manifest}: unsupported version")
    for field in ("group", "name", "brand"):
        value = film.get(field)
        if not isinstance(value, str) or not value.strip() or SEPARATOR in value or (field == "group" and ": " in value):
            raise SystemExit(f"{manifest}: '{field}' must be a text without '{SEPARATOR}'")
    if film.get("colorspace", "srgb") not in COLORSPACES or film.get("interpolation", "tetrahedral") not in INTERPOLATIONS:
        raise SystemExit(f"{manifest}: unknown colorspace or interpolation")
    variants = film.get("variants")
    if not isinstance(variants, list) or not variants:
        raise SystemExit(f"{manifest}: 'variants' must list at least one lookup table")
    for variant in variants:
        lut = variant.get("lut", "")
        texts = [variant.get("key"), variant.get("name"), variant.get("id", "-")]
        if any(not isinstance(t, str) or not t.strip() or SEPARATOR in t for t in texts) or "/" in lut \
                or not lut.lower().endswith(LUT_SUFFIXES) or not (manifest.parent / lut).is_file():
            raise SystemExit(f"{manifest}: variant {variant} needs a key, a name and a lookup table beside film.json")
    return film


def presets(root, manifest):
    """[(preset path, xml)] of one film."""
    film = read(manifest)
    out = []
    for variant in film["variants"]:
        lut = manifest.parent / variant["lut"]
        relative = "camera/" + lut.relative_to(root).as_posix()
        if len(relative.encode()) >= 512:
            raise SystemExit(f"{lut}: path too long for LUT 3D")
        params = struct.pack(LUT3D_LAYOUT, relative.encode(), COLORSPACES.index(film.get("colorspace", "srgb")),
                             INTERPOLATIONS.index(film.get("interpolation", "tetrahedral")), 0, b"", b"")
        description = SEPARATOR.join([film["group"], film["brand"], film["name"], variant["key"],
                                      variant.get("id", manifest.parent.name + "-" + slug(variant["key"]))])
        text = camera_presets.preset_xml(
            f"Omalux {film['group']}: {variant['name']}", description, "lut3d", encode(params), LUT3D_VERSION,
            "%", "%", fmt=0, autoapply=False, multi_name=variant["name"], hand_edited=False)
        out.append((lut.with_suffix(".dtpreset"), text))
    return out


def generate(root):
    count = 0
    for manifest in films(root):
        for path, text in presets(root, manifest):
            path.write_text(text)
            count += 1
    print(f"{count} film presets written below {root}")


def check(root):
    """Every film's presets are the ones its film.json describes, and no preset is left over."""
    wrong, expected, names = [], {}, {}
    for manifest in films(root):
        expected.update(presets(root, manifest))
    for path, text in expected.items():
        if not path.is_file() or path.read_text() != text:
            wrong.append(f"{path}: not what film.json describes; run film_profiles.py generate")
        name = ET.fromstring(text.split("\n", 1)[1]).findtext("preset/name")
        if name in names:
            wrong.append(f"{path}: the name '{name}' is also used by {names[name]}")
        names[name] = path
    for path in sorted(root.rglob("*.dtpreset")):
        if ET.parse(path).getroot().findtext("preset/operation") == "lut3d" and path not in expected:
            wrong.append(f"{path}: a LUT 3D preset without a film.json entry")
    if wrong:
        raise SystemExit("\n".join(wrong))
    print(f"{len(expected)} film presets agree with their film.json")


# ---- lookup tables -------------------------------------------------------------------------

def read_cube(path):
    """(size, [(r, g, b)]) with red changing fastest, as the .cube format stores it."""
    size, values = 0, []
    for line in path.read_text().splitlines():
        parts = line.split()
        if not parts or parts[0].startswith("#"):
            continue
        if parts[0] == "LUT_3D_SIZE":
            size = int(parts[1])
        elif parts[0][0] in "+-.0123456789":
            values.append((float(parts[0]), float(parts[1]), float(parts[2])))
    if size < 2 or len(values) != size ** 3:
        raise SystemExit(f"{path}: not a 3D cube ({len(values)} values for size {size})")
    return size, values


def resample(size, values, nodes):
    """The table at `nodes` points per axis, tetrahedral between the cube's nodes like LUT 3D's
    default interpolation, so darktable renders the result as it would the cube at those points."""
    if nodes == size:
        return values
    out = []
    step = (size - 1) / (nodes - 1)
    last = size - 2
    axis = []
    for i in range(nodes):
        position = i * step
        index = min(int(position), last)
        axis.append((index, position - index))
    s2 = size * size
    for bi, bf in axis:
        for gi, gf in axis:
            for ri, rf in axis:
                base = ri + gi * size + bi * s2
                c000, c111 = values[base], values[base + 1 + size + s2]
                # The six tetrahedra of the cell, by the order of the fractions.
                if rf > gf:
                    if gf > bf:
                        a, b, w = values[base + 1], values[base + 1 + size], (rf, gf, bf)
                    elif rf > bf:
                        a, b, w = values[base + 1], values[base + 1 + s2], (rf, bf, gf)
                    else:
                        a, b, w = values[base + s2], values[base + 1 + s2], (bf, rf, gf)
                elif bf > gf:
                    a, b, w = values[base + s2], values[base + size + s2], (bf, gf, rf)
                elif bf > rf:
                    a, b, w = values[base + size], values[base + size + s2], (gf, bf, rf)
                else:
                    a, b, w = values[base + size], values[base + 1 + size], (gf, rf, bf)
                k0, k1, k2, k3 = 1 - w[0], w[0] - w[1], w[1] - w[2], w[2]
                out.append(tuple(k0 * c000[c] + k1 * a[c] + k2 * b[c] + k3 * c111[c] for c in range(3)))
    return out


def png(width, height, rows):
    """A 16-bit RGB PNG from big-endian rows, each filtered by whichever of the five PNG filters
    leaves the smallest residue."""
    def chunk(kind, data):
        return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data))

    def paeth(a, b, c):
        p = a + b - c
        pa, pb, pc = abs(p - a), abs(p - b), abs(p - c)
        return a if pa <= pb and pa <= pc else b if pb <= pc else c

    bpp, out, previous = 6, bytearray(), bytes(width * 6)
    for row in rows:
        left = bytes(bpp) + row[:-bpp]
        upper_left = bytes(bpp) + previous[:-bpp]
        candidates = [
            row,
            bytes((x - a) & 255 for x, a in zip(row, left)),
            bytes((x - b) & 255 for x, b in zip(row, previous)),
            bytes((x - ((a + b) >> 1)) & 255 for x, a, b in zip(row, left, previous)),
            bytes((x - paeth(a, b, c)) & 255 for x, a, b, c in zip(row, left, previous, upper_left)),
        ]
        best = min(range(5), key=lambda f: sum(v if v < 128 else 256 - v for v in candidates[f]))
        out.append(best)
        out += candidates[best]
        previous = row
    return (b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 16, 2, 0, 0, 0))
            + chunk(b"IDAT", zlib.compress(bytes(out), 9)) + chunk(b"IEND", b""))


def write_hald(path, size, values, level=HALD_LEVEL):
    """A HALD image as LUT 3D reads it (iop/lut3d.c _calculate_clut_haldclut): level³ pixels
    square, level² nodes per axis, red changing fastest along the pixels."""
    nodes, side = level * level, level ** 3
    table = resample(size, values, nodes)
    flat = bytearray()
    for rgb in table:
        for v in rgb:
            flat += struct.pack(">H", int(round(min(max(v, 0.0), 1.0) * 65535)))
    row = side * 6
    path.write_bytes(png(side, side, [bytes(flat[y * row:(y + 1) * row]) for y in range(side)]))


def cube_text(size, transform):
    lines = [f"LUT_3D_SIZE {size}"]
    for b in range(size):
        for g in range(size):
            for r in range(size):
                rgb = transform(r / (size - 1), g / (size - 1), b / (size - 1))
                lines.append(" ".join(f"{min(max(v, 0.0), 1.0):.6f}" for v in rgb))
    return "\n".join(lines) + "\n"


# ---- install -------------------------------------------------------------------------------

def style_lut3d(path):
    """Colour space and interpolation of the lut3d item of a .dtstyle, or the module defaults."""
    if path.is_file():
        for plugin in ET.parse(path).getroot().iter("plugin"):
            if plugin.findtext("operation") == "lut3d" and plugin.findtext("op_params"):
                colorspace, interpolation = struct.unpack("<ii", decode(plugin.findtext("op_params"))[512:520])
                return COLORSPACES[colorspace], INTERPOLATIONS[interpolation]
    return "srgb", "tetrahedral"


def install(source, root, keep_cube=False):
    """Write the catalogue layout for every converted film below `source`."""
    stocks = {}
    for folder in sorted(p for p in source.iterdir() if (p / "style.json").is_file() and (p / "look.cube").is_file()):
        meta = json.loads((folder / "style.json").read_text())
        if meta.get("kind", "film") != "film":
            continue
        key = str(meta.get("family") or "").strip()
        name = meta["name"].strip()
        if not key:
            raise SystemExit(f"{folder}: style.json names no variant ('family')")
        # "Portra 800⁺¹ - C", "Fuji 160C L" and "Fuji 160C 2C" are variants of the film before it.
        stock = re.sub(r"\s+(-\s+)?\d?" + re.escape(key) + r"$", "", name)
        group = meta.get("group", "Film")
        entry = stocks.setdefault((group, stock), [])
        if any(v["key"] == key for v in entry):
            raise SystemExit(f"{folder}: a second '{key}' variant of {stock}")
        entry.append(dict(key=key, name=name, id=folder.name, folder=folder))
    count = 0
    for (group, stock), variants in sorted(stocks.items()):
        target = root / slug(group) / slug(stock)
        if target.exists():
            shutil.rmtree(target)
        target.mkdir(parents=True)
        spaces = {style_lut3d(v["folder"] / "style.dtstyle") for v in variants}
        if len(spaces) != 1:
            raise SystemExit(f"{stock}: its variants disagree on colour space or interpolation")
        colorspace, interpolation = spaces.pop()
        listed = []
        for variant in sorted(variants, key=lambda v: v["key"]):
            stem = slug(variant["key"])
            if keep_cube:
                lut = stem + ".cube"
                shutil.copyfile(variant["folder"] / "look.cube", target / lut)
            else:
                lut = stem + ".png"
                write_hald(target / lut, *read_cube(variant["folder"] / "look.cube"))
            listed.append(dict(key=variant["key"], name=variant["name"], lut=lut, id=variant["id"]))
            count += 1
        (target / "film.json").write_text(json.dumps(
            dict(version=1, group=group, brand=brand_of(stock), name=stock, colorspace=colorspace,
                 interpolation=interpolation, variants=listed), ensure_ascii=False, indent=2) + "\n")
        print(f"{target.relative_to(root)}: {', '.join(v['name'] for v in listed)}", flush=True)
    generate(root)
    print(f"{count} lookup tables of {len(stocks)} films installed below {root}")


def placeholders(root, size=9):
    """Three made-up films as a converter delivers them, installed into `root`: mild warm, cool
    and desaturated renderings, each in a plain (C) and a lifted (L) variant; the third has one
    variant only. They are test data and never part of the catalogue."""
    import tempfile

    def lifted(transform, lift):
        return lambda r, g, b: tuple(v * (1 - lift) + lift for v in transform(r, g, b))

    def muted(r, g, b):
        grey = 0.2126 * r + 0.7152 * g + 0.0722 * b
        return tuple(grey + 0.45 * (v - grey) for v in (r, g, b))

    table = [("Test Warm 100 - C", "C", lifted(lambda r, g, b: (r ** 0.8, g ** 0.97, b ** 1.3), 0.0)),
             ("Test Warm 100 - L", "L", lifted(lambda r, g, b: (r ** 0.8, g ** 0.97, b ** 1.3), 0.1)),
             ("Kodak Test Cool 400⁺¹ 2C", "C", lifted(lambda r, g, b: (r ** 1.3, g, b ** 0.8), 0.0)),
             ("Kodak Test Cool 400⁺¹ L", "L", lifted(lambda r, g, b: (r ** 1.3, g, b ** 0.8), 0.1)),
             ("Fuji Test Mute 800 C", "C", muted)]
    with tempfile.TemporaryDirectory() as folder:
        for name, key, transform in table:
            film = Path(folder) / ("film-" + slug(name))
            film.mkdir()
            (film / "look.cube").write_text(cube_text(size, transform))
            (film / "style.json").write_text(json.dumps(
                dict(version=1, name=name, group="DHH", kind="film", family=key), ensure_ascii=False))
        install(Path(folder), root)


if __name__ == "__main__":
    arguments = [a for a in sys.argv[1:] if not a.startswith("--")]
    command = arguments[0] if arguments else "check"
    if command == "install" and len(arguments) >= 2:
        install(Path(arguments[1]).resolve(), Path(arguments[2]).resolve() if len(arguments) > 2 else CAMERA,
                keep_cube="--cube" in sys.argv)
    elif command == "generate":
        generate(Path(arguments[1]).resolve() if len(arguments) > 1 else CAMERA)
    elif command == "placeholders" and len(arguments) == 2:
        placeholders(Path(arguments[1]).resolve())
    elif command == "check":
        check(Path(arguments[1]).resolve() if len(arguments) > 1 else CAMERA)
    else:
        raise SystemExit(__doc__)
