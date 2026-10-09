#!/usr/bin/env python3
"""Install a set of finished looks into the catalogue as one family.

  install_family.py --looks DIR --index FILE --family ID [--label NAME]
                    [--results DIR] [--skip-flagged] [--makers A,B,…] [--dry-run]

  --looks    DIR/<id>/style.dtstyle and DIR/<id>/look.cube, one folder per look
  --index    JSON with `looks`: [{id, name, film, family, monochrome, duplicate_of?, group?, film_profile?}]
  --results  DIR/<id>/final.done marks a finished look, DIR/<id>/result.json may carry a
             `flag`; without --results every look that has both files counts as finished
  --family   folder under catalog/styles/, e.g. `dhh`; --label is how the pane names it

Writes catalog/styles/<family>/<group>/<slug>/ with `style.dtstyle` (named after the look, its
LUT path pointing at the new place), `look.cube` and `style.json`, and a `family.json` with the
display name of the family and of each group. Groups keep a large family navigable: black and
white looks, then one group per maker, the rest under "Variants"; an index entry may name its
group itself. Looks that are a duplicate of another, unfinished or incomplete are skipped and
listed. Running it again adds what finished meanwhile and updates what changed; a look whose
style or cube changed loses its thumbnail, and the looks without one are listed for
`development/style_preview`.
"""
import argparse
import json
import re
import shutil
import sys
import unicodedata
from pathlib import Path
from xml.sax.saxutils import escape

REPO = Path(__file__).resolve().parents[3]
STYLES = REPO / "catalog/styles"
SUPERSCRIPTS = str.maketrans("⁰¹²³⁴⁵⁶⁷⁸⁹", "0123456789")
DESCRIPTION = "Fitted to a target rendering of this look on a standard camera profile."


def slug(text):
    """"HP5⁻¹" reads hp5-minus-1, "TRI-X⁺²" tri-x-plus-2, "Black & white" black-and-white."""
    text = text.replace("⁻", " minus ").replace("⁺", " plus ").translate(SUPERSCRIPTS)
    text = text.replace("+", " plus ").replace("&", " and ")
    text = unicodedata.normalize("NFKD", text).encode("ascii", "ignore").decode().lower()
    return re.sub(r"[^a-z0-9]+", "-", text).strip("-")


def display_name(look, twins):
    """The look's own name; the pack's family letter is added only where two differ by it."""
    return f"{look['film']} ({look['family']})" if look["film"] in twins and look["family"] != twins[look["film"]] else look["film"]


def group_of(look, makers):
    if look.get("group"):
        return look["group"]
    if look.get("monochrome"):
        return "Black & white"
    first = look["film"].split()[0]
    return first if first in makers else "Variants"


def set_lut_path(text, path):
    m = re.search(r"(<operation>lut3d</operation>\s*<op_params>)([0-9a-f]*)(</op_params>)", text)
    if not m:
        raise ValueError("style has no lut3d item")
    raw = bytearray(bytes.fromhex(m.group(2)))
    encoded = path.encode()
    if len(encoded) > 511:
        raise ValueError(f"LUT path too long: {path}")
    raw[:512] = encoded.ljust(512, b"\0")
    return text[:m.start(2)] + raw.hex() + text[m.end(2):]


def write_if_changed(path, data):
    if path.exists() and path.read_bytes() == data:
        return False
    path.write_bytes(data)
    return True


def write_label(folder, name, order=None):
    data = {"version": 1, "name": name}
    if order is not None:
        data["order"] = order
    folder.mkdir(parents=True, exist_ok=True)
    write_if_changed(folder / "family.json", (json.dumps(data, indent=2, ensure_ascii=False) + "\n").encode())


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--looks", required=True, type=Path)
    ap.add_argument("--index", required=True, type=Path)
    ap.add_argument("--family", required=True)
    ap.add_argument("--label")
    ap.add_argument("--order", type=int, default=1, help="place among the families: 0 with the others, 1 after them")
    ap.add_argument("--results", type=Path)
    ap.add_argument("--skip-flagged", action="store_true", help="leave out looks whose result.json flag is not ok")
    ap.add_argument("--makers", default="Agfa,Fuji,Ilford,Kodak,Polaroid")
    ap.add_argument("--dry-run", action="store_true")
    a = ap.parse_args()
    if not re.fullmatch(r"[a-z0-9][a-z0-9-]*", a.family):
        ap.error("--family is a folder name: lower-case letters, digits and hyphens")
    makers = set(a.makers.split(","))
    looks = json.load(open(a.index))["looks"]
    distinct = [e for e in looks if "duplicate_of" not in e]
    # Films that occur in more than one family of the pack: the first keeps the plain name.
    twins = {}
    for e in distinct:
        twins.setdefault(e["film"], []).append(e["family"])
    twins = {film: families[0] for film, families in twins.items() if len(families) > 1}
    taken = {re.search(r"<name>(.*?)</name>", p.read_text()).group(1): p.parent
             for p in STYLES.rglob("style.dtstyle")}
    family = STYLES / a.family
    installed, updated, unchanged, skipped, flagged, groups = [], [], [], [], [], {}
    for e in looks:
        if "duplicate_of" in e:
            continue
        src = a.looks / e["id"]
        if a.results and not (a.results / e["id"] / "final.done").exists():
            skipped.append((e["id"], "not finished"))
            continue
        if not (src / "style.dtstyle").is_file() or not (src / "look.cube").is_file():
            skipped.append((e["id"], "style or cube missing"))
            continue
        flag = "ok"
        if a.results and (a.results / e["id"] / "result.json").exists():
            flag = json.load(open(a.results / e["id"] / "result.json")).get("flag", "ok")
        name = display_name(e, twins)
        if flag != "ok":
            flagged.append((e["id"], name, flag))
            if a.skip_flagged:
                skipped.append((e["id"], f"flagged {flag}"))
                continue
        group = group_of(e, makers)
        groups[slug(group)] = group
        bundle = family / slug(group) / slug(name)
        if taken.get(name, bundle) != bundle:
            skipped.append((e["id"], f"name “{name}” already used by {taken[name].relative_to(STYLES)}"))
            continue
        taken[name] = bundle
        rel = bundle.relative_to(STYLES).as_posix()
        text = (src / "style.dtstyle").read_text()
        description = ("Black and white. " if e.get("monochrome") else "") + DESCRIPTION
        text = re.sub(r"<name>.*?</name>", lambda _: f"<name>{escape(name)}</name>", text, count=1)
        text = re.sub(r"<description>.*?</description>", lambda _: f"<description>{escape(description)}</description>",
                      text, count=1, flags=re.S)
        text = set_lut_path(text, f"{rel}/look.cube")
        cube = (src / "look.cube").read_text().split("\n")
        if cube and cube[0].startswith("#"):
            cube[0] = f"# {name}"
        cube = "\n".join(cube).encode()
        if a.dry_run:
            print(f"{e['id']} -> {rel}  “{name}”  [{group}]" + ("" if flag == "ok" else f"  flag {flag}"))
            continue
        new = not bundle.exists()
        bundle.mkdir(parents=True, exist_ok=True)
        changed = write_if_changed(bundle / "style.dtstyle", text.encode())
        changed |= write_if_changed(bundle / "look.cube", cube)
        manifest_path = bundle / "style.json"
        manifest = json.load(open(manifest_path)) if manifest_path.exists() else {"version": 1}
        manifest["assets"] = [{"path": "look.cube", "role": "lut"}]
        if e.get("film_profile"):
            # The film profile (catalog/camera, variant id) this look is meant to sit on.
            manifest["film"] = e["film_profile"]
        if changed and not new:
            # The stored preview no longer shows this look.
            (bundle / "thumbnail.jpg").unlink(missing_ok=True)
            manifest.pop("preview", None)
        write_if_changed(manifest_path, (json.dumps(manifest, indent=2) + "\n").encode())
        (installed if new else updated if changed else unchanged).append(rel)
    if a.dry_run:
        return
    write_label(family, a.label or a.family.replace("-", " ").capitalize(), a.order)
    for folder, label in groups.items():
        write_label(family / folder, label)
    print(f"{a.family}: {len(installed)} installed, {len(updated)} updated, {len(unchanged)} unchanged, "
          f"{len(skipped)} skipped")
    for rel in installed:
        print(f"  new      {rel}")
    for rel in updated:
        print(f"  updated  {rel}")
    for pid, why in skipped:
        print(f"  skipped  {pid}: {why}")
    for pid, name, flag in flagged:
        print(f"  flagged  {pid} “{name}”: {flag}" + (" (not installed)" if a.skip_flagged else " (installed; look at it)"))
    missing = sorted(p.parent.relative_to(STYLES).as_posix() for p in family.rglob("style.dtstyle")
                     if not (p.parent / "thumbnail.jpg").exists())
    if missing:
        print(f"{len(missing)} without thumbnail; run for each:  development/style_preview <bundle>")
        for rel in missing:
            print(f"  preview  {rel}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
