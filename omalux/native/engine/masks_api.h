// SPDX-License-Identifier: GPL-3.0-or-later
#pragma once
// Drawn shapes of a module's blend mask (darktable "forms", develop/masks.h). The blend section
// only shows them and asks for them; drawing on the image is a separate piece of work that
// implements these functions. They are declared weak: while no implementation is linked they
// are NULL, the catalog reports "drawn_available": false and the blend section shows a notice
// instead of the shape buttons. Shape types are darktable's (DT_MASKS_CIRCLE, DT_MASKS_ELLIPSE,
// DT_MASKS_PATH, DT_MASKS_BRUSH, DT_MASKS_GRADIENT).
#include "engine.h"
#ifdef __cplusplus
extern "C" {
#endif
// Add one shape to the drawn mask of a module instance. `json` holds the shape's geometry in
// image-relative coordinates, as the on-image tool defines it. Like darktable's shape buttons
// (blend_gui.c, _blendop_masks_add_shape) this switches the module's mask mode to include
// drawn shapes when it had none, and records one history item. Returns 0 or an error code.
int om_engine_masks_add_shape(OmEngine *engine, const char *operation, int instance, int type,
                              const char *json) __attribute__((weak));
// Remove every shape from the drawn mask of a module instance; one history item.
int om_engine_masks_clear(OmEngine *engine, const char *operation, int instance) __attribute__((weak));
#ifdef __cplusplus
}
#endif
