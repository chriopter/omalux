// SPDX-License-Identifier: GPL-3.0-or-later
// Buttons of the geometry modules, ported from darktable 5.6.1 without their GTK code.
#include "module_tools_internal.h"
#include "common/image.h"

static int *param_int(dt_iop_module_t *module, const char *name) {
    const dt_introspection_field_t *field = module->get_f ? module->get_f(name) : NULL;
    if (!field || field->header.size != sizeof(int))
        return NULL;
    return module->get_p(module->params, name);
}

// crop.c _crop_handle_flip (lines 1235-1250): the crop rectangle follows the orientation.
static void crop_follow(OmToolContext *ctx, dt_image_orientation_t mode) {
    dt_iop_module_t *crop = om_tool_module(ctx, "crop");
    if (!crop || !crop->get_f || !crop->get_f("cx"))
        return;
    om_tool_also(ctx, crop);
    float *cx = crop->get_p(crop->params, "cx"), *cy = crop->get_p(crop->params, "cy");
    float *cw = crop->get_p(crop->params, "cw"), *ch = crop->get_p(crop->params, "ch");
    if (*cx == 0.f && *cy == 0.f && *cw == 1.f && *ch == 1.f)
        return;
    const float ocx = *cx, ocy = *cy;
    if (mode == ORIENTATION_FLIP_HORIZONTALLY) {
        *cx = 1.f - *cw;
        *cw = 1.f - ocx;
    } else if (mode == ORIENTATION_FLIP_VERTICALLY) {
        *cy = 1.f - *ch;
        *ch = 1.f - ocy;
    } else if (mode == ORIENTATION_ROTATE_CW_90_DEG) {
        *cx = 1.f - *ch;
        *ch = *cw;
        *cw = 1.f - *cy;
        *cy = ocx;
    } else if (mode == ORIENTATION_ROTATE_CCW_90_DEG) {
        *cx = *cy;
        *cy = 1.f - *cw;
        *cw = *ch;
        *ch = 1.f - ocx;
    }
}

static dt_image_orientation_t current_orientation(OmToolContext *ctx, int *p) {
    dt_image_orientation_t orientation = *p;
    if (orientation == ORIENTATION_NULL)
        orientation = dt_image_orientation(&ctx->dev->image_storage);
    return orientation;
}

// flip.c do_rotate (lines 524-548).
static int rotate(OmToolContext *ctx, gboolean cw) {
    int *p = param_int(ctx->module, "orientation");
    if (!p)
        return 3;
    dt_image_orientation_t orientation = current_orientation(ctx, p);
    if (!cw)
        orientation ^= (orientation & ORIENTATION_SWAP_XY) ? ORIENTATION_FLIP_Y : ORIENTATION_FLIP_X;
    else
        orientation ^= (orientation & ORIENTATION_SWAP_XY) ? ORIENTATION_FLIP_X : ORIENTATION_FLIP_Y;
    orientation ^= ORIENTATION_SWAP_XY;
    *p = orientation;
    crop_follow(ctx, cw ? ORIENTATION_ROTATE_CW_90_DEG : ORIENTATION_ROTATE_CCW_90_DEG);
    return 0;
}
static int rotate_ccw(OmToolContext *ctx) {
    return rotate(ctx, FALSE);
}
static int rotate_cw(OmToolContext *ctx) {
    return rotate(ctx, TRUE);
}

// flip.c _flip_h and _flip_v (lines 560-590).
static int flip(OmToolContext *ctx, gboolean horizontal) {
    int *p = param_int(ctx->module, "orientation");
    if (!p)
        return 3;
    const dt_image_orientation_t orientation = current_orientation(ctx, p);
    const gboolean swapped = (orientation & ORIENTATION_SWAP_XY) != 0;
    *p = orientation ^ ((horizontal != swapped) ? ORIENTATION_FLIP_HORIZONTALLY : ORIENTATION_FLIP_VERTICALLY);
    crop_follow(ctx, horizontal ? ORIENTATION_FLIP_HORIZONTALLY : ORIENTATION_FLIP_VERTICALLY);
    return 0;
}
static int flip_h(OmToolContext *ctx) {
    return flip(ctx, TRUE);
}
static int flip_v(OmToolContext *ctx) {
    return flip(ctx, FALSE);
}

const OmToolSpec om_tools_geometry[] = {
    {"flip", "rotate_ccw", OM_TOOL_BUTTON, -1, 0, rotate_ccw},
    {"flip", "rotate_cw", OM_TOOL_BUTTON, -1, 0, rotate_cw},
    {"flip", "flip_horizontally", OM_TOOL_BUTTON, -1, 0, flip_h},
    {"flip", "flip_vertically", OM_TOOL_BUTTON, -1, 0, flip_v},
    {NULL, NULL, 0, 0, 0, NULL},
};
