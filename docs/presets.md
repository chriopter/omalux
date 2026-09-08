# Bundled looks

Omalux calls these looks **presets** in its UI. Each `.dtstyle` is a real darktable **style**, combining settings from multiple modules, and can also be imported in darktable's styles panel.

## The four bundle files

| File | Purpose | Needed to apply the look? |
| --- | --- | --- |
| `preset.dtstyle` | darktable style: module names, enabled states, versions, parameter values and blending settings. It also references the LUT when the look uses one. | Yes. |
| `look.cube` | A 3D lookup table mapping input RGB colors to output RGB colors. For the converted v0 looks it contains the combined color adjustments, applied through darktable's `lut3d` module. It is not an image or camera profile. | Yes, when referenced by the style; not every preset needs one. |
| `thumbnail.jpg` | Stored beach-photo preview rendered with this style; displayed in the preset list. | No; without it the UI displays “No preview”. |
| `preset.json` | Bundle dependency declarations plus thumbnail generation metadata. The launcher resolves declared assets before starting the engines. | Required to declare dependencies; optional for a standalone style without assets. |

The style and its referenced LUT produce the look. The JPEG is its preview; the manifest declares dependencies and records how the preview was generated. Keep the LUT beside the style; it is not embedded in the `.dtstyle` file.

## Using and adding presets

Each bundled look has its own folder, following the v0 layout with a darktable style instead of the old engine's JSON:

```text
presets/
  chromatic/
    preset.dtstyle
    thumbnail.jpg
    preset.json
```

Create another folder with `preset.dtstyle` and restart `bin/dev` or `bin/dev_split`. Each style needs a unique name. Bundles may be grouped in nested folders. Loose `.dtstyle` files still work, but a bundle keeps its preview with its settings. The UI discovers files automatically and provides search. In the separate **Presets** tab (second sidebar icon), compact rows as in v0 show a 96×64 beach preview beside the name; click the thumbnail or name to apply. The separate arrow expands the actual module settings. Neutral and Chromatic stay above the groups. As in v0, Monochrome opens initially, only one group is expanded at a time, series use their folder names, and Experimental sorts last. Search includes group names and reveals matching presets across collapsed groups. Applying sends all its module settings to darktable, including modules without an Omalux control. No QML button, slider definition or Lua mapping is needed for another style.

The expanded card reads module names, enabled state and parameter descriptions from the installed engine and values from the actual style blob. It includes stored inactive parameters too. Known controls use darktable's display units; other fields show stored engine values, not guessed GUI conversions. Arrays and structured values are shown in full; untyped data is shown as hex. Blending is currently summarized, not individually editable in the inspector.

Malformed files, duplicate names and unavailable or incompatible module layouts are displayed as unavailable with a reason. The adapter requires matching module versions and parameter sizes; it does not yet migrate old styles. Custom module ordering and styles containing drawn-mask records are rejected rather than partially applied. Complex duplicate module instances and raster-mask dependencies still require separate validation. This is not general compatibility with every exported darktable style.

For an isolated development catalogue, set `OMALUX_PRESETS_DIR=/path/to/styles`. Files are read at startup; live reload and saving new styles are not implemented.

## Updating thumbnails

Run `bin/preset_preview chromatic` from the repository root. It renders the shared beach image with the bundle's actual style through the Omalux/darktable adapter and writes a 384×256 JPEG plus `preset.json` with `preview.source` and `preview.darktable_version`, preserving existing asset declarations. It requires the normal development dependencies and ImageMagick (`magick`). Use `bin/preset_preview --all` to regenerate every bundle in one persistent engine session. Each next style is applied after the preceding render completes. Regenerate explicitly after changing a style or LUT; startup only loads the stored thumbnail and does no extra preview rendering. A missing thumbnail displays “No preview” and does not prevent applying the style.

These files are shipped in the repository and loaded from disk, not embedded in the native executable. The preview is an illustration on the standard beach photograph; it does not preview the currently opened image. All 29 bundled v0 looks have been converted approximately; darktable still cannot read the old JSON directly. These are initial approximations; optical calibration remains on the project TODO list.

## Manifest format

`preset.dtstyle` and `thumbnail.jpg` are fixed filenames, so they are not repeated in JSON. The standard beach image is chosen by `bin/preset_preview`, not by each preset. No file hashes are stored.

```json
{
  "version": 1,
  "assets": [
    { "path": "look.cube", "role": "lut" }
  ],
  "preview": {
    "source": "assets/images/beach-volleyball.jpg",
    "darktable_version": "5.6.0"
  }
}
```

| Field | Meaning |
| --- | --- |
| `version` | Required integer version of our manifest format, currently `1`. |
| `assets` | Optional list of external dependencies; omit it or use `[]` when none are needed. |
| `assets[].path` | File path relative to this preset's folder. It must stay inside the bundle and exist. |
| `assets[].role` | How to make the file available to the engine; supported roles are listed below. |
| `assets[].target` | Optional destination filename for watermarks and ICC profiles. Defaults to the source basename. LUTs use their catalogue-relative path instead. |
| `preview` | Optional information about the stored thumbnail. It does not affect rendering. |
| `preview.source` | Repository-relative source image path, written automatically by the preview script. Provenance only, not an input setting or runtime dependency. |
| `preview.darktable_version` | Version of the installed engine that generated the thumbnail. **Not** a minimum version or style compatibility guarantee. |

The launcher reads dependency declarations before starting both engines. It passes setup errors to the UI, which marks the affected preset unavailable. Without a manifest, a standalone style still loads, but no external assets are prepared. Module version/layout compatibility is checked separately against the actual style.

`bin/preset_preview` updates only the thumbnail, `preview.source` and `preview.darktable_version`, preserving asset declarations and other metadata. All bundled references were converted once to this format; archived v0 JSON remains unchanged.

## Chromatic

`chromatic/preset.dtstyle` is an original Omalux look: vivid color, crisp contrast and a gentle exposure lift. Apply it with **PRESETS → Chromatic** in Omalux. It affects these two modules:

| Module | Settings |
| --- | --- |
| exposure (version 7) | manual; exposure +0.15 EV; black level correction 0; exposure-bias compensation off; highlight-preservation compensation on |
| contrast brightness saturation / colisa (version 1) | contrast 0.18; brightness 0.00; saturation 0.25 |

The style uses standard blending defaults, no masks and no custom module ordering. Its parameter blobs use darktable's little-endian hex encoding; target layouts were checked against 5.6.0/5.6.1. Exposure also stores its inactive deflicker defaults (percentile 50, target −4 EV). The adapter checks module version and parameter size before applying bundled items.

Application merges the style into the current image using darktable's style machinery, then reads the visible controls back. Reapplying replaces the matching settings instead of intentionally stacking new instances. Other edits remain. Existing complex imported module-instance combinations still require separate validation. The controls remain editable after applying the style. **R resets the three visible sliders only; it does not undo the style's exposure change.** Session edits are still temporary.

In split mode the same `.dtstyle` is imported and applied in the independent darktable process; later slider updates do not reapply it. This is a development comparison, not a guarantee of pixel-identical output across different workflows, display profiles or module instances.

`colisa` is deprecated upstream. Chromatic intentionally uses the currently connected prototype controls; a future scene-referred version should be a deliberate look migration, not a silent reinterpretation of these values.

Original Omalux style and documentation: GPL-3.0-or-later. darktable remains an independent upstream project.

## One-time v0 conversion

All 29 v0 presets were migrated once, preserving their folder IDs and display names. Chromatic remains as the thirtieth look. No migration scripts or per-preset migration records are required to use them.

Classic, Nightstreet, Ansel Adams, Forge, Lightning and Portrait are black-and-white looks. The lightly tinted v0 variants are deliberately desaturated in this initial conversion. The remaining styles retain their broad colour character; exact visual matching is deferred.

## Mapping

The module parameter layouts target darktable 5.6.0/5.6.1. The adapter checks versions and sizes before applying them. The scales below are migration heuristics, not alternative labels/units for darktable's UI controls.

| v0 settings | Current representation |
| --- | --- |
| Exposure EV | `exposure.exposure`, same numerical EV |
| Blacks | `exposure.black = -blacks × 0.0005`, limited to ±0.05 |
| Brightness | `colisa.brightness = brightness / 200`, limited to ±1 |
| Contrast and saturation | Corresponding `colisa` parameter divided by 100, limited to ±1; −100 saturation becomes monochrome |
| Shadows/highlights | `shadhi` strengths limited to ±100, bilateral filter, default radius/compression/color compensation |
| Whites | `shadhi.whitepoint = whites / 10`, limited to ±10 |
| Temperature/tint | Approximate RGB gains baked into the colour cube, not a camera white-balance replacement |
| Master and RGB curves | Piecewise-linear composition sampled into the cube; this differs from v0 curve interpolation and processing space |
| Color mixer | Weighted hue-band shifts in HSV baked into the cube |
| Vibrance | Saturation-dependent HSV adjustment baked into the cube, including negative values |
| Shadow/midtone/highlight grading | Luminance-weighted RGB tints and offsets, including balance/blending, baked into the cube |
| Original 3D colour table | Trilinear lookup at its stored strength, baked into the cube; v0 blue-fastest storage is reordered to `.cube` red-fastest storage |
| Fade | Small black lift and compression baked into the cube |
| Grain amount/size/midtone response | `grain` strength, scale = clamped ISO / 213.2, and midtones bias |
| Vignette | Negative vignette brightness at amount / 100; centered, automatic aspect, default fall-off, no desaturation |
| Sharpness | `sharpen` amount = amount / 100 × 2, radius 2, threshold 0.5 |
| Clarity, luminance/color denoising | Not transferred; use the original v0 preset values during calibration |

The generated 33³ `.cube` combines the pointwise color adjustments in encoded sRGB. It preserves the source table's contribution but is not the original scene-referred pipeline: interpolation, clipping, tone mapping, ordering, highlight handling and module algorithms differ. The batch thumbnails are rendered by darktable from the resulting styles; no v0 thumbnail is reused as a claim of the new output.

All source geometry and radial masks were checked and are neutral/empty. Bloom/halation are also not mapped (both are zero in the current source presets). Consult the original JSON when calibrating instead of assuming full parameter parity.

Each bundled style sets or disables the same seven modules (exposure, colisa, shadhi, lut3d, grain, vignette, sharpen), so switching looks clears the previous look's LUT/effects. Neutral resets this family only, not unrelated image edits.

Both development processes set darktable's LUT root to `presets/`. When importing a style into standalone darktable, also set its 3D LUT root to that directory and keep `look.cube` alongside the style. The cube is not embedded in `.dtstyle`.

## Declaring external dependencies

Keep the existing four-file layout; additional files may live in an `assets/` subdirectory inside each preset bundle. Declare them in `preset.json`, for example:

```json
{
"version": 1,
"assets": [
  {"path": "look.cube", "role": "lut"},
  {"path": "assets/signature.svg", "role": "watermark", "target": "omalux-signature.svg"},
  {"path": "assets/input.icc", "role": "icc-input", "target": "omalux-input.icc"}
]
}
```

| Role | Resolution |
| --- | --- |
| `lut` | File stays in its bundle. The style must reference its catalogue-relative path, such as `film/film-chrome/look.cube`, under the configured LUT root. |
| `watermark` | Copied to `watermarks/<target>` in both temporary darktable configurations. The style must store that filename. Fonts used by SVG text must already be available; font installation is not implemented. |
| `icc-input` | Copied to `color/in/<target>` in both configurations. |
| `icc-output` | Copied to `color/out/<target>` in both configurations. |

`target` defaults to the source basename. Missing files, escaping paths, unsupported roles and conflicting destinations with different contents mark the preset unavailable. Unknown roles can describe future dependencies but are deliberately rejected until implemented. External raster masks, overlay images and bundled fonts are not yet portable through this adapter; merely copying those files is insufficient (overlay images also depend on database state). Module compatibility checks still apply.

The launcher does not rewrite module parameter blobs or discover every undeclared dependency. Declared names must match the style's stored references. This is explicit bundle support, not an arbitrary darktable-style importer. The old `reference.json` format is no longer used; convert it to `preset.json` before loading an external bundle that used it.

## Saving and sharing a look from the UI

“Save current look…” writes a new bundle under `presets/my-presets/<id>/`. Its style includes the current editable modules and other active history modules that the snapshot adapter can serialize. Referenced LUT files are copied to its own `assets/` directory, and the style's paths are adjusted accordingly. The thumbnail comes from the current edited photo. `preview.source` records that photo (repository-relative when possible, otherwise absolute); `preview.darktable_version` records the loaded engine. These fields remain informational. Check the source path before sharing if it contains private directory names.

This differs from `bin/preset_preview`, which deliberately regenerates catalogue thumbnails using the standard beach photograph. Neither operation is a visual calibration against v0.

A card's menu exports the bundle under its catalogue-relative path in the chosen folder. Configure that folder as standalone darktable's LUT root. Export preserves the bundle contents; it does not discover undeclared or cross-bundle dependencies. Deletion requires confirmation and is available only for `my-presets/` entries, never for built-ins.

The snapshot writer currently rejects drawn/raster masks, additional module instances, and enabled watermark/overlay/raster-file modules. It does not serialize custom module order. Keep complex imported styles in their original explicit bundles until those cases have portable serialization.
