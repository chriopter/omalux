"""Encode/decode op_params of the darktable modules used by the bundled styles (5.6.1 layouts)."""
import re, struct

LAYOUTS = {
    "exposure": ("<iffffii", ["mode", "black", "exposure", "deflicker_percentile", "deflicker_target_level",
                             "compensate_exposure_bias", "compensate_hilite_pres"]),
    "colisa": ("<fff", ["contrast", "brightness", "saturation"]),
    "shadhi": ("<iffffffffIfi", ["order", "radius", "shadows", "whitepoint", "highlights", "reserved2", "compress",
                                 "shadows_ccorrect", "highlights_ccorrect", "flags", "low_approximation", "shadhi_algo"]),
    "vignette": ("<ffffffiffii", ["scale", "falloff_scale", "brightness", "saturation", "center_x", "center_y",
                                  "autoratio", "whratio", "shape", "dithering", "unbound"]),
    "sharpen": ("<fff", ["radius", "amount", "threshold"]),
    "grain": ("<ifff", ["channel", "scale", "strength", "midtones_bias"]),
}
DEFAULTS = {
    "exposure": dict(mode=0, black=0.0, exposure=0.0, deflicker_percentile=50.0, deflicker_target_level=-4.0,
                     compensate_exposure_bias=0, compensate_hilite_pres=1),
    "colisa": dict(contrast=0.0, brightness=0.0, saturation=0.0),
    "shadhi": dict(order=0, radius=100.0, shadows=50.0, whitepoint=0.0, highlights=-50.0, reserved2=0.0, compress=50.0,
                   shadows_ccorrect=100.0, highlights_ccorrect=50.0, flags=0, low_approximation=0.000001, shadhi_algo=1),
    "vignette": dict(scale=80.0, falloff_scale=50.0, brightness=-0.5, saturation=-0.5, center_x=0.0, center_y=0.0,
                     autoratio=0, whratio=1.0, shape=1.0, dithering=0, unbound=1),
    "sharpen": dict(radius=2.0, amount=0.5, threshold=0.5),
    "grain": dict(channel=0, scale=1600.0 / 213.2, strength=25.0, midtones_bias=100.0),
}

_RE = re.compile(r"(<operation>(\w+)</operation>\s*<op_params>)([0-9a-f]*)(</op_params>\s*<enabled>)(\d)(</enabled>)", re.S)


def decode(op, hexstr):
    fmt, names = LAYOUTS[op]
    vals = struct.unpack(fmt, bytes.fromhex(hexstr))
    return dict(zip(names, vals))


def encode(op, d):
    fmt, names = LAYOUTS[op]
    return struct.pack(fmt, *[d[n] for n in names]).hex()


def read_style(text):
    """-> {op: {"params": dict|None, "enabled": bool}} for known modules."""
    out = {}
    for m in _RE.finditer(text):
        op = m.group(2)
        if op in LAYOUTS:
            hx = m.group(3)
            out[op] = dict(params=decode(op, hx) if hx else None, enabled=m.group(5) == "1")
    return out


def write_style(text, state):
    """Apply {op: {"params":..., "enabled":...}} back into the style XML."""
    def repl(m):
        op = m.group(2)
        if op not in state:
            return m.group(0)
        st = state[op]
        hx = encode(op, st["params"]) if st["params"] is not None else m.group(3)
        return f"{m.group(1)}{hx}{m.group(4)}{1 if st['enabled'] else 0}{m.group(6)}"
    return _RE.sub(repl, text)
