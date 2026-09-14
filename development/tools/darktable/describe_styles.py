#!/usr/bin/env python3
"""Give every style a description that says what sets it apart.

Each style carried the same sentence, written when they were first converted and never
revised. A catalogue that repeats one line twenty-eight times tells a reader nothing.

Reading the parameters is not enough on its own: almost every style raises vibrance and local
contrast, so naming those would repeat a second time in different words. A setting is only
worth a mention where it departs from what the rest of the catalogue does, so every value is
first placed against the same value across all styles, and only the ends of that range are
described. Nothing is written by hand, so the text cannot drift from the files again.
"""
import argparse
import re
import statistics
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / "development/tools/calibration"))

DESCRIPTION = re.compile(r"(<description>)(.*?)(</description>)", re.S)


def reading(state):
    """The numbers a description can be built from, one per style."""
    def params(op):
        entry = state.get(op) or {}
        return (entry.get("params") or {}) if entry.get("enabled") else {}

    curve = state.get("rgbcurve") or {}
    shadows = highlights = 0.0
    if curve.get("enabled") and curve.get("params"):
        p = curve["params"]
        nodes = [(p[f"node_0_{n}_x"], p[f"node_0_{n}_y"]) for n in range(int(p.get("num_nodes_0", 2)))]
        inner = [(x, y) for x, y in nodes if 0.02 < x < 0.98]
        low = [y - x for x, y in inner if x < 0.4]
        high = [y - x for x, y in inner if x > 0.6]
        shadows = sum(low) / len(low) if low else 0.0
        highlights = sum(high) / len(high) if high else 0.0

    return {
        "exposure": float((params("exposure") or {}).get("exposure", 0.0)),
        "saturation": float(params("colisa").get("saturation", 0.0)),
        "contrast": float(params("colisa").get("contrast", 0.0)),
        "detail": float(params("bilat").get("detail", 0.0)),
        "vibrance": float(params("colorbalancergb").get("vibrance", 0.0)),
        "grain": float(params("grain").get("strength", 0.0)),
        "vignette": abs(float(params("vignette").get("brightness", 0.0))),
        "sharpen": float(params("sharpen").get("amount", 0.0)),
        "shadows": shadows,
        "highlights": highlights,
    }


def bands(readings):
    """For each number, the range the catalogue covers, so a style can be placed in it."""
    out = {}
    for key in next(iter(readings.values())):
        values = sorted(r[key] for r in readings.values())
        out[key] = (statistics.quantiles(values, n=4) if len(values) >= 4
                    else [values[0], values[len(values) // 2], values[-1]])
    return out


def describe(name, value, spread):
    """Phrases for the one style, each only where it stands out from the rest."""
    low, middle, high = spread["exposure"]
    said = []
    # A style that changes nothing must not be called dark just because the others brighten.
    if not any(abs(value[key]) > 0.02 for key in value):
        return []
    if value["saturation"] < -0.9:
        said.append("all colour taken out")
    elif value["saturation"] < spread["saturation"][0] - 1e-9:
        said.append("noticeably muted colour")
    elif value["saturation"] > spread["saturation"][2] + 1e-9:
        said.append("noticeably fuller colour")

    # Placing a style against the others must not contradict what it plainly does: a style
    # that brightens is not dark, however much more the rest of the catalogue brightens.
    if value["exposure"] > max(high, 0.15):
        said.append("among the brightest in the catalogue")
    elif value["exposure"] < min(low, -0.15):
        said.append("among the darkest in the catalogue")

    if value["shadows"] < -0.02 and value["highlights"] > 0.02:
        said.append("a curve that deepens the shadows and opens the highlights")
    elif value["shadows"] > 0.02:
        said.append("a curve that lifts the shadows")
    elif value["highlights"] < -0.02:
        said.append("a curve that holds the highlights back")

    if value["contrast"] > spread["contrast"][2]:
        said.append("firm contrast")
    elif value["contrast"] < spread["contrast"][0]:
        said.append("soft contrast")

    if value["detail"] > spread["detail"][2]:
        said.append("strong local contrast")
    elif value["detail"] < spread["detail"][0] and value["detail"] < 0:
        said.append("softened detail")

    effects = []
    if value["grain"] > spread["grain"][1]:
        effects.append("visible grain")
    elif value["grain"] > 1:
        effects.append("fine grain")
    if value["vignette"] > spread["vignette"][1]:
        effects.append("a clear vignette")
    elif value["vignette"] > 0.02:
        effects.append("a light vignette")
    if value["sharpen"] > 0.05:
        effects.append("sharpening")
    said += effects
    # Nothing stood out, so say plainly what the style does rather than claim it does nothing.
    if not said:
        if abs(value["exposure"]) > 0.15:
            said.append(f"{abs(value['exposure']):.1f} EV "
                        f"{'brighter' if value['exposure'] > 0 else 'darker'}")
        if value["vibrance"] > 0.02:
            said.append("a little more vibrance")
        if value["detail"] > 0.02:
            said.append("a little more local contrast")
        if not said:
            said.append("a gentle hand throughout")
    return said


def sentence(name, said):
    if not said:
        return f"{name} leaves the photograph as it is."
    first = said[0][0].upper() + said[0][1:]
    if len(said) == 1:
        return f"{first}."
    if len(said) == 2:
        return f"{first} and {said[1]}."
    return f"{first}, " + ", ".join(said[1:-1]) + f" and {said[-1]}."


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--catalog", default=str(ROOT / "catalog/styles"))
    parser.add_argument("--write", action="store_true", help="write the descriptions into the styles")
    arguments = parser.parse_args()
    import dtparams

    paths = sorted(Path(arguments.catalog).rglob("style.dtstyle"))
    states = {p: dtparams.read_style(p.read_text()) for p in paths}
    readings = {p: reading(state) for p, state in states.items()}
    spread = bands(readings)

    for path in paths:
        text = path.read_text()
        name = re.search(r"<name>(.*?)</name>", text).group(1)
        described = sentence(name, describe(name, readings[path], spread))
        print(f"{path.parent.name:30} {described}")
        if arguments.write:
            path.write_text(DESCRIPTION.sub(lambda m: m.group(1) + described + m.group(3), text, count=1))


if __name__ == "__main__":
    main()
