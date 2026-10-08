// SPDX-License-Identifier: GPL-3.0-or-later
#pragma once
// Drawing on the photograph: the on-canvas tools of darktable's modules without GTK.
//
// All positions crossing this interface are normalised preview coordinates: 0..1 across the
// processed image as the preview shows it (the full pipe's processed width and height). The
// adapter converts them with darktable's distortion transforms, the way each module's own
// mouse handlers do, so a point stays on the same image feature through crop, rotation,
// perspective and lens correction. Call these on the engine worker only, after a render.
#include "engine.h"
#ifdef __cplusplus
extern "C" {
#endif
// The overlay of one module instance as JSON (see development/docs/reference/controls.md,
// "Drawing on the image"), or NULL when the module has no on-canvas tool or no image is open.
char *om_engine_canvas(OmEngine *engine, const char *operation, int instance);
// Apply one gesture, a JSON object with an "action" member, as one history item (consecutive
// gestures on the same module merge like a slider drag). Returns 0, or 1 no image, 2 no such
// module, 3 unknown action or shape, 4 not possible for this module, 5 invalid gesture.
int om_engine_canvas_edit(OmEngine *engine, const char *operation, int instance, const char *gesture);
// Write the current develop history, including drawn shapes, as an XMP sidecar to `path` for
// the comparison process (darktable styles cannot carry drawn masks). Returns 0 on success.
int om_engine_canvas_sidecar(OmEngine *engine, const char *path);
// Whether a style snapshot of `operation` (or, for "*", of the whole history) is refused
// because of drawn forms, so that only those go as a sidecar.
int om_engine_canvas_uses_forms(OmEngine *engine, const char *operation);
// Called around each preview render: drawn shapes are not part of darktable's pipe hash in
// a headless develop context, so cached results after the first module that uses them are
// invalidated whenever the shapes change.
void om_engine_canvas_before_render(OmEngine *engine);
// The selected shape of a retouch instance follows the module's own fields, as darktable's
// gui_changed does: call after a generic parameter edit of `module` (a dt_iop_module_t *).
void om_engine_canvas_parameters_changed(OmEngine *engine, void *module);

// Drawn masks of a module's blend section: om_engine_masks_add_shape and om_engine_masks_clear
// are declared in masks_api.h (the blend section's interface) and implemented in shapes.c.
// Shapes are darktable forms in the module's mask group (blend_params->mask_id).
// Number of shapes in the module's mask group, or a negative error.
int om_engine_masks_count(OmEngine *engine, const char *operation, int instance);
#ifdef __cplusplus
}
#endif
