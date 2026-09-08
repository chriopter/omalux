"""Image-pair metrics on the 256x256 proxies (numpy).

Same definitions: display/linear PSNR, global luminance SSIM, CIEDE2000
mean/p95 after D65 sRGB->Lab. Verified against the scalar implementation.
"""

from __future__ import annotations

import numpy as np

_SRGB_TO_LINEAR = np.empty(256, dtype=np.float64)
for _v in range(256):
    _c = _v / 255.0
    _SRGB_TO_LINEAR[_v] = _c / 12.92 if _c <= 0.04045 else ((_c + 0.055) / 1.055) ** 2.4


def _rgb_to_lab(rgb_u8: np.ndarray) -> np.ndarray:
    linear = _SRGB_TO_LINEAR[rgb_u8]
    matrix = np.array([
        [0.4124564, 0.3575761, 0.1804375],
        [0.2126729, 0.7151522, 0.0721750],
        [0.0193339, 0.1191920, 0.9503041],
    ])
    xyz = linear @ matrix.T
    white = np.array([0.95047, 1.0, 1.08883])
    t = xyz / white
    f = np.where(t > (6 / 29) ** 3, np.cbrt(t), t / (3 * (6 / 29) ** 2) + 4 / 29)
    lab = np.empty_like(f)
    lab[:, 0] = 116 * f[:, 1] - 16
    lab[:, 1] = 500 * (f[:, 0] - f[:, 1])
    lab[:, 2] = 200 * (f[:, 1] - f[:, 2])
    return lab


def _delta_e_2000(lab1: np.ndarray, lab2: np.ndarray) -> np.ndarray:
    L1, a1, b1 = lab1[:, 0], lab1[:, 1], lab1[:, 2]
    L2, a2, b2 = lab2[:, 0], lab2[:, 1], lab2[:, 2]
    C1 = np.hypot(a1, b1)
    C2 = np.hypot(a2, b2)
    Cm = (C1 + C2) / 2
    G = 0.5 * (1 - np.sqrt(Cm**7 / (Cm**7 + 25.0**7)))
    a1p = (1 + G) * a1
    a2p = (1 + G) * a2
    C1p = np.hypot(a1p, b1)
    C2p = np.hypot(a2p, b2)
    h1p = np.degrees(np.arctan2(b1, a1p)) % 360
    h2p = np.degrees(np.arctan2(b2, a2p)) % 360
    dLp = L2 - L1
    dCp = C2p - C1p
    dh = h2p - h1p
    dh = np.where(dh > 180, dh - 360, dh)
    dh = np.where(dh < -180, dh + 360, dh)
    dh = np.where((C1p * C2p) == 0, 0.0, dh)
    dHp = 2 * np.sqrt(C1p * C2p) * np.sin(np.radians(dh) / 2)
    Lpm = (L1 + L2) / 2
    Cpm = (C1p + C2p) / 2
    hsum = h1p + h2p
    hdiff = np.abs(h1p - h2p)
    hpm = np.where(
        (C1p * C2p) == 0, hsum,
        np.where(hdiff <= 180, hsum / 2,
                 np.where(hsum < 360, (hsum + 360) / 2, (hsum - 360) / 2)))
    T = (1 - 0.17 * np.cos(np.radians(hpm - 30)) + 0.24 * np.cos(np.radians(2 * hpm))
         + 0.32 * np.cos(np.radians(3 * hpm + 6)) - 0.20 * np.cos(np.radians(4 * hpm - 63)))
    d_theta = 30 * np.exp(-(((hpm - 275) / 25) ** 2))
    Rc = 2 * np.sqrt(Cpm**7 / (Cpm**7 + 25.0**7))
    Sl = 1 + 0.015 * (Lpm - 50) ** 2 / np.sqrt(20 + (Lpm - 50) ** 2)
    Sc = 1 + 0.045 * Cpm
    Sh = 1 + 0.015 * Cpm * T
    Rt = -np.sin(np.radians(2 * d_theta)) * Rc
    return np.sqrt((dLp / Sl) ** 2 + (dCp / Sc) ** 2 + (dHp / Sh) ** 2
                   + Rt * (dCp / Sc) * (dHp / Sh))


def pair_metrics(a: bytes, b: bytes) -> dict:
    pa = np.frombuffer(a, dtype=np.uint8).reshape(-1, 3)
    pb = np.frombuffer(b, dtype=np.uint8).reshape(-1, 3)
    fa = pa.astype(np.float64) / 255.0
    fb = pb.astype(np.float64) / 255.0
    display_mse = float(np.mean((fa - fb) ** 2))
    la = _SRGB_TO_LINEAR[pa]
    lb = _SRGB_TO_LINEAR[pb]
    linear_mse = float(np.mean((la - lb) ** 2))
    luma = np.array([0.2126, 0.7152, 0.0722])
    ya = fa @ luma
    yb = fb @ luma
    ma, mb = float(ya.mean()), float(yb.mean())
    va, vb = float(ya.var()), float(yb.var())
    cov = float(np.mean((ya - ma) * (yb - mb)))
    c1, c2 = 0.01**2, 0.03**2
    ssim = ((2 * ma * mb + c1) * (2 * cov + c2)) / ((ma * ma + mb * mb + c1) * (va + vb + c2))
    des = _delta_e_2000(_rgb_to_lab(pa), _rgb_to_lab(pb))
    des_sorted = np.sort(des)
    index = 0.95 * (len(des_sorted) - 1)
    low = int(index)
    frac = index - low
    p95 = float(des_sorted[low] * (1 - frac)
                + des_sorted[min(low + 1, len(des_sorted) - 1)] * frac)
    psnr = lambda mse: None if mse == 0 else float(10 * np.log10(1 / mse))
    return {"psnr_display": psnr(display_mse), "psnr_linear": psnr(linear_mse),
            "ssim_global": ssim, "delta_e_2000_mean": float(des.mean()),
            "delta_e_2000_p95": p95}
