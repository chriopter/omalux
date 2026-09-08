#!/usr/bin/env python3
"""Progress report for the calibration: work/report/index.html.

One row per preset with target renderings: progress (0 % = score of the
bundled preset before fitting, 100 % = mean tuning score at or below the
target), tuning and holdout scores, cube roughness. Every preset has a
collapsed section with Original | Target | darktable per image, 1024-pixel
thumbnails loaded on demand, linking to the full files.

Serve OMALUX_CALIBRATION_ROOT/work over HTTP to browse it:
    python3 -m http.server -d "$OMALUX_CALIBRATION_ROOT/work" 8000
"""
import json
import subprocess
import sys
from concurrent.futures import ThreadPoolExecutor
from datetime import datetime, timezone
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from common import DATASETS, TARGETS, WORK, images, preset_dirs  # noqa: E402

REPORT = WORK / "report"
THUMBS = REPORT / "thumbs"
TARGET_DE = 2.0


def thumb(path: Path, key: str) -> Path:
    target = THUMBS / key / (path.stem + ".jpg")
    if not target.is_file() or target.stat().st_mtime < path.stat().st_mtime:
        target.parent.mkdir(parents=True, exist_ok=True)
        subprocess.run(["magick", str(path), "-auto-orient", "-resize", "1024x1024>", "-quality", "82",
                        str(target)], check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    return target


def rel(path: Path) -> str:
    return "../" + str(path.relative_to(WORK)) if path.is_relative_to(WORK) else str(path)


def progress(start, best):
    if best <= TARGET_DE:
        return 100.0
    return max(0.0, min(99.0, 100.0 * (start - best) / max(start - TARGET_DE, 1e-9)))


def main():
    imgs = images("all")
    rows, sections, jobs, pcts = [], [], [], []
    done = 0
    for pid in sorted(preset_dirs()):
        fin_path = WORK / pid / "final.json"
        if not fin_path.exists():
            pcts.append(0.0)
            rows.append(f"<tr class='pending'><td>{pid}</td><td><span class='pct'>pending</span></td>"
                        "<td>–</td><td>–</td><td>–</td><td>–</td></tr>")
            continue
        done += 1
        fin = json.load(open(fin_path))
        s, hist = fin["summary"], fin["best"]["history"]
        start, best = hist[0]["mean"], fin["best"]["best"]
        if fin.get("tuned"):
            best = min(best, fin["tuned"]["history"][-1])
        pct = progress(start, best)
        pcts.append(pct)
        rough = fin["best"].get("roughness")
        rows.append(f"<tr><td><a href='#p-{pid}'>{pid}</a></td>"
                    f"<td><div class='bar'><div style='width:{pct:.0f}%'></div></div><span class='pct'>{pct:.0f}%</span></td>"
                    f"<td>{start:.2f}</td><td>{s['tuning']:.2f}</td><td>{s['holdout']:.2f}</td>"
                    f"<td>{rough:.1f}</td></tr>" if rough is not None else
                    f"<tr><td><a href='#p-{pid}'>{pid}</a></td>"
                    f"<td><div class='bar'><div style='width:{pct:.0f}%'></div></div><span class='pct'>{pct:.0f}%</span></td>"
                    f"<td>{start:.2f}</td><td>{s['tuning']:.2f}</td><td>{s['holdout']:.2f}</td><td>–</td></tr>")
        figures = []
        for img in imgs:
            iid = img["id"]
            cells = []
            for title, path, key in (("Original", DATASETS / img["path"].relative_to(DATASETS)
                                      if img["path"].is_relative_to(DATASETS) else img["path"], "original"),
                                     ("Target", TARGETS / pid / f"{iid}.jpg", f"target/{pid}"),
                                     ("darktable", WORK / pid / "final" / f"{iid}.jpg", f"render/{pid}")):
                if not Path(path).is_file():
                    continue
                t = THUMBS / key / (Path(path).stem + ".jpg")
                jobs.append((Path(path), key))
                cells.append(f"<figure><a href='{rel(Path(path))}' target='_blank' rel='noopener'>"
                             f"<img src='{rel(t)}' loading='lazy' alt=''></a><figcaption>{title}</figcaption></figure>")
            sc = fin["scores"].get(iid)
            cap = f" — {sc['split']}, ΔE {sc['de']:.2f}" if sc else ""
            figures.append(f"<h3>{iid}{cap}</h3><div class='row'>{''.join(cells)}</div>")
        curve = " → ".join(f"{h['mean']:.2f}" for h in hist)
        sections.append(f"<details id='p-{pid}'><summary><b>{pid}</b> — {pct:.0f}% · start {start:.2f} → "
                        f"tuning {s['tuning']:.2f}, holdout {s['holdout']:.2f}</summary>"
                        f"<p class='note'>cube fit iterations: {curve}</p>{''.join(figures)}</details>")
    with ThreadPoolExecutor(8) as pool:
        list(pool.map(lambda j: thumb(*j), jobs))
    total = sum(pcts) / max(len(pcts), 1)
    generated = datetime.now(timezone.utc).strftime("%Y-%m-%d %H:%M UTC")
    page = f"""<!doctype html>
<html lang="en"><head><meta charset="utf-8"><meta http-equiv="refresh" content="120">
<title>Omalux calibration</title>
<style>
 body{{font:15px/1.5 system-ui;margin:2rem auto;max-width:1280px;padding:0 1rem;background:#111;color:#ddd}}
 h1{{font-size:1.4rem}} h2{{font-size:1.1rem;margin-top:2rem;border-bottom:1px solid #333;padding-bottom:.3rem}}
 h3{{font-size:.95rem;color:#aaa;margin:.8rem 0 .2rem}}
 table{{border-collapse:collapse;width:100%;font-size:13px}}
 td,th{{padding:.25rem .6rem;border-bottom:1px solid #2a2a2a;text-align:right;white-space:nowrap}}
 th:first-child,td:first-child{{text-align:left}} tr.pending td{{color:#666}}
 a{{color:#8ac}} .note{{color:#999;font-size:13px}}
 .bar{{display:inline-block;width:90px;height:8px;background:#2a2a2a;border-radius:4px;vertical-align:middle}}
 .bar div{{height:100%;background:#5a7;border-radius:4px}} .bar.big{{width:100%;height:14px;margin:.3rem 0 1rem}}
 .pct{{margin-left:.4rem;font-size:12px;color:#999}}
 details{{border:1px solid #2a2a2a;border-radius:6px;margin:.5rem 0;padding:.3rem .8rem}} summary{{cursor:pointer;padding:.3rem 0}}
 .row{{display:flex;gap:8px;margin:.3rem 0 .8rem;flex-wrap:wrap}}
 .row figure{{margin:0;flex:1;min-width:160px;max-width:32%}} img{{max-width:100%;border-radius:4px}}
 figcaption{{font-size:12px;color:#999;text-align:center}}
</style></head><body>
<h1>Omalux calibration</h1>
<p class="note">{generated}. Score: mean CIEDE2000 on 256-pixel proxies against the target rendering, lower is better, target ≤ {TARGET_DE}.
Progress: 0 % = bundled preset before fitting, 100 % = tuning score at or below the target. Holdout images were never fitted.</p>
<h2>Overall {total:.0f}% · {done} of {len(pcts)} presets computed</h2>
<div class='bar big'><div style='width:{total:.0f}%'></div></div>
<table><tr><th>Preset</th><th>Progress</th><th>ΔE start</th><th>ΔE tuning</th><th>ΔE holdout</th><th>cube roughness</th></tr>
{''.join(rows)}</table>
<h2>Per image (Original | Target | darktable)</h2>
{''.join(sections)}
</body></html>
"""
    REPORT.mkdir(parents=True, exist_ok=True)
    (REPORT / "index.html").write_text(page)
    print(f"{REPORT / 'index.html'} written ({done}/{len(pcts)} presets, {total:.0f}%)")


if __name__ == "__main__":
    main()
