#!/usr/bin/env python3
"""Fit a preset's 3D LUT so darktable reproduces the target renderings.

  cube_fit.py baseline [--presets a,b] [--split tuning|holdout|all]
  cube_fit.py fit <preset> [--iters 15] [--resume]
  cube_fit.py fit-all [--presets a,b] [--iters 15]
  cube_fit.py final <preset>|--presets a,b

Fixed-point fit. The LUT input is what the pipeline hands to lut3d, so it is
rendered once per image with lut3d and everything after it disabled. Each
iteration renders the full style, scatters the residual (target minus output)
trilinearly into the 33^3 grid keyed by the LUT input, solves a
Laplacian-regularised correction field on the grid, and adds it to the cube
with damping. The style is
fitted with `colisa` disabled: global tone and colour live in the cube, and a
display-referred adjustment after the LUT would fight it.

Work layout: work/<preset>/style.dtstyle (fitted style), lut/<catalogue path>
(candidate cube), input/, iter-N/, best.cube, best.json, final/, final.json.
"""
import argparse
import json
import os
import re
import shutil
import sys
import time
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parent))
import common  # noqa: E402
import dtparams  # noqa: E402
import dtrender  # noqa: E402
from common import JOBS, PRESETS, RENDER, WORK, images, preset_dirs, score_render  # noqa: E402

POST_LUT = {"colisa", "shadhi", "sharpen", "grain", "vignette"}
LUT_SIZE = 33


# ---------- style helpers ----------
def style_text(preset_dir):
    return (preset_dir / "preset.dtstyle").read_text()


def disable_modules(text, names):
    def repl(m):
        if m.group(1) not in names:
            return m.group(0)
        return m.group(0).replace("<enabled>1</enabled>", "<enabled>0</enabled>")
    return re.sub(r"<operation>(\w+)</operation>\s*<op_params>[0-9a-f]*</op_params>\s*<enabled>\d</enabled>",
                  repl, text)


def lut_relpath(preset_dir):
    s = style_text(preset_dir)
    m = re.search(r"<operation>lut3d</operation>\s*<op_params>([0-9a-f]*)</op_params>\s*<enabled>(\d)", s)
    if not m or m.group(2) != "1":
        return None
    return bytes.fromhex(m.group(1))[:512].split(b"\0")[0].decode()


def base_style(pid, pdir):
    """Bundled style with colisa disabled, written once to work/<pid>/style.dtstyle."""
    w = WORK / pid
    w.mkdir(parents=True, exist_ok=True)
    p = w / "style.dtstyle"
    if not p.exists():
        st = dtparams.read_style(style_text(pdir))
        if "colisa" in st:
            st["colisa"]["enabled"] = False
        p.write_text(dtparams.write_style(style_text(pdir), st))
    return p


def render_many(jobs):
    """jobs: (input, style_path, output, lut_root or None). Existing outputs are kept."""
    def one(j):
        inp, style, out, lut_root = j
        if Path(out).exists():
            return 0.0
        conf = [f"plugins/darkroom/lut3d/def_path={lut_root}"] if lut_root else []
        t, _ = dtrender.render(inp, style, out, RENDER, RENDER, extra_conf=conf)
        return t
    with ThreadPoolExecutor(JOBS) as ex:
        return list(ex.map(one, jobs))


# ---------- cube io ----------
def read_cube(path):
    vals, size = [], None
    for line in Path(path).read_text().splitlines():
        line = line.strip()
        if not line or line.startswith("#"):
            continue
        if line.startswith("LUT_3D_SIZE"):
            size = int(line.split()[1])
            continue
        if line.startswith(("DOMAIN", "TITLE", "LUT_1D")):
            continue
        vals.append([float(x) for x in line.split()])
    a = np.array(vals, dtype=np.float64)
    assert a.shape[0] == size ** 3, (path, a.shape, size)
    return a.reshape(size, size, size, 3)  # [b][g][r], red fastest


def write_cube(path, cube, comment="Omalux calibrated cube"):
    n = cube.shape[0]
    lines = [f"# {comment}", f"LUT_3D_SIZE {n}", "DOMAIN_MIN 0 0 0", "DOMAIN_MAX 1 1 1"]
    lines += [f"{r:.7f} {g:.7f} {b:.7f}" for r, g, b in cube.reshape(-1, 3)]
    Path(path).parent.mkdir(parents=True, exist_ok=True)
    Path(path).write_text("\n".join(lines) + "\n")


def identity_cube(n=LUT_SIZE):
    g = np.linspace(0, 1, n)
    b, gg, r = np.meshgrid(g, g, g, indexing="ij")
    return np.stack([r, gg, b], axis=-1)


# ---------- fixed-point update ----------
def scatter(a_rgb, resid, n):
    """Trilinear scatter of residuals into an n^3 grid. a_rgb (N,3) in [0,1]; resid (N,3)."""
    num = np.zeros((n, n, n, 3))
    den = np.zeros((n, n, n))
    x = np.clip(a_rgb * (n - 1), 0, n - 1 - 1e-6)
    i0 = np.floor(x).astype(int)
    f = x - i0
    for dr in (0, 1):
        for dg in (0, 1):
            for db in (0, 1):
                w = ((f[:, 0] if dr else 1 - f[:, 0]) * (f[:, 1] if dg else 1 - f[:, 1])
                     * (f[:, 2] if db else 1 - f[:, 2]))
                idx = (i0[:, 2] + db, i0[:, 1] + dg, i0[:, 0] + dr)
                np.add.at(num, idx, resid * w[:, None])
                np.add.at(den, idx, w)
    return num, den


def solve_update(num, den, lam=None, sweeps=60):
    """Laplacian-regularised least squares for the correction field u:
    minimise sum den*(u - d*)^2 + lam*sum|grad u|^2 with d* = num/den at populated nodes.
    The penalty acts on the correction, so contrast curves are kept while node noise
    (visible as blotches in smooth gradients) is suppressed. DT_CUBE_LAMBDA tunes lam."""
    if lam is None:
        lam = float(os.environ.get("DT_CUBE_LAMBDA", "150"))
    dstar = np.where(den[..., None] > 0, num / np.maximum(den, 1e-9)[..., None], 0.0)
    u = np.zeros_like(num)
    for _ in range(sweeps):
        up = np.pad(u, ((1, 1), (1, 1), (1, 1), (0, 0)), mode="edge")
        nb = np.zeros_like(u)
        for d in range(3):
            for sgn in (-1, 1):
                nb += np.roll(up, sgn, axis=d)[1:-1, 1:-1, 1:-1]
        u = (den[..., None] * dstar + lam * nb) / (den[..., None] + lam * 6)
    return u


def cube_update(cube, imgs, A, B, out_dir, input_dir, damping=0.7):
    num = np.zeros((LUT_SIZE,) * 3 + (3,))
    den = np.zeros((LUT_SIZE,) * 3)
    for i in imgs:
        b = B[i["id"]]
        o = common.read_rgb(out_dir / (i["id"] + ".jpg"), b.shape[1], b.shape[0])
        a = A[i["id"]]
        if a.shape[0] != o.shape[0] * o.shape[1]:
            a = common.read_rgb(input_dir / (i["id"] + ".png"), o.shape[1], o.shape[0]).reshape(-1, 3) / 255
        resid = (b.astype(np.float64) - o.astype(np.float64)).reshape(-1, 3) / 255
        n1, d1 = scatter(a, resid, LUT_SIZE)
        s = 1e5 / max(d1.sum(), 1)  # every image weighs the same
        num += n1 * s
        den += d1 * s
    return np.clip(cube + damping * solve_update(num, den), 0, 1)


def lut_inputs(pid, style_path, imgs, input_dir):
    """LUT-input renders (fixed per style prefix) and targets at render size."""
    in_style = input_dir.parent / "input.dtstyle"
    in_style.parent.mkdir(parents=True, exist_ok=True)
    in_style.write_text(disable_modules(Path(style_path).read_text(), POST_LUT | {"lut3d"}))
    render_many([(i["path"], in_style, input_dir / (i["id"] + ".png"), None) for i in imgs])
    A, B = {}, {}
    for i in imgs:
        p = input_dir / (i["id"] + ".png")
        aw, ah = common.im_size(p)
        A[i["id"]] = common.read_rgb(p, aw, ah).reshape(-1, 3).astype(np.float64) / 255
        B[i["id"]] = common.target(pid, i, RENDER)
    return A, B


def score_dir(pid, imgs, out_dir):
    scores = {i["id"]: score_render(pid, i, out_dir / (i["id"] + ".jpg")) for i in imgs}
    return float(np.mean(list(scores.values()))), scores


def fit_preset(pid, pdir, iters, resume=False, split="tuning", patience=3):
    rel = lut_relpath(pdir)
    if rel is None:
        print(f"[{pid}] no lut3d in style, skipping cube fit")
        return None
    w = WORK / pid
    lut_dir = w / "lut"
    cube_path = lut_dir / rel
    imgs = images(split)
    style = base_style(pid, pdir)
    t0 = time.time()
    A, B = lut_inputs(pid, style, imgs, w / "input")
    print(f"[{pid}] lut inputs {time.time() - t0:.0f}s")
    cube = read_cube(PRESETS / rel)
    if resume and (w / "best.cube").exists():
        cube = read_cube(w / "best.cube")
        print(f"[{pid}] resuming from best.cube")
    if cube.shape[0] != LUT_SIZE:
        cube = identity_cube()
    for stale in w.glob("iter-*"):
        shutil.rmtree(stale)
    history = []
    best = (None, 1e9)
    for k in range(iters + 1):
        write_cube(cube_path, cube, f"{pid} iteration {k}")
        out_dir = w / f"iter-{k}"
        t0 = time.time()
        render_many([(i["path"], style, out_dir / (i["id"] + ".jpg"), lut_dir) for i in imgs])
        rt = time.time() - t0
        mean, scores = score_dir(pid, imgs, out_dir)
        jp = float(np.mean([v for k2, v in scores.items() if k2.startswith("J")] or [np.nan]))
        rw = float(np.mean([v for k2, v in scores.items() if k2.startswith("R")] or [np.nan]))
        history.append(dict(iter=k, mean=mean, jpeg=jp, raw=rw, scores=scores))
        print(f"[{pid}] iter {k}: dE {mean:.2f} (jpeg {jp:.2f}, raw {rw:.2f}) render {rt:.0f}s", flush=True)
        if mean < best[1]:
            best = (k, mean)
            shutil.copy(cube_path, w / "best.cube")
        if k == iters or k - best[0] >= patience:
            break
        cube = cube_update(cube, imgs, A, B, out_dir, w / "input")
    json.dump(dict(preset=pid, best_iter=best[0], best=best[1], history=history),
              open(w / "best.json", "w"), indent=1)
    print(f"[{pid}] best iter {best[0]} dE {best[1]:.2f}")
    return best


def baseline(pids, split, tag="baseline"):
    res = {}
    for pid in pids:
        pdir = preset_dirs()[pid]
        imgs = images(split)
        out_dir = WORK / pid / tag
        t0 = time.time()
        render_many([(i["path"], pdir / "preset.dtstyle", out_dir / (i["id"] + ".jpg"), None) for i in imgs])
        _, scores = score_dir(pid, imgs, out_dir)
        res[pid] = scores
        by = {sp: [scores[i["id"]] for i in imgs if i["split"] == sp] for sp in ("tuning", "holdout")}
        fmt = lambda v: f"{np.mean(v):.2f}" if v else "-"
        print(f"{pid:36s} tuning {fmt(by['tuning'])}  holdout {fmt(by['holdout'])}  ({time.time() - t0:.0f}s)",
              flush=True)
        json.dump(res, open(WORK / f"{tag}.json", "w"), indent=1)
    return res


def finalize(pid, pdir):
    """Render every image with the best style/cube (tuned if present), score both splits."""
    w = WORK / pid
    rel = lut_relpath(pdir)
    tuned = w / "tuned"
    style = tuned / "preset.dtstyle" if (tuned / "preset.dtstyle").exists() else base_style(pid, pdir)
    cube = tuned / "look.cube" if style.parent == tuned else w / "best.cube"
    lut_dir = w / "final-lut"
    shutil.rmtree(lut_dir, ignore_errors=True)
    dst = lut_dir / rel
    dst.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy(cube, dst)
    out_dir = w / "final"
    shutil.rmtree(out_dir, ignore_errors=True)
    imgs = images("all")
    render_many([(i["path"], style, out_dir / (i["id"] + ".jpg"), lut_dir) for i in imgs])
    scores = {i["id"]: dict(split=i["split"], de=score_render(pid, i, out_dir / (i["id"] + ".jpg")))
              for i in imgs}
    summary = {sp: float(np.mean([x["de"] for x in scores.values() if x["split"] == sp] or [np.nan]))
               for sp in ("tuning", "holdout")}
    summary["all"] = float(np.mean([x["de"] for x in scores.values()]))
    tuned_info = json.load(open(tuned / "tuned.json")) if (tuned / "tuned.json").exists() else None
    json.dump(dict(preset=pid, summary=summary, scores=scores, style=str(style), cube=str(cube),
                   tuned=tuned_info, best=json.load(open(w / "best.json"))),
              open(w / "final.json", "w"), indent=1)
    print(f"[{pid}] final: tuning {summary['tuning']:.2f} holdout {summary['holdout']:.2f} "
          f"all {summary['all']:.2f}")


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("cmd", choices=["baseline", "fit", "fit-all", "final"])
    ap.add_argument("preset", nargs="?")
    ap.add_argument("--presets")
    ap.add_argument("--split", default="all")
    ap.add_argument("--iters", type=int, default=15)
    ap.add_argument("--resume", action="store_true")
    a = ap.parse_args()
    pd = preset_dirs()
    pids = a.presets.split(",") if a.presets else ([a.preset] if a.preset else sorted(pd))
    if a.cmd == "baseline":
        baseline(pids, a.split)
    elif a.cmd in ("fit", "fit-all"):
        for pid in pids:
            fit_preset(pid, pd[pid], a.iters, a.resume)
    elif a.cmd == "final":
        for pid in pids:
            finalize(pid, pd[pid])


if __name__ == "__main__":
    main()
