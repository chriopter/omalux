# Calibration tools

Scripts that tune a built-in look until Omalux renders a set of images the way
a target rendering of each image looks. They need Python 3 with numpy, and
ImageMagick (`magick`) on the path.

## Layout

Everything lives under one directory, named by `OMALUX_CALIBRATION_ROOT`:

```
datasets/manifest.json   items with "id", "path", "media_type" and "split"
datasets/<path>          the image files the manifest points to
datasets/skies.json      optional: {"<id>": share} for frames with a clear sky
targets/<preset>/<id>.*  how each image should come out under that preset
work/<preset>/           what the tools write for one preset
bin/omalux               the release build the tools render with
```

`split` is `tuning` or `holdout`. Every script here reads tuning images only.
The holdout exists so that a finished calibration can be judged once on
images it has never seen; nothing in this directory touches it.
`OMALUX_BINARY` overrides the binary.

## Workflow

```
calibrate_all.py [preset-id...]     the whole recipe per preset, resumable
preset_optimizer.py <id> [...]      tune sliders and curves against the targets
fit_color_table.py <id> [size]      derive the colour table from what is left
retune_strength.py <id>             pick the table strength whose skies stay smooth
validate_preset.py <id>             full-resolution metrics on the tuning images
audit_preset.py <id>                per-tile findings a person would notice
sky_smoothness.py <id>              the arc a colour table can leave in a sky
install_presets.sh <id>...          copy work/<id>/best.json into presets/builtin
```

`calibrate_all.py` runs the optimizer from two starting points, the archived
preset and neutral, keeps the better, fits the colour table, chooses its
strength, polishes once more with the table held fixed, then validates and
audits. It records each preset in `work/ledger.json` and skips presets already
done, so it can be stopped and resumed.

After installing a changed preset, explicitly regenerate its stored pictures
with `cargo run --release --example preset_references -- --write <id>` and review
them. Use `--check` to verify the references without replacing them; see
[preset pictures](../../docs/preset-previews.md).

## What the objective sees

Mean colour difference alone is blind to a colour cast across the whole
frame, to a picture flatter than its target, to highlights sitting too high,
and to a region whose hue tips the other way. `preset_optimizer.py` prices
each of those on top of the colour difference; the comments there say why.
Curve nodes at non-negative input are held at or above zero, because a curve
that maps a tone below black paints bands where it crosses zero.
