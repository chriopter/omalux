# Bundled looks

Omalux calls these looks **presets** in its UI. Each `.dtstyle` is a real darktable **style**, combining settings from multiple modules, and can also be imported in darktable's styles panel.

## Using and adding presets

Copy `.dtstyle` files directly into this folder and restart `bin/dev` or `bin/dev_split`. Each file needs a unique style name. The UI discovers files automatically, provides search, and lets each card expand independently. **Apply** sends all its module settings to darktable, including modules without an Omalux control. No QML button, slider definition or Lua mapping is needed for another style.

The expanded card reads module names, enabled state and parameter descriptions from the installed engine and values from the actual style blob. It includes stored inactive parameters too. Known controls use darktable's display units; other fields show stored engine values, not guessed GUI conversions. Arrays and structured values are shown in full; untyped data is shown as hex. Blending is currently summarized, not individually editable in the inspector.

Malformed files, duplicate names and unavailable or incompatible module layouts are displayed as unavailable with a reason. The adapter requires matching module versions and parameter sizes; it does not yet migrate old styles. Custom module ordering and styles containing drawn-mask records are rejected rather than partially applied. Complex duplicate module instances and raster-mask dependencies still require separate validation. This is not general compatibility with every exported darktable style.

For an isolated development catalogue, set `OMALUX_PRESETS_DIR=/path/to/styles`. Files are read at startup; live reload and saving new styles are not implemented.

## Chromatic

`chromatic.dtstyle` is an original Omalux look: vivid color, crisp contrast and a gentle exposure lift. Apply it with **PRESETS → Chromatic → Apply** in Omalux. It affects these two modules:

| Module | Settings |
| --- | --- |
| exposure (version 7) | manual; exposure +0.15 EV; black level correction 0; exposure-bias compensation off; highlight-preservation compensation on |
| contrast brightness saturation / colisa (version 1) | contrast 0.18; brightness 0.00; saturation 0.25 |

The style uses standard blending defaults, no masks and no custom module ordering. Its parameter blobs use darktable's little-endian hex encoding; target layouts were checked against 5.6.0/5.6.1. Exposure also stores its inactive deflicker defaults (percentile 50, target −4 EV). The adapter checks module version and parameter size before applying bundled items.

Application merges the style into the current image using darktable's style machinery, then reads the visible controls back. Reapplying replaces the matching settings instead of intentionally stacking new instances. Other edits remain. Existing complex imported module-instance combinations still require separate validation. The controls remain editable after applying the style. **R resets the three visible sliders only; it does not undo the style's exposure change.** Session edits are still temporary.

In split mode the same `.dtstyle` is imported and applied in the independent darktable process; later slider updates do not reapply it. This is a development comparison, not a guarantee of pixel-identical output across different workflows, display profiles or module instances.

`colisa` is deprecated upstream. Chromatic intentionally uses the currently connected prototype controls; a future scene-referred version should be a deliberate look migration, not a silent reinterpretation of these values.

Original Omalux style and documentation: GPL-3.0-or-later. darktable remains an independent upstream project.
