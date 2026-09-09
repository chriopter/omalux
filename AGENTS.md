# Working on Omalux

The repository is being prepared for an independent Qt/QML interface using darktable.

- The website lives in the separate repository https://github.com/chriopter/omalux.org.
- `darktable/` is the official upstream Git submodule, pinned to a stable release. Initialize it with `git submodule update --init --recursive`.
- `omalux/` contains the Qt/QML UI, C/C++ native adapter and Python development launcher. The registered editing controls, crop, presets and JPEG/PNG export are connected to darktable; the History pane displays the current image’s native processing stack. See `dev/docs/reference/controls.md` for the mappings and limitations. Do not describe planned functionality as released.
- Keep the adapter and UI separate from upstream. Necessary internal engine changes are authorized, but keep them minimal and documented.
- The native adapter uses internal structures: always compile against the exact installed darktable release headers. Never silently reuse a binary after a library upgrade.
- Before changing engine integration, read `dev/docs/architecture/darktable.md` and verify its source references against the current darktable pin. It records known prototype limitations, including the interactive FULL-pipe cache and presentation-epoch contract, transient GPU error flags, and the deprecated `colisa` controls.
- Bundled darktable looks live in `presets/<id>/` with `preset.dtstyle`, `thumbnail.jpg` and `preset.json`. Regenerate thumbnails explicitly with `dev/scripts/preset_preview <id>` after style changes; use the real engine and shared beach photograph.
- Declare preset dependencies in `preset.json` using `assets` (`path`, `role`, optional `target`). Style and thumbnail filenames are fixed conventions. Record the repository-relative source image under `preview.source` and rendering engine version under `preview.darktable_version` automatically when generating thumbnails; these are provenance, not runtime dependencies or compatibility requirements. Preserve asset declarations when regenerating thumbnails. Unsupported or missing dependencies must not silently produce a different look.
- `assets/logo/`, `assets/images/` and `assets/icons/` contain shared visual assets. Reuse the existing beach photograph.
- Credit darktable and its contributors. Do not imply official affiliation or endorsement.
- Website and README screenshots show the current darktable-based development UI. Generate them with `scripts/screenshots.sh` in the website repository; keep development-preview labels accurate.
- `dev/scripts/update` checks out the latest upstream stable release and its dependencies; it never stages, commits or pushes.
- `dev/tools/calibration/` fits bundled looks (`preset.dtstyle` + `look.cube`) to target renderings through `darktable-cli`; `dev/docs/development/calibration.md` has the data layout and recipe. Fits use tuning images only; holdout images are judged once by a finished candidate. Datasets and targets are not in the repository.

## Documentation layout

- Keep all project documentation in the `dev/docs/` directory. Use `development/` for setup and calibration workflows, `architecture/` for engine internals and source audits, and `reference/` for controls, presets and assets. Keep `dev/docs/README.md` as the entry point.
- Only the main `README.md` and this single `AGENTS.md` belong outside `dev/docs/`. Do not create component READMEs, documentation directories outside `dev/docs/` or additional agent instruction files.
- License and third-party notices stay with their code. Do not rearrange documentation inside the unchanged `darktable/` submodule or generated/dependency directories.
- Update incoming links and relative source links whenever moving a document.

## Interface rules (`omalux/`)

1. **Match darktable's slider names and units.** Use the corresponding darktable UI label and displayed unit; do not rename an operation or invent a percentage/normalized scale. Match its range, default and displayed precision. Verify the pinned module's `gui_init`, introspection and Bauhaus formatting before adding or changing a control. Internal parameter conversion is allowed only to reproduce darktable's own displayed scale (for example percent or EV). Keep translated labels equivalent in the same language.
2. Define controls centrally in `omalux/native/engine/controls.h`. Qt, the native adapter and split synchronization must agree on values. Distinguish displayed values from raw parameters and GTK action paths.
3. Read `dev/docs/architecture/darktable.md` before engine changes and verify relevant findings against the installed/pinned source. Keep upstream changes explicit and minimal.
4. Preserve imported image settings and identify module instances deliberately when extending the prototype. Do not silently treat registry defaults as the original image history.
5. Keep native responsibilities separate: `omalux/native/app/` contains the C++ Qt facade, typed worker queue and application services; `omalux/native/engine/` contains the C adapter behind its opaque `OmEngine` API; `omalux/native/dev/` contains optional smoke/capture drivers. `main.cpp` only composes the application. Implement behavior in `.cpp`/`.c` files, keep headers to interfaces and immutable registry definitions, pass engine state explicitly, and never access editor presentation state from the worker. Format native implementation files with `omalux/native/.clang-format`; preserve the compact control registry layout.
6. Keep each implemented sidebar pane in its own file under `omalux/ui/panels/`. Shared visual components belong in `omalux/ui/components/`; pass data through explicit properties and actions through signals. Only `Main.qml` and the sidebar composition wire the backend; child components must not reach into global `editor` or window IDs.
7. Keep special control adapters explicit: white balance uses darktable’s temperature/color math, denoise curves and blend changes synchronize as module snapshots. Do not substitute a display-only effect or assume a native field name is a GTK action path.
8. Verify affected controls with the actual engine and comparison path. Do not claim identical output or performance without measuring it under matching settings.
