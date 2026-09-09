# Calibrating the bundled looks

The scripts in [`tools/calibration/`](../tools/calibration) adjust a bundled preset (`preset.dtstyle` plus `look.cube`) until darktable's rendering of a set of photographs matches a target rendering of the same photographs. They need Python 3 with numpy, ImageMagick (`magick`) and `darktable-cli` on the path. Target renderings and datasets are not part of the repository.

## Data layout

One directory, named by `OMALUX_CALIBRATION_ROOT`:

```text
datasets/manifest.json     items: id, path (relative to datasets/), split, media_type
datasets/<path>            the photographs (JPEG or RAW)
datasets/proxies/<name>    optional smaller copies, used instead of the original
targets/<preset>/<id>.jpg  what the preset should produce for image <id>
work/                      everything the tools write
```

`split` is `tuning` or `holdout`. Fits only ever see tuning images; holdout images are scored once by the finished candidate and are never tuned on. Images above about 12 megapixels are best supplied as proxies: the score is computed on 256-pixel proxies anyway, and large inputs make darktable slow.

The score is the mean CIEDE2000 between the rendering and the target, both scaled to 256 pixels on the long edge. Lower is better; 2 is roughly where differences stop being visible on a full-size comparison.

## Rendering

`dtrender.py` converts a `.dtstyle` into an XMP history sidecar and runs `darktable-cli` on it, so no database import is needed. All images of one fit round share a style, so they are rendered by one `darktable-cli` process from a folder of links: process start-up (about two seconds) is paid once per round instead of once per image. Fit rounds also let darktable scale early instead of processing RAW files at full resolution (`--hq false`); that is several times faster and differs from full-quality output by a few tenths of ΔE, so final scoring and the baseline use full quality. `DT_OMP` sets the OpenMP threads per process (default 4; several presets can then run side by side on 16 cores). Each worker uses its own configuration directory under `work/dtcfg/`. Rendering is CPU-only by default: the pixelpipe takes a small fraction of the two seconds a `darktable-cli` process needs, and with several parallel processes a GPU reset on some drivers silently corrupts the output of the other processes. `DT_OPENCL=1` enables OpenCL with a CPU retry on failure.

## Recipe (scene-referred, current)

The looks are built from darktable modules that run before the tone mapper: exposure, color balance rgb, tone equalizer and sigmoid, plus local contrast, shadows and highlights, vignette, sharpening and grain. No cube. The result is an ordinary darktable style whose values can be read and edited in darktable.

```sh
export OMALUX_CALIBRATION_ROOT=/path/to/data
python3 tools/calibration/scene_search.py <preset>       # search the module parameters from neutral
python3 tools/calibration/cube_fit.py final <preset>      # render and score every image
python3 tools/calibration/report.py                       # work/report/index.html
python3 tools/calibration/install_preset.py <preset>      # style into presets/, cube and asset removed
bin/preset_preview <preset>                               # refresh the thumbnail
```

`scene_search.py` is coordinate descent over about forty parameters (see `PARAMS` in the file), one render round of the tuning images per trial, steps halving when a pass gains less than 0.05. Five passes take roughly an hour per preset on 16 cores; two presets can run side by side with `DT_OMP=8`. Why this instead of the cube: the targets behave like scene-referred processing (bright scenes and dark scenes get different treatment for the same display colour), which a display-referred cube after the tone mapper cannot express; see the findings below.

## Recipe (display cube, first round)

```sh
export OMALUX_CALIBRATION_ROOT=/path/to/data
python3 tools/calibration/cube_fit.py baseline            # score the bundled presets as they are
python3 tools/calibration/cube_fit.py fit <preset>        # fit the cube on tuning images
python3 tools/calibration/slider_tune.py <preset>         # then search the spatial sliders
python3 tools/calibration/cube_fit.py final <preset>      # render and score every image
python3 tools/calibration/report.py                       # work/report/index.html
python3 tools/calibration/install_preset.py <preset>      # copy into presets/
bin/preset_preview <preset>                               # refresh the thumbnail
```

`run_queue.sh [-j 3] [preset ...]` runs fit, one slider pass, final scoring and the report for many presets, three at a time by default; without ids it queues every preset that has targets and no `final.json` yet. Progress and per-preset logs are under `work/queue/`. With four presets side by side, budget about 7 minutes for a cube fit and 17 minutes for one slider pass per preset on 16 cores.

`cube_fit.py roughness <cube>` prints the mean absolute Laplacian of a cube in 8-bit units. Bundled cubes sit around 1 to 4; above about 10 the interpolation between nodes turns tiny input differences into blotches in smooth gradients, so a fitted cube that scores well but is rough is not finished.

### Cube fit

The cube is fitted by fixed-point iteration. The input to `lut3d` is rendered once per image (style with `lut3d` and every later module disabled). Each iteration renders the full style, scatters the residual between target and output trilinearly into the 33³ grid keyed by the LUT input, solves a Laplacian-regularised correction field on the grid (the penalty acts on the correction, so contrast curves survive while node noise, visible as blotches in smooth gradients, is suppressed), and adds it to the cube with damping. Iteration stops when the score has not improved for three rounds; the best cube is kept as `work/<preset>/best.cube`.

The fit runs with `colisa` disabled (`work/<preset>/style.dtstyle`). Global tone and colour are expressed by the cube; a display-referred contrast, brightness and saturation adjustment after the LUT fights the fit and, at saturation −1, removes any tint the cube adds.

### Slider search

Modules after `lut3d` with a spatial effect cannot be absorbed by the cube: shadows and highlights, vignette, sharpening, grain. `slider_tune.py` searches them by coordinate descent. Because the cube was fitted for the current slider values, every trial gets one cube update and a second render before it is judged; without that, leaving everything unchanged always wins. After each pass the cube is refitted for three iterations.

Parameters before `lut3d` (exposure, black level) are not searched: any change there is undone by the cube once it is refitted.

## What we learned

These came out of the first darktable round and shape the tools. Re-read them before changing the fit.

- **A display-referred module after the LUT fights the fit.** With `colisa` at saturation −1 behind `lut3d`, every tint the cube added was removed again and the fit stalled at ΔE 4. Disabling `colisa` let the same preset reach 2.4 in three iterations. The cube expresses global tone and colour; keep modules after it for spatial work only.
- **Accumulated residuals make rough cubes.** Averaging residuals per node and adding them up produced cubes 15 to 60 times rougher than the originals; the pictures showed it as blotchy skies although the score kept improving. The regularised solve fixed both the roughness and the score.
- **Sliders and cube are coupled.** After the cube is fitted, any slider change first makes the score worse, whichever direction it goes, because the cube compensated the old value. Trials must include a cube update, and only sliders the cube cannot absorb are worth searching.
- **Exposure before the LUT is not a parameter.** Changing it just shifts the cube input; after a refit the result is the same. Leave it as the look defines it.
- **RAW stays about 2 ΔE behind JPEG** with the same cube. Adding darktable's +0.7 EV RAW default on top of the style's exposure (`RAW_EXPOSURE_OFFSET`), or switching the RAW workflow to filmic, display-referred or none, does not close the gap; the scene-referred sigmoid default is the best base among them. The remaining difference is in how the RAW base is developed, not in the look, and needs a base rendition shared by all presets rather than per-preset fitting.
- **darktable applies a style's exposure absolutely.** A RAW opened with the +0.7 EV default and then given a style with exposure +0.4 ends at +0.4, not +1.1. The application behaves the same way, so calibrate against that behaviour rather than against a relative reading of the value.
- **The score is blind to noise.** Mean ΔE on 256-pixel proxies, and even on 1024-pixel renders, prefers a noisy render with the right tone over a clean one with a small offset. A slider search can exploit that (a tiny local-contrast radius scored better while amplifying noise). Look at full-size pairs before accepting a result.
- **Parallel GPU renders are not safe on every driver.** A GPU reset in one darktable-cli process left the others' output dark and green-tinted without any error. Rendering on the CPU costs about 15 % because process start-up dominates.
- **Clarity and noise reduction did not survive the one-time conversion.** The archived presets carry `clarity` and luminance/colour noise reduction; the converted styles had neither. The fit now seeds darktable's local contrast (`bilat`, detail = clarity/100) and `nlmeans` (luma and chroma = value/100) from `tools/calibration/preset-seeds.json`, and the slider search may move local contrast. A wrong local-contrast value cannot be judged with the cube fixed: it shifts tones that the cube had compensated, so the coupled trial is the only fair test.
- **The targets are scene-referred, a display cube is not.** For several looks the target brightens one image and leaves another with the same display colours unchanged; a cube fitted to one image alone explains it to about ΔE 1, all images together do not, and stronger cube regularisation (λ 600, 1500) or a ridge toward no correction changes nothing. The same display value comes from different scene values in different images, and the look acts on the scene values. Hence the second recipe: express the look with modules before the tone mapper. The first full cube round ended at tuning 3.78 / holdout 6.77 over 27 presets.
- **Check the reference images too.** One holdout target turned out to be a dark, letterboxed miniature rather than a rendering; it distorts that preset's holdout mean for every preset alike.

## Rules

- Tune on tuning images only. Judge holdout images once, with the finished candidate.
- Change presets, not the engine. darktable stays unchanged upstream source.
- Inspect full-size pairs before calling a preset finished; the score is a guide, not the verdict.
- Regenerate the thumbnail after installing a preset.

`tools/calibration/preset-seeds.json` retains only the three calibration inputs needed from the former presets (clarity and luminance/colour noise reduction), extracted from commit `1d5eaf6`. Calibration no longer depends on the removed Rust application.
