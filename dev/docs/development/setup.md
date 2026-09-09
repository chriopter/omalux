# Development

Commands run from the repository root. The original Rust application is preserved in [release v0.2.0](https://github.com/chriopter/omalux/releases/tag/v0.2.0) and Git history.

## Fetching darktable

For a new checkout:

```sh
git clone --recurse-submodules https://github.com/chriopter/omalux.git
```

For an existing checkout, run from the repository root:

```sh
git submodule update --init --recursive
```

The submodule points directly to the official upstream repository. Keep it at the recorded commit; its nested submodules are pinned by darktable. Updating this source does not install darktable or change the system application.

## Updating darktable

Requires Git and the GitHub CLI (`gh`). Run from the repository root:

```sh
dev/scripts/update
git diff --submodule=short
```

The script checks out the latest official stable release and its nested submodules. It stops if darktable has local changes. After trying the new version, pin it with `git add darktable` and commit. The script does not stage, commit or push.

## Development scripts

Run from the repository root:

- `dev/scripts/start [image]` — build and open Omalux with the image; defaults to `assets/images/beach-volleyball.jpg`. `--input image` also works.
- `dev/scripts/start_split [image]` — open Omalux and the original darktable window with the same image. Slider changes and resets in Omalux also update the comparison window. Closing Omalux stops both.
- `dev/scripts/preset_preview <folder>` / `dev/scripts/preset_preview --all` — regenerate a bundled preset’s beach thumbnail and preview source/engine version in `preset.json`, e.g. `dev/scripts/preset_preview chromatic` (requires ImageMagick).
- `dev/scripts/update` — check out the latest stable darktable release and its dependencies; review and commit the new pin yourself.

```sh
dev/scripts/start
dev/scripts/start "/path/to/photo.CR3"
dev/scripts/start --input "/path/to/photo.jpg"
dev/scripts/start_split "/path/to/photo.jpg"
```

The UI follows the original dark Omalux layout, with SVG sidebar tabs, grouped controls, colored slider tracks and expandable details. Controls use **darktable’s names, units and precision**. Exposure, local contrast, shadows/highlights, white balance, color grading, bloom, grain, vignetting, sharpening, profiled denoising and LUT opacity are connected. See the [v0 control mapping](../reference/controls.md) for the exact modules and approximations.

The **Presets** tab keeps v0’s groups, search, thumbnails and expandable module details. Applying a preset restores the photo’s opening state before applying the look, so earlier session edits do not leak into it. Save the current look with its thumbnail and LUT, export a bundle, or delete an own preset. Styles may affect modules without an Omalux control. The **Crop & Rotate** tab provides a draggable crop frame, aspect ratios and rotation; the information tab shows image metadata. Open photographs from the toolbar and export full-resolution JPEG or PNG. The **History** tab displays darktable’s processing stack, newest first, including module enablement and the current step. Click a step (or original) to restore it. Later steps remain selectable until a new edit replaces the future branch using darktable’s history rules.

| Keys | Action |
| --- | --- |
| `1` / `2` / `3` / `4` / `5` | Filters / presets / crop / history / metadata |
| `Tab` / `Shift+Tab`, `]` / `[` | Next / previous panel |
| `↑` / `↓`, `K` / `J` | Select parameter |
| `←` / `→`, `H` / `L` | Adjust selected parameter; Shift makes larger steps |
| `R` | Reset selected parameter to its darktable default |
| `G` / `S` / `M`, `A` | Grain strength / coarseness / mid-tones bias, grain details |
| `+` / `−`, `0`, `F` | Zoom, fit, photograph fullscreen |
| `O` / `Ctrl+O`, `Ctrl+S` | Open photograph, export |
| `?` / `F1` | Keyboard reference |

Double-click a slider’s value for numeric entry across its full darktable range. Reset arrows affect a parameter or its group. Keyboard editing pauses in text fields and dialogs. Zoom currently magnifies the interactive preview, not a full-resolution detail render.

The launcher builds the C/C++ adapter on demand using `cc`, `c++`, `pkg-config` and Qt 6’s `moc` and `qsb` (Qt Shader Tools). It needs Python 3, Git, Qt 6 Quick/Quick Controls, and development headers for GTK 3, JSON-GLib, Little CMS, SQLite, Lua and librsvg. The current Linux build expects Qt tools under `/usr/lib/qt6/` and an installed release build of darktable 5.6.0 or 5.6.1. It does not build the darktable submodule itself.

The build extracts the **matching installed release’s headers** into ignored `omalux/build/` (fetching its official tag if needed); it never changes the submodule checkout. Override `DARKTABLE_LIBRARY`, `DARKTABLE_BIN`, `DARKTABLE_MODULEDIR` and `DARKTABLE_DATADIR` for another matching installation. Unsupported versions are rejected until the adapter has been reviewed for their internal ABI.

The darktable engine runs inside the Qt application on a dedicated worker thread. It keeps the develop context, decoded image cache and pixelpipe alive; slider changes update the selected darktable modules using internal parameter introspection and specialized adapters. Only the newest requested value is queued, and obsolete frames are discarded. Preview buffers are copied directly into QImage, without XMP reloads, image export or JPEG encoding. The full preview fits within 1400 × 1000 pixels; slider gestures use 700 × 500 previews. Rendering uses darktable’s automatic OpenCL selection, with CPU fallback. A visible warning reports unavailable or failed GPU acceleration. Zooming still magnifies the preview rather than rendering full-resolution details.

For AMD GPUs using Mesa Rusticl, install `opencl-mesa`; the dev launcher defaults `RUSTICL_ENABLE` to `radeonsi` unless you override it. The UI status “OpenCL auto” indicates automatic device selection, not that every module ran on the GPU.

Each launch uses temporary config, cache and database directories, with source sidecar writes disabled. Session edits are discarded on close unless explicitly exported as an image or saved as a preset. Window placement follows your desktop rules. On the development machine, Omalux opens silently on workspace 3 and the darktable comparison window on workspace 4.

Split mode runs two independent instances of darktable’s engine with separate databases. Omalux publishes style events and coalesced control snapshots to an atomic session file; `omalux/comparison.lua` polls it every 50 ms and applies scalar values through darktable’s GUI actions when the darkroom is open. Curve and blend changes use temporary single-module styles. Omalux never waits for the comparison render. Synchronization is one-way and covers the registered controls, module enablement, curve/recipe updates, image changes and compatible styles; changes made in darktable do not flow back. Two engines consume additional RAM/GPU resources and can compete for processing time. The comparison installation needs Lua support; bridge failures are logged in the launching terminal.

### Application structure

- `omalux/native/main.cpp` — application startup only.
- `omalux/native/app/` — C++ Qt facade, render worker, preset catalogue, export and comparison bridge.
- `omalux/native/engine/` — C adapter with an explicit engine context; no Qt dependencies.
- `omalux/native/dev/` — optional smoke tests, screenshot capture and batch preview drivers.
- `omalux/ui/Main.qml` — window layout, backend wiring and keyboard shortcuts.
- `omalux/ui/EditorSidebar.qml` — tabs, panel selection and panel/backend connections.
- `omalux/ui/panels/` — separate Filters, Presets, Geometry, History and Metadata panes.
- `omalux/ui/components/` — reusable sliders, preset cards, toolbar, image viewport, GPU notice, crop overlay, dialogs, keyboard bindings and status bar; `EditorTheme.qml` holds shared colors and typography.

Give each new sidebar pane its own file. Panels receive data through properties and emit action signals; shared components do not access the global backend. Engine work stays in the native adapter and its worker thread.

### Adding a slider

Add one row to [`omalux/native/engine/controls.h`](../../../omalux/native/engine/controls.h): ID, label, darktable module and float parameter name, UI minimum/maximum/step/default, scale/offset (`parameter = UI value × scale + offset`), unit suffix, decimal places, section, GTK action path, track colors, detail visibility and optional soft limits. Match darktable’s own slider label and displayed scale; the current colisa controls are unitless −1.00 to +1.00, not percentages. The QML sliders, native parameter lookup and split-mode messages all use this definition; no new Qt property or Lua mapping is needed. Verify the parameter type/range and GUI action in the matching darktable source first. Float parameters use introspection; integer fields, enablement, blend opacity, white balance and denoise curve ordinates have explicit native adapters. New special types require source and ABI review.

`editor.setControl(id, value)` queues a complete parameter snapshot. Per-control revisions identify actual changes; the engine updates only those parameters and adds history once per affected module. Startup and style application read values from darktable. Presets restore a per-image opening baseline before applying their own settings. Rendering alone does not overwrite style values or enable unchanged modules. The split bridge receives the same revisions and values; style boundaries retain intervening control snapshots, so coalescing does not lose earlier edits.

See the [darktable source analysis](../architecture/darktable.md) for lifecycle, pixelpipe caching, color management, history, GPU reporting and export. The interactive FULL pipe reuses intermediate results; slider drags use reduced previews and release requests full preview quality. The colisa controls remain deprecated upstream. Color management, complex masks/instances and exact pixel parity between separate render contexts still have documented limitations.

