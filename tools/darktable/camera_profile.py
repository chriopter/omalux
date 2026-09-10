#!/usr/bin/env python3
"""Write an ICC input profile for a camera, and the preset that selects it.

    camera_profile.py --camera "FUJIFILM X-T10" [--out catalog/camera/fujifilm/x-t10.icc]
    camera_profile.py --matrix "8458 -2451 -855 -4597 12447 2407 -1475 2482 6526" \
                      --name "Fujifilm X-T10" --out catalog/camera/fujifilm/x-t10.icc

The matrix is the DNG/dcraw convention: XYZ (D65) to camera RGB, scaled by
10000, the same numbers rawspeed keeps in `data/cameras.xml` and darktable
uses as the "standard color matrix". With `--camera` the matrix is read from
that file in the darktable submodule.

The profile is a v2 matrix/TRC input profile: the inverse matrix, adapted to
the ICC connection white point D50 by a Bradford transform and normalised so
camera white maps to D50, with linear tone curves. That is the same class of
profile darktable offers under "input color profile", so a profile written
here is a starting point for an own characterisation: replace the matrix with
measured values, or convert a DCP with DCamProf, and keep everything else.

`--preset` additionally writes a .dtpreset next to the profile that selects it
automatically for that camera, so the pair works like a Lightroom camera profile.
"""
import argparse
import re
import struct
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
CAMERAS_XML = REPO / "darktable/src/external/rawspeed/data/cameras.xml"
D50 = (0.9642, 1.0, 0.8249)
# Bradford cone response, used by ICC for chromatic adaptation between white points.
BRADFORD = ((0.8951, 0.2664, -0.1614), (-0.7502, 1.7135, 0.0367), (0.0389, -0.0685, 1.0296))
D65 = (0.95047, 1.0, 1.08883)


def mat_mul(a, b):
    return [[sum(a[i][k] * b[k][j] for k in range(3)) for j in range(3)] for i in range(3)]


def mat_vec(m, v):
    return [sum(m[i][k] * v[k] for k in range(3)) for i in range(3)]


def mat_inv(m):
    (a, b, c), (d, e, f), (g, h, i) = m
    det = a * (e * i - f * h) - b * (d * i - f * g) + c * (d * h - e * g)
    if abs(det) < 1e-12:
        raise ValueError("singular matrix")
    return [[(e * i - f * h) / det, (c * h - b * i) / det, (b * f - c * e) / det],
            [(f * g - d * i) / det, (a * i - c * g) / det, (c * d - a * f) / det],
            [(d * h - e * g) / det, (b * g - a * h) / det, (a * e - b * d) / det]]


def bradford(src, dst):
    """Chromatic adaptation matrix from white point src to dst."""
    s = mat_vec(BRADFORD, src)
    d = mat_vec(BRADFORD, dst)
    scale = [[d[0] / s[0], 0, 0], [0, d[1] / s[1], 0], [0, 0, d[2] / s[2]]]
    return mat_mul(mat_inv(BRADFORD), mat_mul(scale, BRADFORD))


def camera_matrix(name):
    """XYZ-to-camera matrix of `name` ("MAKE MODEL") from rawspeed's camera database."""
    if not CAMERAS_XML.is_file():
        raise SystemExit(f"{CAMERAS_XML} missing; run: git submodule update --init --depth 1 "
                         "darktable && git -C darktable submodule update --init --depth 1 "
                         "src/external/rawspeed")
    text = CAMERAS_XML.read_text()
    for m in re.finditer(r'<Camera make="([^"]+)" model="([^"]+)"[^>]*>(.*?)</Camera>', text, re.S):
        make, model, body = m.group(1), m.group(2), m.group(3)
        if f"{make} {model}".lower() != name.lower():
            continue
        rows = re.findall(r'<ColorMatrixRow plane="\d">([^<]+)</ColorMatrixRow>', body)
        if len(rows) >= 3:
            return [[int(v) / 10000 for v in row.split()] for row in rows[:3]], f"{make} {model}"
    raise SystemExit(f'no colour matrix for "{name}" in the camera database')


def cam_to_xyz_d50(xyz_to_cam, adapt="scale"):
    """Camera RGB to XYZ (D50). The matrix maps camera white to the ICC white point either by
    a Bradford adaptation from D65 ("bradford") or by mapping camera white onto D50 directly
    ("scale"), which is how camera matrix profiles are usually built and what darktable's own
    matrix path is closest to."""
    cam_to_xyz = mat_inv(xyz_to_cam)                    # camera RGB -> XYZ (D65-referred)
    if adapt == "bradford":
        cam_to_xyz = mat_mul(bradford(D65, D50), cam_to_xyz)
    white = mat_vec(cam_to_xyz, (1.0, 1.0, 1.0))        # where camera white lands
    fix = bradford(white, D50)                          # move it onto the ICC white point
    return mat_mul(fix, cam_to_xyz)


# ---------- ICC v2 matrix/TRC input profile ----------
def s15(x):
    return int(round(x * 65536))


def tag_xyz(v):
    return b"XYZ " + b"\0" * 4 + struct.pack(">iii", s15(v[0]), s15(v[1]), s15(v[2]))


def tag_curve_linear():
    return b"curv" + b"\0" * 4 + struct.pack(">I", 0)   # count 0 = identity


def tag_text(text):
    """'desc' (ICC v2): ASCII string, then the unused Unicode and ScriptCode fields."""
    data = text.encode("ascii", "replace") + b"\0"
    return (b"desc" + b"\0" * 4 + struct.pack(">I", len(data)) + data
            + struct.pack(">II", 0, 0)          # Unicode language code and count
            + struct.pack(">HB", 0, 0) + b"\0" * 67)  # ScriptCode code, count, string


def tag_copyright(text):
    return b"text" + b"\0" * 4 + text.encode("ascii", "replace") + b"\0"


def icc_header(size):
    """The 128-byte ICC v2.4 header of an RGB input profile with an XYZ connection space."""
    header = bytearray(128)
    struct.pack_into(">I", header, 0, size)
    struct.pack_into(">I", header, 8, 0x02400000)      # version 2.4
    header[12:16] = b"scnr"                            # input device profile
    header[16:20] = b"RGB "
    header[20:24] = b"XYZ "
    header[36:40] = b"acsp"
    struct.pack_into(">I", header, 64, 0)              # perceptual rendering intent
    struct.pack_into(">iii", header, 68, s15(D50[0]), s15(D50[1]), s15(D50[2]))  # PCS illuminant
    header[80:84] = b"OMLX"                            # creator
    return bytes(header)


def write_icc(path, matrix, description, copyright_text):
    tags = [(b"desc", tag_text(description)), (b"cprt", tag_copyright(copyright_text)),
            (b"wtpt", tag_xyz(D50)),
            (b"rXYZ", tag_xyz([matrix[i][0] for i in range(3)])),
            (b"gXYZ", tag_xyz([matrix[i][1] for i in range(3)])),
            (b"bXYZ", tag_xyz([matrix[i][2] for i in range(3)])),
            (b"rTRC", tag_curve_linear()), (b"gTRC", tag_curve_linear()), (b"bTRC", tag_curve_linear())]
    table = struct.pack(">I", len(tags))
    offset = 128 + 4 + 12 * len(tags)
    body = b""
    for sig, data in tags:
        table += struct.pack(">4sII", sig, offset + len(body), len(data))
        body += data + b"\0" * ((-len(data)) % 4)
    profile = icc_header(128 + len(table) + len(body)) + table + body
    Path(path).parent.mkdir(parents=True, exist_ok=True)
    Path(path).write_bytes(profile)


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--camera", help='"MAKE MODEL" as in the camera database')
    ap.add_argument("--matrix", help="nine XYZ-to-camera values, scaled by 10000")
    ap.add_argument("--name", help="profile description (default: the camera name)")
    ap.add_argument("--out", required=True)
    ap.add_argument("--adapt", choices=["scale", "bradford"], default="scale",
                    help="how camera white reaches the ICC white point (default: scale)")
    ap.add_argument("--preset", action="store_true", help="also write a .dtpreset selecting this profile")
    ap.add_argument("--autoapply", action="store_true",
                    help="let that preset apply itself to this camera; off by default, because a "
                         "profile built from the camera matrix is a starting point, not a measured "
                         "characterisation")
    ap.add_argument("--maker", help="EXIF maker pattern for the preset (default: first word of --camera)")
    ap.add_argument("--model", help="EXIF model pattern for the preset (default: rest of --camera)")
    a = ap.parse_args()
    if bool(a.camera) == bool(a.matrix):
        ap.error("give either --camera or --matrix")
    if a.matrix:
        values = [float(v) / 10000 for v in a.matrix.replace(",", " ").split()]
        if len(values) != 9:
            ap.error("--matrix needs nine values")
        xyz_to_cam = [values[0:3], values[3:6], values[6:9]]
        camera = a.name or "camera"
    else:
        xyz_to_cam, camera = camera_matrix(a.camera)
    name = a.name or camera
    matrix = cam_to_xyz_d50(xyz_to_cam, a.adapt)
    write_icc(a.out, matrix, f"Omalux {name}", "Omalux, GPL-3.0-or-later")
    print(f"{a.out}: input profile for {name}")
    for row in matrix:
        print("   " + " ".join(f"{v: .6f}" for v in row))
    if a.preset:
        sys.path.insert(0, str(Path(__file__).resolve().parent))
        import camera_presets
        import dtparams
        maker = a.maker or (a.camera.split()[0] if a.camera else "%")
        model = a.model or (" ".join(a.camera.split()[1:]) if a.camera else "%")
        params = dict(dtparams.DEFAULTS["colorin"])
        params.update(type=0, filename=Path(a.out).name.encode())  # 0 = DT_COLORSPACE_FILE
        text = camera_presets.preset_xml(f"Omalux {name} input profile", f"{name}: input profile rebuilt from the camera matrix",
                                         "colorin", dtparams.encode("colorin", params),
                                         dtparams.VERSIONS["colorin"], maker, model,
                                         autoapply=a.autoapply)
        dest = Path(a.out).with_suffix(".dtpreset")
        dest.write_text(text)
        print(f"{dest}: preset selecting it for {maker} / {model}"
              f"{' automatically' if a.autoapply else ', to be chosen in the module'}")


if __name__ == "__main__":
    main()
