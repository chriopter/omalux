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

- Each pane has its own QML file: Filters, Presets, Geometry, History and Metadata. Shared sliders, curve editor, crop overlay, viewport, dialogs and shortcuts live in `omalux/ui/components/`.
- The five sidebar SVG icons come from v0. History displays the native darktable processing stack, with current/enabled state, and refreshes after edits, styles and image changes. Click a step to restore it; later steps remain available until a new edit replaces the future branch.
- Group headers identify the module, including repeated labels such as `strength`, `brightness` or `chroma`. Click a module heading or its small right-hand checkmark to enable or disable it without losing its values. The checkmark indicates enabled state, not whether parameter values differ from defaults. Defaults may already produce an effect; reset does not mean bypass. Disabled headings and controls are dimmed together but remain interactive; editing a parameter enables the module again. Sections belonging to the same darktable module share its enabled state.
- Sidebar panes scroll vertically and stop at their edges. Wheel and touchpad events move the content immediately, without a separate scroll animation: Touchpad pixel deltas use an explicit 4× speed multiplier; angle-only events move 120 logical pixels per notch, including when Wayland classifies the seat as a touchpad. Like Omawrite 0.5.0, event shape determines the path, not the device label. Display scaling is used only to align positions to physical pixels, not to multiply speed. Both paths clamp to the content edges. Sliders and mode selectors do not consume the wheel. Hovering a slider does not change the selected parameter; keyboard navigation follows the displayed group order. History refreshes no longer explicitly reset the scroll position.
- Colored tracks indicate luminance, hue, saturation, temperature or tint. They are visual hints, not a simulation of the actual output.
- Slider tracks use darktable's soft range where specified. Double-click the numeric value to enter values within the full hard range. Reset arrows and `R` restore registry defaults, not the photo's original history.
- Crop uses the native crop module; dragging the frame changes a draft until Apply/Enter. Escape restores prior crop enablement. Rotation uses darktable's signed display conversion.
- Zoom, pan, pinch and fullscreen operate on the interactive preview. Zoom does not yet request a full-resolution detail render. JPEG/PNG export uses the actual full-resolution darktable export pipe.

See the main README for keyboard bindings and development commands. The native registry is `omalux/native/controls.h`; source labels and formatting were checked against the pinned/installed `src/iop/` modules, including Bauhaus percent conversion and GUI action paths. Display units are converted to native values before both native edits and GTK actions. Curves, blend settings and the diffusion recipe use temporary single-module styles for split synchronization.

## Validation and remaining limits

The optional `OMALUX_SMOKE_SCRIPT` development driver runs deterministic actions against the real engine. It exercised scalar edits, white balance, grain, wavelet curves, the diffusion recipe, style application, LUT opacity, own preset save/reapply and full-resolution JPEG/PNG export. A square crop produced a 1024 × 1024 export from the 1536 × 1024 source; bundle export retained the thumbnail and declared LUT, and own-preset deletion was checked. Split mode acknowledged control updates and a source-image switch followed by exposure editing; this does not establish byte-identical output or performance parity.

The adapter edits the base module instance. It does not expose every darktable parameter, custom module ordering, multiple-instance editing or masks. Own-preset snapshots reject unsupported masks, extra instances and external image dependencies rather than silently losing them. The original colisa controls are deprecated upstream; they remain for existing presets. See the architecture notes for cache and GPU-reporting limitations.

Scrolling reference: [Omawrite 0.5.0, Main.qml](https://github.com/omacom/omawrite/blob/v0.5.0/src/Main.qml), event handling and `snapToPixel`. Omalux adopts its event classification and pixel alignment; its angle-only movement remains immediate rather than using Omawrite’s animated wheel curve.
