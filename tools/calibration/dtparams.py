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
    "nlmeans": ("<ffff", ["radius", "strength", "luma", "chroma"]),
    "bilat": ("<iffff", ["mode", "sigma_r", "sigma_s", "detail", "midtone"]),
    "lens": ("<iiifffffi128s128sifffffffififffff", ['method', 'modify_flags', 'inverse', 'scale', 'crop', 'focal', 'aperture', 'distance', 'target_geom', 'camera', 'lens', 'tca_override', 'tca_r', 'tca_b', 'cor_dist_ft', 'cor_vig_ft', 'cor_ca_r_ft', 'cor_ca_b_ft', 'scale_md_v1', 'md_version', 'scale_md', 'has_been_set', 'v_strength', 'v_radius', 'v_steepness', 'reserved0', 'reserved1']),
    "demosaic": ("<ififfffffifi", ["green_eq", "median_thrs", "color_smoothing", "demosaicing_method", "lmmse_refine",
                                   "dual_thrs", "cs_radius", "cs_thrs", "cs_boost", "cs_iter", "cs_center", "cs_enabled"]),
    "colorbalancergb": ("<" + "f" * 32 + "i", ['shadows_Y', 'shadows_C', 'shadows_H', 'midtones_Y', 'midtones_C', 'midtones_H', 'highlights_Y', 'highlights_C', 'highlights_H', 'global_Y', 'global_C', 'global_H', 'shadows_weight', 'white_fulcrum', 'highlights_weight', 'chroma_shadows', 'chroma_highlights', 'chroma_global', 'chroma_midtones', 'saturation_global', 'saturation_highlights', 'saturation_midtones', 'saturation_shadows', 'hue_angle', 'brilliance_global', 'brilliance_highlights', 'brilliance_midtones', 'brilliance_shadows', 'mask_grey_fulcrum', 'vibrance', 'grey_fulcrum', 'contrast', 'saturation_formula']),
    "toneequal": ("<" + "f" * 15 + "iii", ['noise', 'ultra_deep_blacks', 'deep_blacks', 'blacks', 'shadows', 'midtones', 'highlights', 'whites', 'speculars', 'blending', 'smoothing', 'feathering', 'quantization', 'contrast_boost', 'exposure_boost', 'details', 'method', 'iterations']),
    "sigmoid": ("<ffffiffffffffi", ["middle_grey_contrast", "contrast_skewness", "display_white_target",
                                   "display_black_target", "color_processing", "hue_preservation", "red_inset",
                                   "red_rotation", "green_inset", "green_rotation", "blue_inset", "blue_rotation",
                                   "purity", "base_primaries"]),
}
VERSIONS = {"lens": 10, "demosaic": 6, "colorbalancergb": 5, "toneequal": 2, "bilat": 3, "nlmeans": 2, "exposure": 7, "colisa": 1, "shadhi": 5, "vignette": 4, "sharpen": 1, "grain": 2, "sigmoid": 3}
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
    "nlmeans": dict(radius=2.0, strength=50.0, luma=0.5, chroma=1.0),
    "bilat": dict(mode=1, sigma_r=0.5, sigma_s=0.5, detail=0.25, midtone=0.5),
    "lens": dict(method=1, modify_flags=7, inverse=0, scale=1.0, crop=0.0, focal=0.0, aperture=0.0, distance=0.0, target_geom=1,
                 camera=b"", lens=b"", tca_override=0, tca_r=1.0, tca_b=1.0, cor_dist_ft=1.0, cor_vig_ft=1.0, cor_ca_r_ft=1.0,
                 cor_ca_b_ft=1.0, scale_md_v1=1.0, md_version=1, scale_md=1.0, has_been_set=0, v_strength=0.0, v_radius=0.5,
                 v_steepness=0.5, reserved0=0.0, reserved1=0.0),
    "demosaic": dict(green_eq=0, median_thrs=0.0, color_smoothing=0, demosaicing_method=5, lmmse_refine=1, dual_thrs=0.2,
                     cs_radius=0.0, cs_thrs=0.4, cs_boost=0.0, cs_iter=8, cs_center=0.0, cs_enabled=0),
    "colorbalancergb": {'shadows_Y': 0.0, 'shadows_C': 0.0, 'shadows_H': 0.0, 'midtones_Y': 0.0, 'midtones_C': 0.0, 'midtones_H': 0.0, 'highlights_Y': 0.0, 'highlights_C': 0.0, 'highlights_H': 0.0, 'global_Y': 0.0, 'global_C': 0.0, 'global_H': 0.0, 'shadows_weight': 1.0, 'white_fulcrum': 0.0, 'highlights_weight': 1.0, 'chroma_shadows': 0.0, 'chroma_highlights': 0.0, 'chroma_global': 0.0, 'chroma_midtones': 0.0, 'saturation_global': 0.0, 'saturation_highlights': 0.0, 'saturation_midtones': 0.0, 'saturation_shadows': 0.0, 'hue_angle': 0.0, 'brilliance_global': 0.0, 'brilliance_highlights': 0.0, 'brilliance_midtones': 0.0, 'brilliance_shadows': 0.0, 'mask_grey_fulcrum': 0.1845, 'vibrance': 0.0, 'grey_fulcrum': 0.1845, 'contrast': 0.0, 'saturation_formula': 1},
    "toneequal": {'noise': 0.0, 'ultra_deep_blacks': 0.0, 'deep_blacks': 0.0, 'blacks': 0.0, 'shadows': 0.0, 'midtones': 0.0, 'highlights': 0.0, 'whites': 0.0, 'speculars': 0.0, 'blending': 5.0, 'smoothing': 1.414213562, 'feathering': 1.0, 'quantization': 0.0, 'contrast_boost': 0.0, 'exposure_boost': 0.0, 'details': 4, 'method': 4, 'iterations': 1},
    "sigmoid": dict(middle_grey_contrast=1.5, contrast_skewness=0.0, display_white_target=100.0,
                    display_black_target=0.0152, color_processing=0, hue_preservation=100.0, red_inset=0.0,
                    red_rotation=0.0, green_inset=0.0, green_rotation=0.0, blue_inset=0.0, blue_rotation=0.0,
                    purity=0.0, base_primaries=0),
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


_PLUGIN = """    <plugin>
      <num>{num}</num>
      <module>{ver}</module>
      <operation>{op}</operation>
      <op_params>{params}</op_params>
      <enabled>{enabled}</enabled>
      <blendop_params />
      <blendop_version>0</blendop_version>
      <multi_priority>0</multi_priority>
      <multi_name />
      <multi_name_hand_edited>0</multi_name_hand_edited>
    </plugin>
"""


def ensure_module(text, op, params=None, enabled=True):
    """Return style text that contains `op` (appended if missing) with the given params/enabled."""
    st = read_style(text)
    if op in st:
        st[op]["params"] = dict(DEFAULTS[op], **(params or {})) if params is not None or st[op]["params"] is None else st[op]["params"]
        st[op]["enabled"] = enabled
        return write_style(text, st)
    num = len(re.findall(r"<plugin>", text))
    block = _PLUGIN.format(num=num, ver=VERSIONS[op], op=op, params=encode(op, dict(DEFAULTS[op], **(params or {}))),
                           enabled=1 if enabled else 0)
    return text.replace("  </style>", block + "  </style>")
