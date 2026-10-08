// SPDX-License-Identifier: GPL-3.0-or-later
// The blend section's "display mask" and "temporarily switch off blend mask" buttons
// (develop/blend_gui.c _blendop_blendif_showmask_clicked 1352, _blendop_blendif_suppress_toggled
// 1535) without darktable's GUI.
//
// darktable honours both only for the focused module on the darkroom pipe
// (blend.c:501, valid_request = dt_iop_has_focus(self), which needs an attached GUI). Headless:
// - display mask: the full pipe keeps every module's blend mask (store_all_raster_masks, the
//   export mechanism behind raster masks); after rendering, each pixel of the displayed image
//   is taken back through the later distortions to the module's mask, and composed as gamma.c
//   _mask_display does (grey image mixed with the mask by darkroom/ui/develop_mask_mix, 0.3,
//   then yellow by mask strength).
// - switch off mask: while rendering the module blends uniformly with its opacity
//   (blend.c:519-522, suppress_mask makes the mask uniform); its parameters and history are
//   untouched.
// Both are view state of one module, set by the "blend_display" tool:
//   gui {"mask": 0/1, "suppress": 0/1}; activating either switches the module on, as darktable.
#include "module_tools_internal.h"
#include "blend_display.h"
#include "develop/blend.h"

static struct {
    char operation[64];
    int instance;
    gboolean mask, suppress;
    dt_develop_blend_params_t saved;
    gboolean swapped;
    unsigned char *pixels;
    size_t size;
} state;

static dt_iop_module_t *display_module(OmEngine *engine) {
    if (!state.operation[0] || !engine->loaded)
        return NULL;
    return dt_iop_get_module_by_op_priority(engine->dev.iop, state.operation, state.instance);
}

static int blend_display(OmToolContext *ctx) {
    dt_iop_module_t *m = ctx->module;
    if (!m->blend_params || !(m->flags() & IOP_FLAGS_SUPPORTS_BLENDING))
        return 3;
    const gboolean mask = om_tool_gui(ctx, "mask", 0) > 0.5, suppress = om_tool_gui(ctx, "suppress", 0) > 0.5;
    dt_dev_pixelpipe_t *pipe = ctx->dev->full.pipe;
    dt_iop_module_t *previous = display_module(ctx->engine);
    if (previous)
        dt_dev_pixelpipe_cache_invalidate_later(pipe, previous->iop_order, "omalux mask display: ");
    memset(state.operation, 0, sizeof(state.operation));
    state.mask = mask;
    state.suppress = suppress;
    if (mask || suppress) {
        g_strlcpy(state.operation, m->op, sizeof(state.operation));
        state.instance = m->multi_priority;
    }
    pipe->store_all_raster_masks = mask;
    dt_dev_pixelpipe_cache_invalidate_later(pipe, m->iop_order, "omalux mask display: ");
    pipe->changed |= DT_DEV_PIPE_SYNCH;
    om_tool_set_gui(ctx, "mask", mask);
    om_tool_set_gui(ctx, "suppress", suppress);
    return 0;
}

void om_blend_display_reset(OmEngine *engine) {
    if (engine->loaded && engine->dev.full.pipe)
        engine->dev.full.pipe->store_all_raster_masks = FALSE;
    memset(state.operation, 0, sizeof(state.operation));
    state.mask = state.suppress = FALSE;
}

void om_blend_display_before_render(OmEngine *engine) {
    dt_iop_module_t *m = display_module(engine);
    state.swapped = FALSE;
    if (!m || !state.suppress || !m->blend_params)
        return;
    dt_dev_pixelpipe_t *pipe = engine->dev.full.pipe;
    if (pipe->loading)
        return;
    // The pipe commits parameters from history when it synchronises; synchronise first, then
    // let only this render's copy of the module's blend parameters blend uniformly.
    if (pipe->changed != DT_DEV_PIPE_UNCHANGED)
        dt_dev_pixelpipe_change(pipe, &engine->dev);
    dt_dev_pixelpipe_iop_t *piece = dt_dev_distort_get_iop_pipe(&engine->dev, pipe, m);
    dt_develop_blend_params_t *bp = piece ? piece->blendop_data : NULL;
    if (!bp || !(bp->mask_mode & ~DEVELOP_MASK_ENABLED))
        return;
    state.saved = *bp;
    bp->mask_mode = DEVELOP_MASK_ENABLED;
    state.swapped = TRUE;
    dt_dev_pixelpipe_cache_invalidate_later(pipe, m->iop_order, "omalux suppress mask: ");
}

// gamma.c _write_pixel (63-84) and _mask_display (246-262).
static inline unsigned char encode(const float v) {
    const float p = v <= 0.0031308f ? 12.92f * v : 1.055f * powf(v, 1.0f / 2.4f) - 0.055f;
    return (unsigned char)CLAMP(roundf(255.0f * p), 0.0f, 255.0f);
}

const unsigned char *om_blend_display_after_render(OmEngine *engine, const unsigned char *pixels, int width,
                                                    int height) {
    dt_iop_module_t *m = display_module(engine);
    if (state.swapped && m) {
        dt_dev_pixelpipe_iop_t *piece = dt_dev_distort_get_iop_pipe(&engine->dev, engine->dev.full.pipe, m);
        if (piece && piece->blendop_data)
            *(dt_develop_blend_params_t *)piece->blendop_data = state.saved;
        dt_dev_pixelpipe_cache_invalidate_later(engine->dev.full.pipe, m->iop_order, "omalux suppress mask: ");
        state.swapped = FALSE;
    }
    if (!m || !state.mask || !pixels || width <= 0 || height <= 0)
        return pixels;
    dt_dev_pixelpipe_t *pipe = engine->dev.full.pipe;
    dt_dev_pixelpipe_iop_t *piece = dt_dev_distort_get_iop_pipe(&engine->dev, pipe, m);
    const float *mask = piece && piece->raster_masks
                            ? g_hash_table_lookup(piece->raster_masks, GINT_TO_POINTER(BLEND_RASTER_ID))
                            : NULL;
    if (!mask)
        return pixels;
    const dt_iop_roi_t roi = piece->processed_roi_out;
    const size_t count = (size_t)width * height;
    float *points = g_try_new(float, 2 * count);
    if (!points)
        return pixels;
    const float scale = pipe->backbuf_scale > 0.f ? pipe->backbuf_scale : 1.f;
    for (int y = 0; y < height; y++)
        for (int x = 0; x < width; x++) {
            points[2 * ((size_t)y * width + x)] = (x + 0.5f) / scale;
            points[2 * ((size_t)y * width + x) + 1] = (y + 0.5f) / scale;
        }
    dt_dev_distort_backtransform_plus(&engine->dev, pipe, m->iop_order, DT_DEV_TRANSFORM_DIR_FORW_EXCL, points, count);
    if (state.size < 4 * count) {
        g_free(state.pixels);
        state.pixels = g_new(unsigned char, 4 * count);
        state.size = 4 * count;
    }
    const float mix = CLIP(dt_conf_get_float("darkroom/ui/develop_mask_mix"));
    for (size_t i = 0; i < count; i++) {
        const int mx = (int)floorf(points[2 * i] * roi.scale - roi.x);
        const int my = (int)floorf(points[2 * i + 1] * roi.scale - roi.y);
        const float a = mx >= 0 && my >= 0 && mx < roi.width && my < roi.height
                            ? CLIP(mask[(size_t)my * roi.width + mx])
                            : 0.0f;
        // the displayed bytes are BGRA: the encoded output profile values gamma received
        const unsigned char *in = pixels + 4 * i;
        const float lum = (0.3f * in[2] + 0.59f * in[1] + 0.11f * in[0]) / 255.0f;
        const float gray = mix * (a - lum) + lum;
        const unsigned char g = encode(gray);
        unsigned char *out = state.pixels + 4 * i;
        out[0] = (unsigned char)CLAMP(roundf(g * (1.0f - a)), 0.f, 255.f);             // blue: 0 in yellow
        out[1] = (unsigned char)CLAMP(roundf(g * (1.0f - a) + 255.0f * a), 0.f, 255.f); // green
        out[2] = (unsigned char)CLAMP(roundf(g * (1.0f - a) + 255.0f * a), 0.f, 255.f); // red
        out[3] = in[3];
    }
    g_free(points);
    return state.pixels;
}

const OmToolSpec om_tools_blend_display[] = {
    // blend_gui.c:3168-3190 the two buttons at the bottom of every blend section
    {"*", "blend_display", OM_TOOL_BUTTON, -1, 0, blend_display},
    {NULL, NULL, 0, 0, 0, NULL},
};
