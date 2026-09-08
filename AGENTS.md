# Working on Omalux

Omalux is a Rust photo developer: a scene-referred pipeline from decoded
image to finished file, a command line, and a Qt/QML desktop app. This file
is for anyone, human or agent, changing it.

## Where things are

```
presets/builtin/        one folder per look: preset.json, thumbnail.jpg, reference.json
src/preset/             preset format, versions, catalogue of built-ins
src/develop/            the pipeline: settings/ (data), pipeline/stages/ (processing),
                        render/ (scene to display), kernels.rs (shared convolutions)
src/io/                 decoding (raw/, raster/), colour management, encoding
src/job/                the develop job: runner, production, reports
src/command/            the omalux command line
crates/omalux-gui/      the desktop app (Rust backend, QML in qml/)
tools/calibration/      scripts that tune presets against target renderings
omalux.org/             Astro website, static images, and Cloudflare configuration
tests/                  integration tests; docs/ the design notes
```

## Rules that are easy to break

- A built-in preset must be byte-exact canonical: compact JSON plus one line
  feed, as `omalux presets canonicalize` writes it. The catalogue refuses to
  load otherwise, and then every preset command fails at run time. Run
  `cargo test --test develop_catalog` after touching `presets/builtin`.
- Presets are embedded at build time. Rebuild before judging a preset change
  with the binary.
- Stored preset pictures are updated explicitly with `cargo run --release
  --example preset_references -- --write [preset-id ...]`. Verify without
  overwriting with `--check`; normal GUI builds only embed the thumbnails.
- The renderer is changed only for real defects. Matching a look is done in
  preset values; a renderer constant fitted to one preset is a bug in waiting.
- Look-matching tunes only on tuning images. Holdout images are judged once,
  by a finished candidate, and never tuned on.
- Gates before every commit: `cargo fmt --all --check`,
  `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo test --workspace` (and `--features heic` for the CLI tests).
- Commit messages say what changed and why, in prose. No attribution
  trailers.

## Calibration tools

`tools/calibration/README.md` describes the data layout and the recipe:
optimize sliders and curves, derive the colour table, choose its strength,
polish, validate, audit. The tools need Python 3 with numpy and ImageMagick,
and one directory named by `OMALUX_CALIBRATION_ROOT` holding the images,
their target renderings and the release binary. Target renderings and datasets
are not part of the repository.
