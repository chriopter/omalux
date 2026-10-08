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
- The sidebar has nine tabs in one recessed, rounded grid of three rows of three, each an icon with a short name beside it, in this order: Filters (the curated controls), Styles, Tone, Color, Detail, Effects, Crop & Rotate, History, Info. The first five icons come from v0; tone, color, detail and effects are new line icons in the same style. The active pane is a raised tile with an accent icon and name, hovered tabs brighten on a subtle fill, and a tooltip gives the longer description. The short names are Filters, Styles, Tone, Color, Detail, Effects, Crop, History and Info. Keys 1–9 select the panes in this strip order and Tab/Shift+Tab (] and [) step through it, so both follow what is shown (earlier, 1–5 kept the old pane numbers: Crop & Rotate was 3; it is now 7). The image toolbar uses the same flat outlined buttons: the key hint in muted text before the label. Zoom out, the current level and zoom in are grouped in one outline; the level reads `[0] FIT` at fit and the zoom factor (e.g. `2.0×`) otherwise, and clicking it fits the photograph, so there is no separate fit button. History displays the native darktable processing stack, with current/enabled state, and refreshes after edits, styles and image changes. Click a step to restore it; later steps remain available until a new edit replaces the future branch.
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
- **Generated modules** (Tone, Color, Detail, Effects, the look modules under Styles and the modules under Crop & Rotate) take part like the curated ones: group headings, module headings (Enter or ←/→ open the details, `E` switches the module, `Shift+R` resets it), every slider (one step of darktable's displayed precision, written in raw units through the module's parameter queue), choice and switch rows (`R` restores the row's default), module page tabs and channel choosers (←/→), colour swatches (Enter opens the picker), list rows (Enter opens the list; typing searches, ↑/↓ and Enter choose, Esc closes), text fields such as the watermark text (Enter edits), the colour checker patches (←/→ select a patch, `R` resets it to its source), "more", and curves and graphs. Notices and section captions are not stops.
- **Curves and graphs.** Enter lends the keys to the widget: its own arrows edit the points (as documented in `CurveEditor`/`GraphView`); `Esc` (a second one when a point is selected) or any key the widget does not use gives them back.
- **Keys on the selection.** ←/→ change a slider by darktable's step (`Shift` ×10, `Ctrl`/`Alt` ×0.1, clamped to the hard range, whole steps for integer parameters), choose the previous/next option, toggle switches and close/open modules and style groups. `Enter`/`Space` activate (module, style, history step, button, switch, search field). `R` resets the parameter, `Shift+R` the whole module, `E` switches the module; both go through the module heading when one is shown. A style selected from the keyboard previews like hovering.
- **Focus.** Clicking a control selects it for the keyboard, then focus returns to the navigator; the same happens after menus, the value entry and dialogs close. Text fields keep all keys while typing (no shortcut fires); `Esc` leaves the field, `Enter` or `↓` leave it and select the item below. Popups and dialogs own their keys while open.
- **Search.** `/` or `Ctrl+F` focus the global search under the tab strip; it is not one of the ↑/↓ stops. `Esc` clears it and returns the keys; `Enter` or `↓` keep the term and select the pane's first item. The Styles pane keeps its own search as a listed item.
- **Panes.** `1` … `9` follow the tab strip as shown, like Tab/Shift+Tab, so a number always names the tab at that place (reading the rows left to right).
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

The curated controls edit the base module instance. The generic module API below reaches every described parameter of any existing instance, but the UI does not yet create instances, and custom module ordering and masks are not supported. Own-style snapshots reject unsupported masks, extra instances and external image dependencies rather than silently losing them. The original colisa controls are deprecated upstream; they remain for existing styles. See the architecture notes for cache and GPU-reporting limitations.

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
the latest state. The curated controls keep their GTK action path. Snapshots reject extra
module instances and drawn masks, so edits of an instance other than 0 are not mirrored.
`python3 omalux/tests/run.py` records the mailbox of `omalux/tests/module-parameters.json`
(`OMALUX_RECORD_MAILBOX`, no comparison window) and checks it carries one current snapshot
each for exposure, tonecurve and rgbcurve. `omalux/tests/module-values.json` does the same for
the displayed conversions and runtime lists (color balance, color calibration, color
harmonizer, split-toning, color look up table, input profile, lens, LUT 3D).

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
- Drawn or on-canvas features (retouch, liquify, spots, the graduated density line, ashift
  structure lines, colour checker calibration) become one `notice` row; plain sliders of the
  same module are kept. Rows darktable fills from a runtime list or a file dialog (profiles,
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
- Shown as a muted notice instead of a control, merged per section: image pickers, buttons
  (auto-tune, lens/camera search, flip rotations, structure-line fitting), drawn features
  (retouch, liquify, spots, graduated density line, monochrome and colour correction grids,
  relight center, zone system), darktable's own graphs of filmic rgb/AgX/filmic, the picker
  settings of color calibration's mapping section and exposure's area mode, and the remaining
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

Not covered: gmic-compressed LUTs (`.gmz`, darktable's reader is bound to its GUI), the
"from image area" white balance entry and the lens/camera "find" buttons (image pickers and
buttons), color calibration's colour checker calibration and mapping picker settings. Own
styles: lens names, LUT paths (packaged as before), profiles and noise profiles are plain
parameters; watermark, overlay and raster mask modules that are switched on are rejected, as
before, because their files are not packaged. colorin and colorout have no enable button and
are not part of own styles.

The per-module blend section is written separately in the same row format to
`omalux/design/layout-blending.json` for a later step. Regenerate both after changing the
inventory, the decisions or `controls.h`:

```
python3 development/tools/darktable/build_layout.py      # add -v to list unconverted conditions
```

The generator stops with an error if a decision names a missing module or row, a module is
placed twice or not at all, a primary is not a row, a slider lacks a numeric range or default,
a combobox lacks values or its default, or a field repeats within a module. It prints modules
and rows per group, widget and custom-widget counts and the modules with notices.

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
