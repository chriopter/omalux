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

- Each pane has its own QML file: Filters, Styles, Geometry, History and Metadata. Shared sliders, curve editor, crop overlay, viewport, dialogs and shortcuts live in `omalux/ui/components/`.
- The five sidebar SVG icons come from v0. History displays the native darktable processing stack, with current/enabled state, and refreshes after edits, styles and image changes. Click a step to restore it; later steps remain available until a new edit replaces the future branch.
- Module headings use medium-weight 11 px text: enabled modules are bright, disabled modules muted. Modules with a visible heading share one subtly lighter, borderless background across the heading and all associated sliders. Control labels and values stay at 12 px; headings have extra space above and a smaller gap to their controls.
- Each editing group corresponds to exactly one native darktable module. Controls in `shadows and highlights` and `color balance rgb` stay together. By explicit UI choice, brightness and saturation additionally have top-level shortcut rows sharing the colisa module state. The parent name toggles the module, and an accent dot marks its enabled state. Child labels select controls but never toggle a module. Editing a child enables its parent module, as in darktable; reset does not imply bypass.
- A collapsed single-primary module uses its primary row as the parent. Grain, bloom and vignetting show only their module name there. When expanded, the parent heading owns enablement and the same slider appears once below it with its original darktable label (`strength` or `brightness`). Additional parameters stay inside the same module background. Values stay right-aligned and disclosure occupies a separate right column. Right-click resets a parameter; child context menus do not expose a second module toggle.
- Eleven standard sliders remain available without expanding: brightness, contrast, saturation, exposure, shadows, temperature, vibrance, sharpen amount, grain strength, bloom strength and vignetting brightness. Brightness, contrast and saturation are separate top-level rows without a common heading. Editing any of them enables the same colisa module. Expanding contrast also shows all three together in its module block; repeated controls bind to the same values. At the user’s request, global vibrance is displayed as vibrance; its underlying parameter and units are unchanged. Shadows and highlights exposes only shadows while collapsed; highlights and white point adjustment appear on expansion. Local contrast is a separate native module and lives under Advanced alongside denoise, diffuse or sharpen and LUT 3D. Expansion is remembered per module and never changes processing values. Direct shortcuts reveal hidden secondary parameters. All parameter names and units come from darktable.
- Sidebar panes scroll vertically and stop at their edges. Wheel and touchpad events move the content immediately, without a separate scroll animation: Touchpad pixel deltas use an explicit 4× speed multiplier; angle-only events move 120 logical pixels per notch, including when Wayland classifies the seat as a touchpad. Like Omawrite 0.5.0, event shape determines the path, not the device label. Display scaling is used only to align positions to physical pixels, not to multiply speed. Both paths clamp to the content edges. Sliders and mode selectors do not consume the wheel. Hovering a slider does not change the selected parameter; keyboard navigation follows the displayed group order. History refreshes no longer explicitly reset the scroll position.
- Colored tracks indicate luminance, hue, saturation, temperature or tint. They are visual hints, not a simulation of the actual output.
- Slider tracks use darktable's soft range where specified. Double-click the numeric value to enter values within the full hard range. The context-menu reset and `R` restore registry defaults, not the photo's original history.
- Crop uses the native crop module; dragging the frame changes a draft until Apply/Enter. Escape restores prior crop enablement. Rotation uses darktable's signed display conversion.
- Zoom, pan, pinch and fullscreen operate on the interactive preview. Zoom does not yet request a full-resolution detail render. JPEG/PNG export uses the actual full-resolution darktable export pipe.

See the main README for keyboard bindings and development commands. The native registry is `omalux/native/engine/controls.h`; source labels and formatting were checked against the pinned/installed `src/iop/` modules, including Bauhaus percent conversion and GUI action paths. Display units are converted to native values before both native edits and GTK actions. Curves, blend settings and the diffusion recipe use temporary single-module styles for split synchronization.

## Validation and remaining limits

The optional `OMALUX_SMOKE_SCRIPT` development driver runs deterministic actions against the real engine. It exercised scalar edits, white balance, grain, wavelet curves, the diffusion recipe, style application, LUT opacity, own style save/reapply and full-resolution JPEG/PNG export. A square crop produced a 1024 × 1024 export from the 1536 × 1024 source; bundle export retained the thumbnail and declared LUT, and own-style deletion was checked. Split mode acknowledged control updates and a source-image switch followed by exposure editing; this does not establish byte-identical output or performance parity.

The adapter edits the base module instance. It does not expose every darktable parameter, custom module ordering, multiple-instance editing or masks. Own-style snapshots reject unsupported masks, extra instances and external image dependencies rather than silently losing them. The original colisa controls are deprecated upstream; they remain for existing styles. See the architecture notes for cache and GPU-reporting limitations.

Scrolling reference: [Omawrite 0.5.0, Main.qml](https://github.com/omacom/omawrite/blob/v0.5.0/src/Main.qml), event handling and `snapToPixel`. Omalux adopts its event classification and pixel alignment; its angle-only movement remains immediate rather than using Omawrite’s animated wheel curve.

- Denoise is available only in the expandable Advanced section. The collapsed vignetting brightness slider focuses on darkening (−1 to 0); expanding its details restores the full darktable range. Existing positive brightness values remain visible and are never changed by collapsing. Names, units and the default −0.5 are unchanged.

### Style hover

Hover over a style's thumbnail/name to preview it on the open photograph. Leaving the card or the Styles pane restores the current edited image immediately. Hover does not change controls or history, including any future history after a backward jump. Click to apply the style through the normal history path. Unsupported styles remain unavailable. The preview starts after a short hover delay and uses a separate temporary engine context.

### Native regression checks

Run `python3 omalux/tests/run.py --split` from the repository root to exercise the actual engine and comparison path after integration changes. The runner builds through `development/start`, uses isolated settings and a temporary copy of the style bundles, and checks slider gestures, hover/history, special controls, bundle assets, crop, export and image reopening. `magick` and a working desktop/OpenCL runtime are required. Omit `--split` to run only Omalux.

## What was applied for this camera

The History pane starts with a box listing what darktable set up before any style: the input profile and white balance (Colour), the lens correction (Lens), and exposure, tone mapping, highlight reconstruction, denoising and sharpening (Base tone). Values are read from the loaded modules after the image's history has been applied, so they show the actual state; entries in grey are not applied. Camera presets from `catalog/camera/` are imported into the session and their profiles copied into darktable's `color/in`, so this box also reflects them.

## Display data from darktable

darktable's introspection gives a parameter's name, type, range and default, but not how
the value reads on screen. That part lives in the calls that build the module's own
widgets: the unit, the factor between stored and shown value, the digits worth showing,
and the range a slider covers before it has to be forced wider.

`development/tools/darktable/extract_display.py` collects those calls from the module
sources into `omalux/design/display.json`, keyed `operation/field`. The raw module list
builds its sliders from it, so a parameter reads correctly without an entry in the
registry. Re-run the tool after updating the darktable submodule:

```
python3 development/tools/darktable/extract_display.py
```

It covers the 57 modules that build their sliders declaratively. The rest draw their own
widgets and still need a dedicated adapter.
