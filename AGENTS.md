# Working on Omalux

The repository is being prepared for an independent Qt/QML interface using darktable.

- `omalux-v0/` contains the original application and Rust engine. The archive rules below apply there. Run Cargo commands from that directory.
- `omalux.org/` contains the website; its rules are collected below.
- `darktable/` is the official upstream Git submodule, pinned to a stable release. Initialize it with `git submodule update --init --recursive`.
- `omalux/` contains the Qt/QML UI, C/C++ native adapter and Python development launcher. Brightness, contrast, saturation and the preset catalogue are connected to darktable; other editing tools remain placeholders. Do not describe planned functionality as released.
- Keep the adapter and UI separate from upstream. Necessary internal engine changes are authorized, but keep them minimal and documented.
- The native adapter uses internal structures: always compile against the exact installed darktable release headers. Never silently reuse a binary after a library upgrade.
- Before changing engine integration, read `docs/darktable-architecture.md` and verify its source references against the current darktable pin. It records known prototype limitations, including the `IMAGE` flag disabling intermediate cache reuse, transient GPU error flags, and the deprecated `colisa` controls.
- Bundled darktable looks live in `presets/<id>/` with `preset.dtstyle`, `thumbnail.jpg` and `preset.json`. Regenerate thumbnails explicitly with `bin/preset_preview <id>` after style changes; use the real engine and shared beach photograph.
- Declare preset dependencies in `preset.json` using `assets` (`path`, `role`, optional `target`). Style and thumbnail filenames are fixed conventions. Record the repository-relative source image under `preview.source` and rendering engine version under `preview.darktable_version` automatically when generating thumbnails; these are provenance, not runtime dependencies or compatibility requirements. Preserve asset declarations when regenerating thumbnails. Unsupported or missing dependencies must not silently produce a different look.
- `assets/logo/` and `assets/images/` contain shared visual assets. Reuse the existing beach photograph.
- Credit darktable and its contributors. Do not imply official affiliation or endorsement.
- Website screenshots currently depict v0 and must be labelled accordingly.
- `bin/update` checks out the latest upstream stable release and its dependencies; it never stages, commits or pushes.
- `tools/calibration/` fits bundled looks (`preset.dtstyle` + `look.cube`) to target renderings through `darktable-cli`; `docs/calibration.md` has the data layout and recipe. Fits use tuning images only; holdout images are judged once by a finished candidate. Datasets and targets are not in the repository.

## Documentation layout

- Keep all project documentation in the repository-root `docs/` directory. Current documentation lives directly there; archived engine documentation lives under `docs/v0/`.
- Only the main `README.md` and this single `AGENTS.md` belong outside `docs/`. Do not create component READMEs, nested `docs/` directories or additional agent instruction files.
- License and third-party notices stay with their code. Do not rearrange documentation inside the unchanged `darktable/` submodule or generated/dependency directories.
- Update incoming links and relative source links whenever moving a document. Commands in archived documentation still run from `omalux-v0/`.

## Interface rules (`omalux/`)

1. **Match darktable's slider names and units.** Use the corresponding darktable UI label and displayed unit; do not rename an operation or invent a percentage/normalized scale. Match its range, default and displayed precision. Verify the pinned module's `gui_init`, introspection and Bauhaus formatting before adding or changing a control. Internal parameter conversion is allowed only to reproduce darktable's own displayed scale (for example percent or EV). Keep translated labels equivalent in the same language.
2. Define controls centrally in `omalux/native/controls.h`. Qt, the native adapter and split synchronization must agree on values. Distinguish displayed values from raw parameters and GTK action paths.
3. Read `docs/darktable-architecture.md` before engine changes and verify relevant findings against the installed/pinned source. Keep upstream changes explicit and minimal.
4. Preserve imported image settings and identify module instances deliberately when extending the prototype. Do not silently treat registry defaults as the original image history.
5. Keep each implemented sidebar pane in its own file under `omalux/ui/panels/`. Shared visual components belong in `omalux/ui/components/`; pass data through explicit properties and actions through signals. Only `Main.qml` and the sidebar composition wire the backend; child components must not reach into global `editor` or window IDs.
6. Verify affected controls with the actual engine and comparison path. Do not claim identical output or performance without measuring it under matching settings.

## Archived engine rules (`omalux-v0/`)

These rules and relative paths apply only to changes inside the archived Rust application. Moving its contributor instructions does not require rebuilding the archive.

### Where things are

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
../omalux.org/          Astro website, static images, and Cloudflare configuration
tests/                  integration tests; ../docs/v0/ the design notes
```

### Rules that are easy to break

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

### Calibration tools

`docs/v0/calibration-tools.md` (from the repository root) describes the data layout and the recipe:
optimize sliders and curves, derive the colour table, choose its strength,
polish, validate, audit. The tools need Python 3 with numpy and ImageMagick,
and one directory named by `OMALUX_CALIBRATION_ROOT` holding the images,
their target renderings and the release binary. Target renderings and datasets
are not part of the repository.

## Website rules (`omalux.org/`)

### Development

When starting the dev server, use background mode:

```
astro dev --background
```

Manage the background server with `astro dev stop`, `astro dev status`, and `astro dev logs`.

### Documentation

Full documentation: https://docs.astro.build

Consult these guides before working on related tasks:

- [Adding pages, dynamic routes, or middleware](https://docs.astro.build/en/guides/routing/)
- [Working with Astro components](https://docs.astro.build/en/basics/astro-components/)
- [Using React, Vue, Svelte, or other framework components](https://docs.astro.build/en/guides/framework-components/)
- [Adding or managing content](https://docs.astro.build/en/guides/content-collections/)
- [Adding styles or using Tailwind](https://docs.astro.build/en/guides/styling/)
- [Supporting multiple languages](https://docs.astro.build/en/guides/internationalization/)
