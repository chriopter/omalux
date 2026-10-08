# Editing controls and v0 mapping

Omalux displays darktable's English module/control labels and units. The v0 names below describe the migration only; they are not new slider captions. The algorithms are nearest equivalents, not a visual recreation of the archived engine.

| v0 control | darktable module → displayed control | Displayed unit / approach |
| --- | --- | --- |
| Exposure | exposure → exposure | EV |
| Contrast | contrast brightness saturation → contrast | unitless |
| Clarity | local contrast → detail | % |
| Highlights | shadows and highlights → highlights | unitless |
| Shadows | shadows and highlights → shadows | unitless |
| Whites | shadows and highlights → white point adjustment | unitless |
| Blacks | exposure → black level correction | unitless |
| Saturation | contrast brightness saturation → saturation | unitless |
| Vibrance | color balance rgb → global vibrance | % |
| Temperature | white balance → temperature | K; darktable's coefficient/temperature conversion |
| Tint | white balance → tint | unitless |
| Highlight hue | color balance rgb / highlights gain → hue | degrees |
| Highlight saturation | color balance rgb / highlights gain → chroma | % |
| Shadow hue | color balance rgb / shadows lift → hue | degrees |
| Shadow saturation | color balance rgb / shadows lift → chroma | % |
| Bloom | bloom → strength | %; size and threshold under details |
| Fade | color balance rgb / global offset → luminance | %; offset approximation, not a film fade algorithm |
| Halation | diffuse or sharpen → opacity, radius span | %, px; explicit experimental recipe button |
| Grain | grain → strength | % |
| Grain size | grain → coarseness | ISO |
| Grain midtones | grain → mid-tones bias | % |
| Vignette | vignetting → brightness | unitless; fall-off start/radius under details |
| Sharpening | sharpen → amount | unitless; radius and threshold under details |
| Luminance denoising | denoise (profiled) → Y0 | seven wavelet control points |
| Color denoising | denoise (profiled) → U0V0 | seven wavelet control points |
| LUT strength | LUT 3D → opacity | %; uniform blending enabled when edited |
| Rotation | rotate and perspective → rotation | degrees |

The existing brightness control remains `contrast brightness saturation → brightness`. Denoising also exposes the original `strength`, `mode` and `color mode`. The curve editor adjusts the seven band ordinates in Y0U0V0 wavelet modes. Its connecting lines show a control polygon, not darktable's interpolated response; horizontal positions currently assume the standard evenly spaced bands. Imported nonstandard abscissas are preserved by the engine, but are not represented by that graph.

The halation button initializes a red-channel diffusion recipe (one iteration, radius span 32, threshold 0.8, opacity 35%). It replaces the base diffuse module's parameters. This is an experimental approximation and is not calibrated to v0. Its sliders retain the original names `opacity` and `radius span`.

## Interaction and structure

- Each pane has its own QML file: Filters, Styles, Tone, Color, Detail, Effects, Geometry, History and Metadata. Shared sliders, curve editor, crop overlay, viewport, dialogs and shortcuts live in `omalux/ui/components/`.
- The sidebar first separates three areas in one switch: **Edit** (working on the photograph), **Styles** (applying a look) and **Details** (reading about it). Below it, a second strip shows only the panes of the chosen area, each an icon with its short name beside it: Edit has Filters (the curated controls), Tone, Color, Detail, Effects and Crop; Details has History and Info; Styles has a single pane, so its strip is hidden. Choosing an area returns to the pane last used in it. The active area and pane are raised tiles with an accent icon and name, hovered ones brighten on a subtle fill, and tooltips give the longer description. While searching, a dot marks the areas and panes with matches. Keys 1–9 select the panes in reading order — Filters, Tone, Color, Detail, Effects, Crop, Styles, History, Info — and Tab/Shift+Tab (] and [) step through the same order, so both follow what is shown (Crop & Rotate is 6). The image toolbar uses the same flat outlined buttons: the key hint in muted text before the label. Zoom out, the current level and zoom in are grouped in one outline; the level reads `[0] FIT` at fit and the zoom factor (e.g. `2.0×`) otherwise, and clicking it fits the photograph, so there is no separate fit button. History displays the native darktable processing stack, with current/enabled state, and refreshes after edits, styles and image changes. Click a step to restore it; later steps remain available until a new edit replaces the future branch.
- Text sizes are set only in `omalux/ui/components/EditorTheme.qml`: control labels and values 13 px, module headings medium-weight 12 px, other interface text 12 px. Enabled module headings are bright, disabled ones muted. Modules with a visible heading share one subtly lighter, borderless background across the heading and all associated sliders; headings have extra space above and a smaller gap to their controls.
- Each editing group corresponds to exactly one native darktable module. Controls in `shadows and highlights` and `color balance rgb` stay together. By explicit UI choice, brightness and saturation additionally have top-level shortcut rows sharing the colisa module state. The parent name toggles the module, and an accent dot marks its enabled state. Child labels select controls but never toggle a module. Editing a child enables its parent module, as in darktable; reset does not imply bypass.
- A collapsed single-primary module uses its primary row as the parent. Grain, bloom and vignetting show only their module name there. When expanded, the parent heading owns enablement and the same slider appears once below it with its original darktable label (`strength` or `brightness`). Additional parameters stay inside the same module background. Values stay right-aligned and disclosure occupies a separate right column that every row reserves, so values line up whether or not a row has a chevron. Collapsed rows and expanded headings use the same chevron (› closed, rotated ⌄ open), which brightens on hover. Right-click resets a parameter; child context menus do not expose a second module toggle.
- Eleven standard sliders remain available without expanding: brightness, contrast, saturation, exposure, shadows, temperature, vibrance, sharpen amount, grain strength, bloom strength and vignetting brightness. Brightness, contrast and saturation are separate top-level rows without a common heading. Editing any of them enables the same colisa module. All three rows carry a chevron that opens the one colisa module block; while it is open the block shows all three controls under the module heading and replaces the three single rows, so no control appears twice. Collapsing it restores the three rows. At the user’s request, global vibrance is displayed as vibrance; its underlying parameter and units are unchanged. Shadows and highlights exposes only shadows while collapsed; highlights and white point adjustment appear on expansion. Local contrast is a separate native module and lives under Advanced alongside denoise, diffuse or sharpen and LUT 3D. Expansion is remembered per module and never changes processing values. Direct shortcuts reveal hidden secondary parameters. All parameter names and units come from darktable.
- Sidebar panes scroll vertically and stop at their edges. Wheel and touchpad events move the content immediately, without a separate scroll animation: Touchpad pixel deltas use an explicit 4× speed multiplier; angle-only events move 120 logical pixels per notch, including when Wayland classifies the seat as a touchpad. Like Omawrite 0.5.0, event shape determines the path, not the device label. Display scaling is used only to align positions to physical pixels, not to multiply speed. Both paths clamp to the content edges. Sliders and mode selectors do not consume the wheel. Hovering a slider does not change the selected parameter; keyboard navigation follows the displayed order (see below). History refreshes no longer explicitly reset the scroll position.
- Colored tracks indicate luminance, hue, saturation, temperature or tint. They are visual hints, not a simulation of the actual output.
- The slider knob (an 11 px ring on compact rows, 9 px square otherwise) and the 3 px track are placed with the same integer rounding, so the ring is centred on the track at scale 1 and 2. Generated rows and coloured tracks use the same `ControlSlider`.
- Slider tracks use darktable's soft range where specified. Double-click the numeric value to enter values within the full hard range. The context-menu reset and `R` restore registry defaults, not the photo's original history.
- Crop uses the native crop module; dragging the frame changes a draft until Apply/Enter. Escape restores prior crop enablement. Rotation uses darktable's signed display conversion.
- Zoom, pan, pinch and fullscreen operate on the interactive preview. Zoom does not yet request a full-resolution detail render. JPEG/PNG export uses the actual full-resolution darktable export pipe.

See the main README for keyboard bindings and development commands.

## Keyboard navigation

One item, `KeyboardNavigator` (`omalux/ui/components/`), holds keyboard focus for the whole editor and hands every key to the single binding table in `EditorShortcuts.qml`; the `?` reference is generated from the same table, so it cannot drift. Rules:

- **Order.** The selectable items are the `NavTarget`s of the visible sidebar pane, ordered by where they are shown (top to bottom, then left to right). ↑/↓ cross module and group boundaries, stop at the first and last item (no wrap), and skip disabled items. Collapsed parameters, hidden panes and the other filter view are never reached: a hidden pane never receives keys. Only a direct shortcut (`G`, `S`, `M`) opens a collapsed module to select its parameter. Panes without items (Info) scroll with ↑/↓.
- **Selection.** Each pane remembers its selection; a new pane starts at the item it marks as active (the active control, the current history step, the last applied style). The selection is marked in the accent colour and is scrolled into view at once, by the smallest movement with a small margin, also when a key adjusts it after the wheel moved it away.
- **Blending and instances.** The rows of the blend section are ordinary stops (sliders, choices, switches, channel chips; the parametric ranges are graphs). The multi-instance button in a heading is not an ↑/↓ stop; the last stop of an open module, **multiple instances**, opens its menu with Enter.
- **Generated modules** (Tone, Color, Detail, Effects, the look modules under Styles and the modules under Crop & Rotate) take part like the curated ones: group headings, module headings (Enter or ←/→ open the details, `E` switches the module, `Shift+R` resets it), every slider (one step of darktable's displayed precision, written in raw units through the module's parameter queue), choice and switch rows (`R` restores the row's default), module page tabs and channel choosers (←/→), colour swatches (Enter opens the picker), list rows (Enter opens the list; typing searches, ↑/↓ and Enter choose, Esc closes), text fields such as the watermark text (Enter edits), the colour checker patches (←/→ select a patch, `R` resets it to its source), "more", and curves and graphs. Notices and section captions are not stops.
- **Curves and graphs.** Enter lends the keys to the widget: its own arrows edit the points (as documented in `CurveEditor`/`GraphView`); `Esc` (a second one when a point is selected) or any key the widget does not use gives them back.
- **Keys on the selection.** ←/→ change a slider by darktable's step (`Shift` ×10, `Ctrl`/`Alt` ×0.1, clamped to the hard range, whole steps for integer parameters), choose the previous/next option, toggle switches and close/open modules and style groups. `Enter`/`Space` activate (module, style, history step, button, switch, search field). `R` resets the parameter, `Shift+R` the whole module, `E` switches the module; both go through the module heading when one is shown. A style selected from the keyboard previews like hovering.
- **Focus.** Clicking a control selects it for the keyboard, then focus returns to the navigator; the same happens after menus, the value entry and dialogs close. Text fields keep all keys while typing (no shortcut fires); `Esc` leaves the field, `Enter` or `↓` leave it and select the item below. Popups and dialogs own their keys while open.
- **Search.** `/` or `Ctrl+F` focus the global search under the tab strip; it is not one of the ↑/↓ stops. `Esc` clears it and returns the keys; `Enter` or `↓` keep the term and select the pane's first item. The Styles pane keeps its own search as a listed item.
- **Panes.** `1` … `9` follow the tab strip as shown, like Tab/Shift+Tab, so a number always names the pane at that place in the order above.
- **Precedence of `Esc` and `Enter`.** A text field first, then an open dialog or menu, then the crop frame (apply/cancel), then photograph fullscreen.
- **Bottom bar.** Shows the keys of the current selection, e.g. `[↑/↓] SELECT   [←/→] exposure   [R] RESET VALUE`.

### Making a control or pane keyboard-reachable

Declare a `NavTarget` inside the control's root item; no registration and no reference to the navigator is needed (the navigator walks the visible sidebar):

```qml
NavTarget {
    id: nav
    navId: root.control.id      // stable, unique within the pane
    label: root.control.label   // darktable's label, shown in the hint bar
    kind: "slider"              // slider · choice · switch · module · group · style · step · button · search · graph
    group: "shadhi"             // module key: Page Up/Down, Shift+R and E act on it
    enabled: root.editable      // false: skipped and inert
    active: root.selected       // fallback selection of the pane
    onAdjust: steps => …        // ±1, ±10, ±0.1
    onActivate: …               // Enter / Space
    onReset: …                  // R
    onResetGroup: …             // Shift+R (module headings)
    onToggleGroup: …            // E (module headings)
    onSelected: …               // became the selection (e.g. update the active control)
    onRevealRequested: …        // a direct shortcut targets it while hidden: expand
}
```

Bind the highlight to `nav.current`, call `nav.claim()` on mouse press, and set `input: someTextField` with kind `search` for a search field (`listed: false` keeps it out of the ↑/↓ order). A widget with its own key handling sets `focusItem` (kind `graph`): Enter gives it the keys, Escape takes them back. `GeneratedRows` sets `navTarget.navId` (`operation/instance/path`) and `navTarget.group` (`navGroup`) on every row it builds; `ModuleTabs`, `ChannelChooser`, `ColorSwatch`, `CurveEditor` and `GraphView` carry their own `navTarget`. `ControlSlider`, `ControlChoice` and `ControlSwitch` already contain one (`navTarget`, with `navTarget.group`/`navTarget.navId` settable by the parent, and `activated`, `moduleResetRequested`, `revealRequested` and `resetRequested` signals). A module heading is a `NavTarget` of kind `module` with `groupActions: true`. Hints come from `adjustLabel`, `activateLabel`, `resettable` and `groupActions`, or set `hints` directly. New keys go into the table in `EditorShortcuts.qml` (with section and label), never into separate `Shortcut` items.

Tests: `omalux/tests/components/tst_keyboard.qml` (qmltestrunner, every pane against a recorded registry) and `omalux/tests/keyboard.json` (real application, run by `omalux/tests/run.py`). The native registry is `omalux/native/engine/controls.h`; source labels and formatting were checked against the pinned/installed `src/iop/` modules, including Bauhaus percent conversion and GUI action paths. Display units are converted to native values before both native edits and GTK actions. Curves, blend settings and the diffusion recipe use temporary single-module styles for split synchronization.

## Validation and remaining limits

The optional `OMALUX_SMOKE_SCRIPT` development driver runs deterministic actions against the real engine. It exercised scalar edits, white balance, grain, wavelet curves, the diffusion recipe, style application, LUT opacity, own style save/reapply and full-resolution JPEG/PNG export. A square crop produced a 1024 × 1024 export from the 1536 × 1024 source; bundle export retained the thumbnail and declared LUT, and own-style deletion was checked. Split mode acknowledged control updates and a source-image switch followed by exposure editing; this does not establish byte-identical output or performance parity.

The curated controls edit the base module instance. The generic module API below reaches every described parameter of any instance; the multi-instance menu creates, duplicates, moves, renames and deletes instances, and every module with darktable's blending has its blend section (see [Blending and module instances](#blending-and-module-instances)). Drawn shapes, liquify warps and the other on-image tools are drawn on the photo (see [Drawing on the image](#drawing-on-the-image)). Own-style snapshots reject drawn and raster masks, extra instances and external image dependencies rather than silently losing them. The original colisa controls are deprecated upstream; they remain for existing styles. See the architecture notes for cache and GPU-reporting limitations.

Scrolling reference: [Omawrite 0.5.0, Main.qml](https://github.com/omacom/omawrite/blob/v0.5.0/src/Main.qml), event handling and `snapToPixel`. Omalux adopts its event classification and pixel alignment; its angle-only movement remains immediate rather than using Omawrite’s animated wheel curve.

- Denoise is available only in the expandable Advanced section. The collapsed vignetting brightness slider focuses on darkening (−1 to 0); expanding its details restores the full darktable range. Existing positive brightness values remain visible and are never changed by collapsing. Names, units and the default −0.5 are unchanged.

### Style hover

Hover over a style's thumbnail/name to preview it on the open photograph. Leaving the card or the Styles pane restores the current edited image immediately. Hover does not change controls or history, including any future history after a backward jump. Click to apply the style through the normal history path. Unsupported styles remain unavailable. The preview starts after a short hover delay and uses a separate temporary engine context.

### Native regression checks

Run `python3 omalux/tests/run.py --split` from the repository root to exercise the actual engine and comparison path after integration changes. The runner builds through `development/start`, uses isolated settings and a temporary copy of the style bundles, and checks slider gestures, hover/history, special controls, bundle assets, crop, export and image reopening. `magick` and a working desktop/OpenCL runtime are required. Omit `--split` to run only Omalux.

## What was applied for this camera

The Styles pane starts with a small collapsible `camera` block: the camera maker/model darktable detected (or "not identified") and the Omalux camera presets (`catalog/camera/*.dtpreset`) that match this image the way darktable's auto-apply matches them — camera, lens, ISO, exposure, aperture, focal length and the raw/LDR/HDR, matrix and monochrome format flags (`camera_defaults.c`, after `develop.c` `_dev_auto_apply_presets`). Each entry shows darktable's module name and the preset's description; it is greyed and marked "not applied" when that module is off in the loaded pipeline. When no preset matches, one line says so. The same entries appear as a "Camera presets" group in the History box below. The block is read-only.

The History pane starts with a box listing what darktable set up before any style: the input profile and white balance (Colour), the lens correction (Lens), and exposure, tone mapping, highlight reconstruction, denoising and sharpening (Base tone). Values are read from the loaded modules after the image's history has been applied, so they show the actual state; entries in grey are not applied. Camera presets from `catalog/camera/` are imported into the session and their profiles copied into darktable's `color/in`, so this box also reflects them.

## Display data from darktable

darktable's introspection gives a parameter's name, type, range and default, but not how
the value reads on screen. That part lives in the calls that build the module's own
widgets: the unit, the factor between stored and shown value, the digits worth showing,
and the range a slider covers before it has to be forced wider.

`development/tools/darktable/extract_display.py` collects those calls from the module
sources into `omalux/design/display.json`, keyed `operation/field` (the operation is the
plugin name from `src/iop/CMakeLists.txt`, e.g. `colorreconstruct` for
`colorreconstruction.c`). The raw module list builds its sliders from it, so a parameter
reads correctly without an entry in the registry. Re-run the tool after updating the
darktable submodule or the installed release:

```
python3 development/tools/darktable/extract_display.py
```

The entries are what darktable actually shows, not the literal arguments. The tool starts
from what `dt_bauhaus_slider_from_params` sets up (`develop/imageop_gui.c`: two or more
digits derived from the hard range for floats, none for integers, factor 1) and replays
the `dt_bauhaus_slider_set_*` calls in source order with Bauhaus' rules
(`bauhaus/bauhaus.c`). The one that matters most: `set_format` with a `%` unit on a slider
whose hard maximum is at most 10 multiplies by 100 unless a factor is already set, and
removes two digits. Digits set before the unit are therefore reduced, later ones are not.
Hard ranges come from the installed plugins' compiled introspection (`--library`, default
`/usr/lib/darktable/libdarktable.so`), so the tool must run against the release the adapter
is built for. Soft limits are clamped into the hard range as darktable does; numeric
`#define`s of the module source (such as `GRAIN_SCALE_FACTOR`) are resolved.

Each entry has `digits` and, where darktable sets them, `format`, `factor` (≠ 1), `offset`
(≠ 0; shown value = native × factor + offset), `soft_minimum`/`soft_maximum` (native),
and `hard_minimum`/`hard_maximum` when the widget narrows or widens the parameter's own
range. A field with several sliders (bilat shows `sigma_r` as "range" or "highlights"
depending on the mode) lists the later ones under `alternatives`. 382 sliders in 68
modules are covered; the rest draw their own widgets and still need a dedicated adapter.

## The curated decisions as data

`development/tools/darktable/export_controls.py` writes the part of the registry that is
genuinely our own — which parameters are worth showing, under which name, in which group
and order, with which colour track — to `omalux/design/controls.json`, and reports every
row that says something other than darktable does: a scale or offset that does not invert
darktable's factor and offset, a different unit, or different digits once the conversion
agrees.

With the effective display data, the `local contrast → detail` row turned out to be wrong:
darktable shows the stored −1…4 as 0…500 % (factor 100 plus an offset of 100, default
125 %), while the row showed −100…400 with default 25. The row now matches darktable
(scale 0.01, offset −1). The two remaining reports, `highlights_chroma` and
`shadows_chroma`, only note that the row leaves the soft range to darktable's data, which
`Editor::controls` then uses.

## Generic module edits

Every processing module, including those without a curated control, can be read and edited
through darktable's introspection. This is the native contract the generated Filters pane
builds on.

### Catalog

`backend.moduleCatalog` is a JSON string: an array with one entry per module instance in
pipeline order (`operation`, `label`, `instance`, `position`, `enabled`, `default_enabled`,
`hidden`, `blending`, `described`, `params_version`, `parameters`). `parameters` lists the
fields of the params struct; nested structs are flattened, arrays are one row. Every row
has `name`, `field`, `path`, `label`, `type`, `size`, `curated` and, for scalars, `value`
with `minimum`/`maximum`/`default` (and `values` for enums).

`path` is the full address relative to the params, e.g. `exposure`, `rgb_bands`, or for a
nested struct member `outer.inner`. An array row's `value` holds the whole array: nested
arrays for several dimensions, objects keyed by member name for structs, a string for
`char` arrays. Array rows also carry `count`, `element` (as before), `dimensions` and
`item`, which describes one element: its scalar limits, or `members` with each struct
member's `field`, `type` and limits. For example (abridged):

```json
{"operation": "exposure", "instance": 0, "enabled": true, "parameters": [
  {"path": "exposure", "field": "exposure", "type": "float", "value": 0.7,
   "minimum": -18, "maximum": 18, "default": 0}]}
{"operation": "tonecurve", "instance": 0, "parameters": [
  {"path": "tonecurve", "type": "array", "count": 3, "element": "array", "dimensions": [3, 20],
   "value": [[{"x": 0, "y": 0}, {"x": 0.5, "y": 0.6}, {"x": 1, "y": 1}, …], …],
   "item": {"type": "struct", "members": [{"field": "x", "type": "float", …}, …]}},
  {"path": "tonecurve_nodes", "type": "array", "element": "int", "dimensions": [3], "value": [3, 3, 3]}]}
```

`moduleUpdated(operation, instance, moduleJson)` carries one such entry after a generic
edit, a reset or a curated control change of that module. The whole catalog is re-announced
(`modulesChanged`) when the module stack may have changed (image, style, history step,
halation recipe) and once a gesture ends; during a drag only `moduleUpdated` fires.
Describing all modules takes 7–10 ms for the beach JPEG (93 modules, about 300 KB), one
module 0.07–0.15 ms, so the worker keeps a per-module cache and re-describes only the
touched modules.

### Editing

- `setParameter(operation, instance, path, value)` sets one scalar by path, e.g.
  `tonecurve[0][1].x`, `curve_num_nodes[0]`, `matrix[3]` or `exposure`. `@enabled` switches
  the module itself.
- `setParameters(operation, instance, {path: value, …})` sets several values of one module
  and records a single history item. All paths are resolved before anything is written, so
  an unknown path leaves the module untouched.
- A value may be a string for a `char` array (file names, profile files, lens models, watermark
  text); it must fit the array including its terminator. Paths starting with `@` (other than
  `@enabled`) are darktable's displayed conversions and runtime choices, converted by
  `native/engine/module_values.c` and `module_choices.c` into parameters before the history
  item is recorded (see "Displayed values and runtime lists" below). A rejected conversion
  leaves the parameters unchanged.
- `requestChoices(operation, instance, list, query)` asks for a list darktable fills at runtime;
  `choicesReady(operation, instance, list, query, json)` answers with
  `{"items": [{"label", "detail", "section", "set"}], "current", "more", "error"}` without
  rendering. Choosing an item sends its `set` object through `setParameters`.
- `resetModule(operation, instance)` is darktable's module reset without its GUI
  (`_gui_reset_callback`, `develop/imageop.c`): image-dependent defaults are reloaded,
  default parameters and blending restored, and the module is switched on, in one history
  item. Modules that are always off cannot be reset.

Values are clamped to the introspection range (float, double, int, uint, short, ushort,
char as an integer; enums as written, booleans above 0.5). As with a darktable slider,
editing a parameter switches its module on unless the edit sets `@enabled` itself or the
module has no enable button. Module-specific `gui_changed` consistency logic does not run
headless. Failures (unknown module or path, unsupported type) appear as the status line.

Edits queue in order on the worker and do not start a new presentation epoch, so frames
keep arriving during a drag. Consecutive edits of the same module merge while the worker is
busy, and darktable merges consecutive history items of the same module, so a drag leaves
one history item, as a curated slider does. Pass `setInteractive(true/false)` around a
drag to get the reduced previews.

### Split mode

Every module touched by a generic edit, reset or `@enabled` change is sent to the
comparison window as a temporary single-module style (`omalux-sync-<operation>-<revision>`),
the same recipe mechanism the curves and the diffusion recipe use. A newer snapshot of a
module replaces older ones of the same epoch in the mailbox, so the comparison applies only
the latest state. The curated controls keep their GTK action path. A snapshot holds every
instance of the operation with its `multi_priority` and name, and the blend parameters
including a raster mask taken from another module, so blend edits and edits of further
instances are mirrored; darktable matches the instances by name, then unused, default and
priority (`dt_history_merge_module_into_history`). Deleting or moving an instance is not
mirrored (a style never removes or reorders modules). darktable styles cannot carry drawn forms: a module
that uses a mask group (retouch, spot removal, a drawn blend mask), and a history step while
shapes exist, reach the comparison as an XMP sidecar of the whole history instead
(`omalux-sidecar-<revision>.xmp`, a `sidecar <epoch> <name>` mailbox line), which `comparison.lua`
applies with `image:apply_sidecar`; like a history snapshot it starts a new epoch.
`python3 omalux/tests/run.py` records the mailbox of `omalux/tests/module-parameters.json`
(`OMALUX_RECORD_MAILBOX`, no comparison window) and checks it carries one current snapshot
each for exposure, tonecurve and rgbcurve. `omalux/tests/module-values.json` does the same for
the displayed conversions and runtime lists (color balance, color calibration, color
harmonizer, split-toning, color look up table, input profile, lens, LUT 3D); for `omalux/tests/blending.json` it checks that
the exposure snapshot carries the second instance, and for `omalux/tests/canvas-engine.json` that
the current sidecar holds the drawn shapes. The real comparison window was not run with
instances or sidecars.

## Generated layout of every module

The Filters pane can list every darktable processing module from data instead of hand-written
rows. Three inputs feed it:

- `omalux/design/ui-map/{tone,color,correct,effects}.json` — the verified inventory: every
  control of the 95 modules (plus the blend section) with darktable's label, unit, factor, digits, ranges, default,
  enum values, widget, tab/section, condition, action path and source line.
- `omalux/design/layout-decisions.json` — our own choices only: group membership and order,
  hidden modules, which modules belong to the curated block, rows left out or replaced by a
  notice, primary and tier overrides, coloured tracks and short notes.
- `omalux/native/engine/controls.h` — rows registered there stay in the curated block and are
  dropped from the generated modules.

`development/tools/darktable/build_layout.py` (standard library only) combines them into
`omalux/design/layout.json`: groups `base`, `tone`, `color`, `correct`, `effect` (darktable's
own module group names), `technical` and `deprecated`, each with modules and their rows in
darktable's GUI order. A module carries `curated`, `deprecated`, `show` (`if-used` for
deprecated modules), `tabs`, `primary` (0–2 rows shown while collapsed) and `notes`. A row
carries field, params path, label, tab, section, widget, unit, factor, offset, digits, hard
and soft range, default, enum values, `visible_when`, tier, colour hint, custom-widget wiring
and the GTK action path. Labels, units and ranges are the inventory's; rows darktable leaves
unlabelled get an empty label.

- `path` is the params member the engine writes (`name`, `name[i]`, `name.member`).
  `name[@sel]` takes its index from the GUI-only row `@sel` (`@tab` is the active tab). A path
  starting with `@` is a displayed conversion that needs an adapter, as in `controls.h`.
  `null` marks GUI-only state and actions (buttons, pickers, notices).
- `visible_when` is `{field, in}` or `{all: [...]}`. Conditions on the sensor, the image file
  or darktable preferences cannot be expressed this way and stay `null`, with the condition
  in the module's notes. A condition may name a curated parameter by its params member.
- On-image tools (retouch and spot removal shapes, liquify warps, the graduated density line,
  rotate and perspective's structure, the vignetting ellipse) become one `canvas` row from the
  decisions' `canvas` block (`custom.tool`, `custom.hint`); the rows it replaces are left out.
  Colour checker calibration and retouch's wavelet bar stay a `notice` row; plain sliders of
  the same module are kept. Rows darktable fills from a runtime list or a file dialog (profiles,
  lensfun camera and lens, focal length, aperture, distance, noise profiles, LUT, watermark,
  raster mask and overlay files, white balance settings; `CHOICES` in `build_layout.py`)
  become `choice` rows whose `custom` names the engine list (`list`), the `@` path a chosen
  file goes to (`browse`) and the dialog's name filters. color look up table's patch grid is a
  `patches` row; its target sliders use `@target_*[@absolute_target][@patch]`.
- crop, rotate and perspective and orientation are curated by the Geometry pane; only rows
  that pane lacks are listed. Hidden pipeline modules (finalscale, gamma, mask_manager,
  rotatepixels, overexposed, rawoverexposed, the uncompiled useless) are left out.

### How the panes use it

The Filters pane keeps the curated block unchanged. Every other module is shown from the layout:

- **Tone** lists `base` and `tone`, **Color** `color`, **Detail** `correct` plus `technical` as a
  quiet group that starts collapsed, **Effects** `effect`. Look-like modules (color look up table,
  color mapping, split-toning) sit under the style cards in **Styles**; rotate and perspective
  (the rows the crop pane lacks), orientation and lens correction sit under **Crop & Rotate**.
  Deprecated modules appear in the pane of the group they used to belong to, under
  `deprecated`, and only while the current image has them switched on. Curated modules are not
  listed again: their remaining rows (for example exposure's mode, color balance rgb's 4 ways
  and masks pages, the denoise all/R/G/B curve) appear inside the curated block when it is
  expanded, behind a quiet `more`.
- Group headings use darktable's group names, collapse with a click and show the number of
  modules. Each module uses the curated block's visual language: heading with darktable's
  module name and an accent dot (click toggles the module, right-click offers enable/disable and
  reset, hovering shows darktable's purpose), primary rows visible while collapsed (labelled
  `section · label` where the section caption is hidden), the chevron for detail rows and a
  quiet `more` for advanced rows. Modules with darktable notebook pages show `ModuleTabs`.
  Open groups, expanded modules and `more` are remembered per pane. Additional instances of a
  module are listed after the base instance with their number.
- Rows: sliders show `raw × factor + offset` with darktable's unit and digits, the soft range on
  the track and the hard range for typed values; choices and switches use `ControlChoice` and
  `ControlSwitch` in the slider label font; curves (`CurveEditor`, with `ChannelChooser` where
  darktable has channels in the current mode) send all node coordinates and the node count as
  one `setParameters` batch; x/y band arrays (contrast equalizer, denoise all/R/G/B, raw
  denoise, low light) and the tone equalizer bands use `GraphView`; levels show darktable's
  black/gray/white handles as three sliders (per channel when the channels are independent);
  colour parameters use `ColorSwatch`. GUI-only selectors (`@destination`, `@patch`, `@controls`,
  the active page or channel for `@tab`) are local view state and pick which array element the
  rows edit. Displayed conversions (`@` paths) are sliders and swatches like any other row;
  runtime lists and file choices are `ChoiceRow`s; the colour checker patches are a
  `PatchGrid` (see "Displayed values and runtime lists" below).
- Pickers and module buttons are rows of `ModuleToolButtons` and pickers on sliders (see
  "Pickers and module buttons" below). Shown as a muted notice instead of a control, merged per
  section: the pickers and buttons not ported yet (listed there), sidebar-drawn
  features (retouch's wavelet bar and preview levels, monochrome and colour correction grids,
  relight center, zone system), darktable's own graphs of filmic rgb/AgX/filmic, and the remaining
  `@` conversions without an adapter (rotate and perspective/clipping `@flip` and `@aspect` of the
  deprecated crop module). Right-click on a row resets it to darktable's default; module reset uses
  `backend.resetModule`.
- `ModuleCatalog.qml` parses the catalog once per change into states keyed `operation/instance`
  and keeps the previous object for modules whose entry did not change; `moduleUpdated` updates
  one module (the only update during a drag). `ParameterQueue.qml` sends one request at a time
  (the engine worker keeps one pending action) and merges edits made meanwhile, so dragging a
  generated slider produces draft frames like a curated one.
- A search field under the tab strip searches every pane by darktable module name, operation
  and row label. Matching groups and modules open, other modules are hidden, and a dot marks
  the panes with matches; if the current pane has none, the sidebar switches to the first pane
  that has. Escape or × clears it. Keyboard navigation (arrows, `R`) still covers only the
  curated controls; generated rows report selection through `controlSelected` with ids
  `operation/instance/path`.

### Displayed values and runtime lists

darktable stores some parameters in a form its sliders do not show, and fills some
comboboxes only at runtime. The engine reports both with each catalog entry: `derived`
(`{"@path": displayed value}`, `native/engine/module_values.c`) and `labels` (`{field: text}`
of a runtime-list row, `module_choices.c`). `ModuleCatalog` resolves `@` paths like
parameters; a slider whose `@` path the engine does not report for this image is hidden, as
darktable hides the widget. Each conversion is darktable 5.6.1's slider callback without GTK:

| Module | Rows | darktable's conversion |
| --- | --- | --- |
| color calibration | illuminant hue °, chroma (custom illuminant) | CIE x, y ↔ LCh at L 100 (`gui_changed`, `_illum_xy_callback`); writing also sets the temperature (`xy_to_CCT`, below 3000 K `CCT_reverse_lookup`) |
| color balance | hue °, saturation % of shadows, mid-tones, highlights | `rgb2hsl`/`hsl2rgb` of the RGB factors ÷ 2 at lightness 0.5 (`set_HSL_sliders`, `HSL_CALLBACK`) |
| color harmonizer | anchor hue, custom node hues (RYB °) | darktable UCS hue ↔ painter's wheel through the module's 720-entry lookup tables |
| split-toning | the two colour swatches | `hsl2rgb(hue, saturation, 0.5)`; a picked colour stores `rgb2hsl`'s hue and saturation |
| color look up table | lightness, green-magenta, blue-yellow, saturation of the selected patch; patch grid | relative to the source patch or absolute after "target color" (`@target_L[mode][patch]`); saturation scales a, b; a, b clamped to ±128 |
| white balance | settings, finetune (mired) | the five standard entries and the camera's wb presets (`_generate_preset_combo`); choosing applies their coefficients (`_preset_tune_callback`); finetune only for presets with tuning variants, in their range |

Runtime lists (`ChoiceRow`, list name in `custom.list` of the layout row): input and working
profile (color input profile: the image's own profiles, then darktable's), export profile
(output color profile), noise profile (denoise: the automatic "found/interpolated ISO" entry and
the camera's measured profiles), camera and lens (lensfun, searchable; choosing a lens stores
the model, a prime lens's focal length and darktable's automatic scale), focal length,
aperture and distance (darktable's steps for the lens; a typed number is offered too), LUT
file (every `.cube`, `.3dl` and `.png` below the LUT root folder, searchable), watermark
marker (SVG/PNG in darktable's and the user's `watermarks` folders), raster mask file (PFM/PNG
in the chosen folder) and white balance settings. File dialogs: LUT file (only below the LUT
root folder, stored relative to it, as darktable), raster mask file (only below the raster mask
root folder, default the home folder), overlay image (imported into the session library like an
opened photo, then recorded as darktable records a dropped image). Watermark text and font are
text fields. Each choice is one `setParameters` batch: one history item, a single-module style
in split mode.

Not covered: gmic-compressed LUTs (`.gmz`, darktable's reader is bound to its GUI) and color
calibration's colour checker calibration. The "from image area" white balance, the lens/camera
"find" buttons and color calibration's mapping picker are described under "Pickers and module
buttons". Own
styles: lens names, LUT paths (packaged as before), profiles and noise profiles are plain
parameters; watermark, overlay and raster mask modules that are switched on are rejected, as
before, because their files are not packaged. colorin and colorout have no enable button and
are not part of own styles.

The per-module blend section is written separately in the same row format to
`omalux/design/layout-blending.json` (see [Blending and module instances](#blending-and-module-instances)).
A curated module also carries `instance_rows`, `instance_primary` and `instance_tabs`: every
row, as an uncurated module would have them, for its further instances, which are not edited
through `controls.h`. Regenerate both after changing the inventory, the decisions or
`controls.h`:

```
python3 development/tools/darktable/build_layout.py      # add -v to list unconverted conditions
```

The generator stops with an error if a decision names a missing module or row, a module is
placed twice or not at all, a primary is not a row, a slider lacks a numeric range or default,
a combobox lacks values or its default, or a field repeats within a module. It prints modules
and rows per group, widget and custom-widget counts and the modules with notices.

## Pickers and module buttons

darktable's colour pickers and the buttons that compute parameters run in the engine
(`omalux/native/engine/module_tools.c`, ports in `tools_*.c`) through
`backend.runModuleTool(operation, instance, {tool, box, gui})`; the answer arrives as
`moduleToolResult` (picked statistics, values for GUI-only rows, histograms, graph markers).

**Picking.** darktable samples the module input on the preview pipe while the module has focus
(`pixelpipe_hb.c` `_pixelpipe_picker`, `color_picker.c` `dt_color_picker_box`/`_helper`).
Headless, the engine renders the module input with a private export-type pipe of the editor's
develop state (every node from the module on disabled; the output is the module input at the
1400 × 1000 fit size, or 1:1 around the box for a module before demosaic of a raw), converts it
to the module's input colour space — or to the blend colour space while a mask is active, as
darktable does — transforms the box drawn on the displayed image back through the later
distortions, and calls darktable's own `dt_color_picker_helper` in the picker's colour space
(the module's default one, or the one the module sets, e.g. LCh for color zones, none for white
balance), with denoising where darktable asks for it. Pickers with darktable's `DT_COLOR_PICKER_IO`
also pick the module output. The module's `color_picker_apply` logic is ported per module and
writes the parameters; every module a tool changed (or switched on — activating a picker switches
its module on, as in darktable) gets one history item and is mirrored to split mode as a
single-module style. A tool that fails leaves the parameters untouched.

**On the photo.** `ModuleTools.qml` (sidebar composition, handed to rows as `catalogModel.tools`)
holds the active picker; `PickerOverlay` in the viewport shows its box. Area pickers start with
darktable's default area (2–98 %), point pickers at the centre; dragging outside the box draws a
new area, inside moves it, at a corner resizes it; point-or-area pickers pick a point on a click
and an area on a drag. Every release applies the picker again; each picker remembers its box.
A picker stays active until its button is pressed again, the pane changes, cropping starts, or a
parameter of its module is edited (darktable's `dt_iop_gui_changed`), except darktable's
"keep-active" pickers (show color of rgb curve, color zones and the blend section). The
colour balance optimisers switch themselves off after one run, as in darktable. Ctrl/Shift on
release select darktable's positive/negative "create curve" and the output slider of "set range".

| Module | Pickers and buttons (darktable source) |
| --- | --- |
| white balance | from image area (temperature.c:1948) |
| exposure | area exposure mapping: picker with area mode (correction/measure) and target lightness, input L shown (exposure.c:865) |
| rgb levels | black/gray/white point pickers on the shown channel, auto, auto region; input histogram behind the handles (rgblevels.c:774, 1180) |
| levels (deprecated) | auto from the input histogram; histogram behind the handles (levels.c:186, 978) |
| filmic rgb, filmic (deprecated) | auto tune levels and the pickers of middle gray, white and black relative exposure (filmicrgb.c:2582–2715, filmic.c:606–757) |
| AgX | auto tune levels, black/white relative exposure and pivot pickers, read exposure, reset primaries (menu: blender-like, smooth, unmodified), set from above (agx.c:1103–1220, 2087–2140, 2657) |
| unbreak input profile | auto tune levels and the middle gray, black relative exposure and dynamic range pickers (profile_gamma.c:322–416) |
| negadoctor | film material, shadows and illuminant pickers, and the D max, scan exposure bias, paper black and print exposure pickers (negadoctor.c:633–810) |
| basic adjustments (deprecated) | auto, select region, middle gray picker (basicadj.c:482, 676–1333) |
| color balance | factor and hue pickers of each wheel, contrast fulcrum, optimize luma, neutralize colors; the picked patches stay in the rows' GUI state (colorbalance.c:945–1342) |
| tone curve, rgb curve, color zones | pick/show color (band, mean and "input → output" on the graph), create curve (rgbcurve.c:507–581, colorzones.c:2396) |
| color calibration | the CAT picker with the area mapping (correction/measure, target lightness/hue/chroma, take channel mixing into account), input LCh shown (channelmixerrgb.c:4161–4421) |
| color harmonizer | auto detect (camera button), anchor and custom node hue pickers (colorharmonizer.c:1170–1378) |
| orientation | rotate 90° CCW/CW, flip horizontally/vertically, with the crop following (flip.c:524–590, crop.c:1235) |
| lens correction | find camera, find lens: lensfun's matches for the EXIF names as a menu (lens.cc:3857, 4184; list `find_camera`/`find_lens` of `requestChoices`) |
| every blending module | show color and set range of the parametric mask (blend_gui.c:1050–1910) |

GUI-only state darktable keeps in its widgets or configuration (exposure's and color
calibration's target and mode, the colour balance patches) lives in the rows' GUI state for the
session; it is sent with each request and the tool returns what it measured. It is not stored
across restarts as darktable's `darkroom/modules/*` keys are.

Still a notice: ashift's structure fitting buttons (vertical/horizontal/both and the auto toggle
need darktable's line detection and optimiser), color mapping's acquire as source/target
(k-means clustering and the cross-image hand-over through darktable's GUI), color harmonizer's
set from vectorscope (darktable's scopes panel), color calibration's colour checker buttons,
rasterfile's vectorize (drawn masks), and the pickers of colorize, split-toning, graduated
density, monochrome, borders, watermark, invert, relight, color equalizer, colour checker and
retouch. The tone equalizer's auto-adjust buttons for its mask need the module's guided-filter
mask and are not ported.

## Components

Reusable module widgets in `omalux/ui/components/`. Each takes `theme` and its data as
properties and reports changes through signals; none of them edits a parameter itself.
`omalux/tests/components/ComponentGallery.qml` shows all of them with sample data and
`tst_components.qml` checks their interaction (`qmltestrunner -input` on that file).

- `CurveEditor` — node curve for tone curve, rgb curve, base curve, color zones and rgb
  levels. Draws darktable's interpolation (`CurveMath.js`: natural cubic spline, darktable's
  Catmull-Rom, monotone Hermite, plus linear) with the edge behaviour of either sampler
  (`splineVersion` 1: `curve_tools.c`, flat beyond the end nodes; 2: `splines.cpp`, linear
  extrapolation and the periodic hue axis); checked value for value against `splines.cpp`.
  Drag a node (it never passes its neighbours), Ctrl+click adds one on the curve,
  double-click adds or removes, right-click removes a node or offers the reset; arrows move
  the active node. `xLog`/`yLog` use tone curve's "scale for graph" mapping.
- `GraphView` — response over fixed x positions (tone equalizer, wavelet levels); read-only
  or with draggable bars when editable.
- `ModuleTabs` — a module's notebook pages in the sidebar's recessed tab style.
- `ChannelChooser` — chips that pick which channel or curve is shown; a view choice only.
- `ColorSwatch` — colour parameter with a hue × saturation, value and hex picker.
- `ModuleNotice` — one muted line for what cannot be edited here yet.
- `ModuleToolButtons` — a row of pickers (darktable's pipette, highlighted while active) and
  buttons, optionally with a menu; reports the entry and menu item chosen.
- `PickerOverlay` — the active picker's area or point on the photo (in `PhotoViewport`).
- `HistogramView` — a module input histogram (logarithmic) with handle markers.
- `ModuleTools` — non-visual: which rows and sliders have pickers and buttons, the active
  picker, its boxes and results.
- `ChoiceRow` — a value from a list darktable fills at runtime or from a file dialog: shows
  the current value, opens a list with a search field (sections, details, "n more — refine the
  search"), reports `requested(query)`, `chosen(index)` and `fileChosen(path)`.
- `PatchGrid` — the colour checker patches of color look up table as darktable draws them:
  click selects, double-click resets the patch to its source, right-click removes it.
- `ControlChoice` and `ControlSwitch` — darktable enums (`[{ value, label }]`) and booleans;
  `labelFont`/`labelColor` let them match slider rows.
- `GeneratedModule`, `GeneratedRows`, `RowWrapper`, `ModuleList` — the generated modules and
  their rows (see above); `ModuleCatalog` and `ParameterQueue` are their non-visual data and
  write helpers, instantiated by the sidebar composition.
- `BlendSection` — darktable's blend section of one module instance (below).
- `BlendifRange` — one range of a parametric mask: darktable's gradient slider with four
  markers over the channel's colour gradient, the mask's opacity drawn as a line, darktable's
  marker labels and the ± polarity button. Drag a marker (it never passes its neighbours),
  double-click resets the range; Enter lends the keys, ←/→ move the active marker by the
  channel's increment (Shift ×10, Ctrl ×0.1), ↑/↓ pick the marker.
- `InstanceButton`, `InstanceFooter` — darktable's multi-instance button in a module heading
  and the keyboard's way to its menu at the end of an open module.

## Blending and module instances

### The blend section

Every module darktable lets blend (`IOP_FLAGS_SUPPORTS_BLENDING`, with an enable button) shows
its blend section at the end of the expanded module, in curated blocks and generated modules
alike. Labels, units, digits and ranges come from `layout-blending.json`; the rows follow
darktable's `dt_iop_gui_init_blending` / `dt_iop_gui_update_blending` (`develop/blend_gui.c`
3393, 3051):

- **blend mask mode** — off, uniformly, drawn mask, parametric mask, drawn & parametric mask,
  raster mask. Drawn and raster need mask support (`IOP_FLAGS_NO_MASKS` unset), parametric a
  module that blends in Lab or RGB; only the possible modes are offered.
- **blend colorspace** — darktable's blending options menu (`_blendif_options_callback`):
  Lab only for Lab modules, RGB (display), RGB (scene); right-click resets to the module's
  default. Changing it re-initialises the parametric mask and takes the settings of the last
  history item in that colour space (`_blendif_change_blend_colorspace`, 1912).
- **blend mask**: **mode** with the modes and order darktable offers in the current colour
  space (deprecated modes only while used), **toggle blend order**, **fulcrum** (RGB (scene)
  with addition, multiply, subtract, divide or an RGB channel mode; other modes reset it to 0,
  `_blendop_blend_mode_callback`, 716) and **opacity**.
- **drawn mask**: how many shapes the mask group holds ("no mask used", "N shapes used") and
  **toggle polarity of drawn mask** (`DEVELOP_COMBINE_MASKS_POS`). The shape buttons (add
  gradient, path, ellipse, circle, brush) show the module's drawn mask on the photo with that
  shape picked (`EditorSidebar.drawnShapeRequested` → `Main.requestDrawnShape`, see
  [Drawing on the image](#drawing-on-the-image)); `masks_api.h` is implemented in `shapes.c`.
- **raster mask**: the raster masks of earlier modules as darktable lists them
  (`_raster_combo_populate`, 2856: every module before this one that advertises one) and
  **toggle polarity of raster mask**.
- **parametric mask**: the channel chips (Lab: L a b C h; RGB (display): g R G B H S L;
  RGB (scene): g R G B Jz Cz hz), the **output** range (shown once used or with **show output
  channels**; switching that off resets them, "reset and hide output channels") and the
  **input** range (`BlendifRange`), **boost factor** (shown relative to darktable's offset, so
  Jz and Cz read 0 at their default; rescales the markers like
  `_blendop_blendif_boost_factor_callback`, 1283), **combine masks** (inverts the unused
  channels for inclusive modes, `_blendop_masks_combine_callback`, 765), **reset blend mask
  settings** and **invert all channel's polarities** (`_blendop_blendif_reset`,
  `_blendop_blendif_invert`, 1553/1570). A channel is processed once its range no longer spans
  everything (`_blendop_blendif_sliders_callback`, 831).
- **mask refinement** (drawn or parametric mask, or a raster mask): **details threshold**
  (raw images only), **feathering guide**, **feathering radius**, **blurring radius**, **mask
  opacity**, **mask contrast**; a module blending in raw data keeps only the blur.

The parametric mask's two pickers are built (see "Pickers and module buttons"): **show color**
marks the picked mean and min…max band on both channel sliders with darktable's value label,
**set range** sets the four markers of the input slider from the picked area (Ctrl+drag: the
output slider, while it is shown) with darktable's 1 % feather, switches the channel on and sets
its polarity so the picked values are included (`blend_color_picker_apply`, blend_gui.c:1757).
Not built yet: darktable's display mask / temporarily switch off mask buttons and the
alternative (log, magnifier) marker scales.

Edits use the generic path `blend.<name>` in `setParameters`, so a module edit and a blend
edit can share one history item and the parameter queue merges them like any other drag:

| path | value | darktable rule |
| --- | --- | --- |
| `blend.mask_mode` | 0, 1, 3, 5, 7, 9 | `_blendop_masks_mode_callback` |
| `blend.blend_cst` | 0 (module default), 2, 3, 4 | `_blendif_change_blend_colorspace` |
| `blend.blend_mode`, `blend.reverse` | mode value, 0/1 | `_blendop_blend_mode_callback`, `_blendop_blend_order_clicked` |
| `blend.blend_parameter`, `blend.opacity` | EV, % | fulcrum, opacity |
| `blend.drawn_polarity` | 0/1 | `_blendop_masks_polarity_callback` |
| `blend.mask_combine` | 0–3 | `_blendop_masks_combine_callback` (with `blend.output_channels_shown`) |
| `blend.blendif_parameters[i]` | 0…1, i = 4 × channel + marker | `_blendop_blendif_sliders_callback` |
| `blend.polarity[ch]` | 1 = negative | `_blendop_blendif_polarity_callback` |
| `blend.reset_channel[ch]` | 1 | double-click on a range |
| `blend.boost_factor[in]` | shown EV | `_blendop_blendif_boost_factor_callback` |
| `blend.reset_parametric`, `blend.invert_all`, `blend.clean_output_channels` | 1 | reset, invert, hide output channels |
| `blend.details`, `blend.feathering_guide`, `blend.feathering_radius`, `blend.blur_radius`, `blend.brightness`, `blend.contrast` | raw values | refinement |
| `blend.raster_mask@<operation>/<instance>` | mask id | `_raster_value_changed_callback` |
| `blend.raster_mask` | −1 | "no mask used" |
| `blend.raster_mask_invert` | 0/1 | `_raster_polarity_callback` |

Like a darktable blend widget, a blend edit switches its module on. The catalog entry of a
module carries a `blend` object: the current values, what the module supports (`masks`,
`parametric`, `csp`, `default_cst`, `raw`), `outputs_used`, `drawn_shapes`, `drawn_available`,
the link state of a raster mask and the `raster_masks` the module offers to later modules;
while a mask is on also the parametric ranges and boost factors, the offered `blend_modes` and
the parametric `channels` (a module whose mask is off stays small in the catalog).

### Module instances

The heading of every module, curated or generated, carries darktable's multi-instance button
(two frames). Its menu has darktable's entries — **new instance**, **duplicate instance**,
**move up**, **move down**, **delete**, **rename** — enabled as darktable's `_get_multi_show`
decides; right-click on the button creates a new instance, and the heading's context menu
offers new and duplicate too. Move up means later in the pipeline, as in darktable's panel; the
neighbour it moves past is the next module with a GUI (deprecated ones only while enabled),
since Omalux has no right-hand panel order. Rename opens a field; Enter or leaving the field
keeps the name, Escape cancels, an empty name gives the module back its automatic label. The
heading shows darktable's instance label after a dot (`_iop_panel_name`): the hand-edited name,
otherwise the automatic one (darktable names unnamed instances by their number or by a matching
preset, e.g. "exposure • scene-referred default"). Further instances are listed after their base
instance; under a curated block they appear as generated modules with every row.

`backend.moduleInstance(operation, instance, action, name)` runs one action on the worker
(`om_engine_module_instance`, `native/engine/module_instances.c`), after which the whole catalog
is described again. Panes pass actions through `changesRequested` as action keys that the
sidebar composition routes (`{"@instance": action, "@name": name}`; `{"@reset": 1}` for a
module reset and `{"@drawn": type}` for a shape request, re-emitted as
`EditorSidebar.drawnShapeRequested`). History follows darktable: a new instance records the base
module (when it is not the last history item) and the new one; deleting an instance removes its
history items, and deleting instance 0 renumbers the instance first in history to 0. Duplicating
a module whose drawn mask holds shapes is refused, as their copy needs darktable's GUI develop
context (`dt_masks_iop_use_same_as`).

## Drawing on the image

darktable draws some modules on the image: shapes, lines, warps and handles that its mouse
handlers turn into parameters. Omalux shows them over the photo for the module whose row is
selected, as darktable shows the overlay of the focused module: any row of vignetting,
graduated density, rotate and perspective (also the curated rotation in Crop & Rotate),
liquify, retouch or spot removal, the module's `canvas` row ("show on photo"), or a
blend section's shape button (`operation/instance/@shapes`). History and Info show none, nor
does cropping. A small toolbar at the top left of the photo names the module and holds its
tools; Escape leaves a drawing tool. All positions cross the engine interface as fractions of
the processed image as the preview shows it (`omalux/native/engine/canvas.h`); the adapter
converts them with darktable's distortion transforms on the full pipe, so a shape stays on its
image feature through crop, rotation, perspective and lens correction.

| Module | On the photo | darktable source ported |
| --- | --- | --- |
| vignetting | centre, inner and outer ellipse; handles for centre, width, height (Ctrl: size instead of ratio) and fall-off | `vignette.c` `gui_post_expose`, `mouse_moved` (381, 469), in QML (`VignetteOverlay`) |
| graduated density | the line with its end arrows; drag an end or the line, right-drag (or *draw line*) draws a new one; applied on release | `graduatednd.c` `_set_grad_from_points`, `_set_points_from_grad`, `button_released` (206, 307, 670) |
| rotate and perspective | right-drag (or *straighten*) a level or plumb line: rotation minus its angle (at least 25 screen pixels); *lines* and *rectangle* draw structure, stored in `last_drawn_lines`/`last_quad_lines` in input pixels; ends and corners stay draggable; *clear* | `ashift.c` `_calculate_straightening` (4070), `_draw_save_lines_to_params` (3008) |
| retouch | shapes circle, ellipse, path, brush; algorithms clone, heal, blur, fill (Ctrl: the selected shape, within clone↔heal and blur↔fill); the selected shape's opacity and *remove* | `retouch.c` `rt_resynch_params`, `rt_select_algorithm_callback`, `rt_shape_selection_changed`, `gui_changed` |
| spot removal | circle, ellipse and path clones | `spots.c` `_resynch_params` |
| drawn blend mask | circle, ellipse, path, brush and gradient in the module's mask group; adding one sets the drawn mask mode | `blend_gui.c` shape buttons, `masks.c` `dt_masks_gui_form_save_creation` |
| liquify | each warp's centre, radius circle and strength arrow, curve control points; *point*, *line*, *curve*; Ctrl+click on an arrow cycles linear, grow, shrink; right-click deletes a node, Ctrl+right-click its path | `liquify.c` node handling, `smooth_paths_linsys`, `get_point_scale`, `get_stamp_params` |

Shapes (develop/masks): pick a shape, then click (circle, ellipse), drag (gradient: the
direction), click the corners (path; finish with a right-click, a double-click or on the
first corner) or paint (brush; simplified with darktable's Ramer–Douglas–Peucker step and
Catmull-Rom control points). Ctrl when picking keeps adding; Shift+click first places a clone
source, otherwise darktable's default offset is used. Drag a shape to move it, its dashed
source to move that, its outline to resize it, its dashed feather line to soften it, the
round handle to turn an ellipse or gradient. The wheel over a shape: size, Shift feather,
Ctrl opacity, Shift+Ctrl rotation (darktable's steps and limits); right-click removes it.
New shapes take darktable's configured defaults, and resizing stores the new default, as in
darktable. The selected shape is darktable's `mask_form_selected_id`; editing retouch's blur
or fill fields edits the selected shape, as darktable's `gui_changed` does.

Each gesture is one history item, consecutive steps of one drag merge like a slider drag
(the worker merges queued moves and multiplies scale factors), and the module is switched
on. While a drag is on, the shape follows the pointer locally; the engine's overlay replaces
it after release. Drawn forms are part of the history (`dt_dev_add_masks_history_item_ext`),
so history jumps restore them, and of the XMP written for export and for split mode.

Engine and app pieces: `canvas.c` (dispatch, spaces, graduated density, rotate and
perspective, sidecar), `shapes.c` (forms, retouch/spots bookkeeping, `masks_api.h`),
`liquify_canvas.c`; `EngineWorker` keeps the shown module and sends its overlay after each
render that changed it (`canvasReady`), `Editor.setCanvasModule`, `editCanvas` and
`canvasOverlay`; QML `CanvasOverlay` picks `VignetteOverlay`, `GradientLineOverlay`,
`StructureOverlay`, `ShapesOverlay` or `LiquifyOverlay`, `CanvasToolbar` and `CanvasToolRow`
(sidebar row). Tests: `omalux/tests/components/tst_canvas.qml` (gestures without an engine),
`omalux/tests/canvas-engine.json` (every gesture against the engine, history jumps comparing
pixels, the sidecar) and `omalux/tests/canvas.json` (real pointer events on the photo).

Not built: darktable's path and brush node editing (moving single corners or control
points, adding or deleting nodes on a path), feather handles per path node, gradient
curvature by drag (the wheel changes it), shape groups with several operations
(union/intersection/difference set in the mask manager), the mask manager itself, colour
checker calibration on the image, retouch's wavelet scales bar and preview levels, ashift's
automatic cropping (darktable computes it in its GUI; a straightened photo keeps black
corners until crop is set) and the fit buttons that use the drawn structure. The overlay of
a path border and a brush stroke follows darktable's geometry (border × shorter input side
along the Bézier normal) but not its exact border construction.

