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

`dtrender.py` converts a `.dtstyle` into an XMP history sidecar and runs `darktable-cli` on it, so no database import is needed and renders run in parallel (`DT_JOBS`, default 6). Each worker uses its own configuration directory under `work/dtcfg/`. Rendering is CPU-only by default: the pixelpipe takes a small fraction of the two seconds a `darktable-cli` process needs, and with several parallel processes a GPU reset on some drivers silently corrupts the output of the other processes. `DT_OPENCL=1` enables OpenCL with a CPU retry on failure.

## Recipe

```sh
export OMALUX_CALIBRATION_ROOT=/path/to/data
python3 tools/calibration/cube_fit.py baseline            # score the bundled presets as they are
python3 tools/calibration/cube_fit.py fit <preset>        # fit the cube on tuning images
python3 tools/calibration/slider_tune.py <preset>         # then search the spatial sliders
python3 tools/calibration/cube_fit.py final <preset>      # render and score every image
python3 tools/calibration/install_preset.py <preset>      # copy into presets/
bin/preset_preview <preset>                               # refresh the thumbnail
```

### Cube fit

The cube is fitted by fixed-point iteration. The input to `lut3d` is rendered once per image (style with `lut3d` and every later module disabled). Each iteration renders the full style, scatters the residual between target and output trilinearly into the 33³ grid keyed by the LUT input, solves a Laplacian-regularised correction field on the grid (the penalty acts on the correction, so contrast curves survive while node noise, visible as blotches in smooth gradients, is suppressed), and adds it to the cube with damping. Iteration stops when the score has not improved for three rounds; the best cube is kept as `work/<preset>/best.cube`.

The fit runs with `colisa` disabled (`work/<preset>/style.dtstyle`). Global tone and colour are expressed by the cube; a display-referred contrast, brightness and saturation adjustment after the LUT fights the fit and, at saturation −1, removes any tint the cube adds.

### Slider search

Modules after `lut3d` with a spatial effect cannot be absorbed by the cube: shadows and highlights, vignette, sharpening, grain. `slider_tune.py` searches them by coordinate descent. Because the cube was fitted for the current slider values, every trial gets one cube update and a second render before it is judged; without that, leaving everything unchanged always wins. After each pass the cube is refitted for three iterations.

Parameters before `lut3d` (exposure, black level) are not searched: any change there is undone by the cube once it is refitted.

## Rules

- Tune on tuning images only. Judge holdout images once, with the finished candidate.
- Change presets, not the engine. darktable stays unchanged upstream source.
- Inspect full-size pairs before calling a preset finished; the score is a guide, not the verdict.
- Regenerate the thumbnail after installing a preset.
