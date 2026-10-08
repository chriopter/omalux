// SPDX-License-Identifier: GPL-3.0-or-later
// darktable's RYB vectorscope and its colour harmony guide without the scopes panel, ported
// from darktable 5.6.1 (src/libs/scopes/vectorscope.c), for color harmonizer's "vectorscope
// two-way sync" and "set from vectorscope" (colorharmonizer.c:814-940, 1380).
//
// Both tools run on the hidden "gamma" module: its input is the fully processed image in the
// output profile, which is what darktable's histogram receives from the preview pipe (with the
// default sRGB histogram profile the RYB plot reads it as sRGB, _get_chromaticity).
//   vectorscope:   the RYB plot (log scale, darktable's 2 × 2 averaging, bins and HLG intensity
//                  of _vec_process) drawn as _vec_draw orients and colours it, as a base64
//                  PNG of diameter² pixels ("png"), plus the image's guide
//   harmony_guide: gui {type, rotation, width}: stores the guide like
//                  _color_harmony_changed_record (dt_conf keys and the image's guide)
#include "module_tools_internal.h"
#include "common/color_harmony.h"
#include "common/color_ryb.h"
#include "common/colorspaces_inline_conversions.h"
#include "common/curve_tools.h"
#include "common/image_cache.h"
#include "common/math.h"

#define VS_DIAMETER 256 // darktable draws 384 px; the bins scale with it (gain below)
#define VS_BASE_LOG 30  // vectorscope.c:35
#define VS_RADIUS 0.01f // the RYB hue ring's radius (_lib_histogram_vectorscope_bkgd, 286-289)

static const char *const harmony_names[DT_COLOR_HARMONY_N] = {
    "none", "monochromatic", "analogous", "analogous complementary", "complementary", "split complementary",
    "dyad", "triad", "tetrad", "square"}; // _vec_color_harmonies, vectorscope.c:60

// vectorscope.c:150, the RGB hue knots of the RYB spline.
static const float vs_rgb_y_vtx[7] = {0.0, 0.083333, 0.166667, 0.383838, 0.586575, 0.833333, 1.0};

static cairo_status_t png_append(void *closure, const unsigned char *data, unsigned int length) {
    g_byte_array_append(closure, data, length);
    return CAIRO_STATUS_SUCCESS;
}

// vectorscope.c baselog and log_scale (174-194).
static inline float baselog(const float x, const float bound) {
    return log1pf((VS_BASE_LOG - 1.f) * x / bound) / logf(VS_BASE_LOG) * bound;
}
static inline void log_scale(float *x, float *y, const float r) {
    const float h = hypotf(*x, *y);
    if (h >= FLT_MIN) {
        const float s = baselog(h, r) / h;
        *x *= s;
        *y *= s;
    }
}

static void add_guide(JsonObject *out, const dt_color_harmony_guide_t *guide) {
    JsonObject *g = json_object_new();
    json_object_set_int_member(g, "type", guide->type);
    json_object_set_string_member(g, "name", harmony_names[CLAMP((int)guide->type, 0, DT_COLOR_HARMONY_N - 1)]);
    json_object_set_int_member(g, "rotation", guide->rotation);
    json_object_set_int_member(g, "width", guide->width);
    json_object_set_object_member(out, "guide", g);
}

// The image's guide as _vec_signal_image_changed (1301-1336) restores it: a stored guide, or
// none with the configured rotation and width.
static void image_guide(OmToolContext *ctx, dt_color_harmony_guide_t *guide) {
    dt_color_harmony_init(guide);
    const dt_image_t *img = dt_image_cache_get(ctx->dev->image_storage.id, 'r');
    if (img) {
        *guide = img->color_harmony_guide;
        dt_image_cache_read_release(img);
    }
    if (guide->type == DT_COLOR_HARMONY_NONE) {
        guide->rotation = dt_conf_get_int("plugins/darkroom/histogram/vectorscope/harmony_rotation");
        guide->width = dt_conf_get_int("plugins/darkroom/histogram/vectorscope/harmony_width");
    }
    guide->custom_n = 0;
}

// vectorscope.c _vec_process (494-683) for the RYB type and the logarithmic scale.
static int vectorscope(OmToolContext *ctx) {
    OmCapture *c = ctx->capture;
    if (!c || !c->input || c->dsc.channels != 4)
        return 4;
    float *ypp = interpolate_set(7, (float *)dt_color_ryb_x_vtx, (float *)dt_color_ryb_y_vtx, CUBIC_SPLINE);
    if (!ypp)
        return 4;
    const int diam_px = VS_DIAMETER, width = c->roi.width, height = c->roi.height;
    const float max_radius = VS_RADIUS, max_diam = 2.f * max_radius;
    int *binned = g_new0(int, diam_px * diam_px);
    const int sample_max_x = width - (width % 2), sample_max_y = height - (height % 2);
    for (int y = 0; y < sample_max_y; y += 2)
        for (int x = 0; x < sample_max_x; x += 2) {
            dt_aligned_pixel_t RGB = {0.f}, rgb, RYB, HSV, HCV;
            const float *px = c->input + 4 * ((size_t)y * width + x);
            for (int xx = 0; xx < 2; xx++)
                for (int yy = 0; yy < 2; yy++)
                    for (int ch = 0; ch < 3; ch++)
                        RGB[ch] += px[4 * (yy * width + xx) + ch] * 0.25f;
            // _get_chromaticity, RYB (466-477) with _rgb2ryb (163-172)
            dt_sRGB_to_linear_sRGB(RGB, rgb);
            dt_RGB_2_HSV(rgb, HSV);
            HSV[0] = interpolate_val(7, (float *)dt_color_ryb_x_vtx, HSV[0], (float *)dt_color_ryb_y_vtx, ypp,
                                     CUBIC_SPLINE);
            dt_HSV_2_RGB(HSV, RYB);
            dt_RGB_2_HCV(RYB, HCV);
            const float alpha = DT_2PI_F * HCV[0];
            float cx = cosf(alpha) * HCV[1] * 0.01f, cy = sinf(alpha) * HCV[1] * 0.01f;
            log_scale(&cx, &cy, max_radius);
            const int out_x = (diam_px - 1) * (cx / max_diam + 0.5f);
            const int out_y = (diam_px - 1) * (cy / max_diam + 0.5f);
            if (out_x >= 0 && out_x <= diam_px - 1 && out_y >= 0 && out_y <= diam_px - 1)
                binned[out_y * diam_px + out_x]++;
        }
    free(ypp);
    // intensity through the HLG curve (vectorscope.c:660-680)
    const dt_iop_order_iccprofile_info_t *profile =
        dt_ioppr_add_profile_info_to_list(ctx->dev, DT_COLORSPACE_HLG_REC2020, "", DT_INTENT_PERCEPTUAL);
    guchar *graph = g_new0(guchar, diam_px * diam_px);
    const float gain = 1.f / 30.f;
    const float scale = gain * (diam_px * diam_px) / ((float)width * height);
    for (int i = 0; i < diam_px * diam_px; i++) {
        const float v = MIN(1.f, scale * binned[i]);
        const float intensity =
            profile && profile->lut_out[0] ? profile->lut_out[0][(int)(v * (profile->lutsize - 1))] : v;
        graph[i] = (guchar)CLAMP(intensity * 255.0f, 0.f, 255.f);
    }
    g_free(binned);
    // The picture as _vec_draw shows it: rotated by the default angle (270°, conf
    // .../vectorscope/angle), v up, each bin in the display colour of its RYB hue
    // (_lib_histogram_vectorscope_bkgd: _ryb2rgb of the hue ring) with the graph as alpha.
    float *ryb2rgb = interpolate_set(7, (float *)dt_color_ryb_x_vtx, (float *)vs_rgb_y_vtx, CUBIC_SPLINE);
    cairo_surface_t *surface = cairo_image_surface_create(CAIRO_FORMAT_ARGB32, diam_px, diam_px);
    const int stride = cairo_image_surface_get_stride(surface);
    unsigned char *pixels = cairo_image_surface_get_data(surface);
    cairo_surface_flush(surface);
    for (int sy = 0; sy < diam_px; sy++)
        for (int sx = 0; sx < diam_px; sx++) {
            uint32_t *px = (uint32_t *)(pixels + sy * stride) + sx;
            *px = 0;
            // screen = rotate(270°) · (cx, −cy): the inverse gives the bin
            const float dx = sx - 0.5f * (diam_px - 1), dy = sy - 0.5f * (diam_px - 1);
            const float cx = -dy, cy = -dx;
            const int ox = (int)lroundf(cx + 0.5f * (diam_px - 1)), oy = (int)lroundf(cy + 0.5f * (diam_px - 1));
            if (ox < 0 || oy < 0 || ox >= diam_px || oy >= diam_px)
                continue;
            const int a = graph[oy * diam_px + ox];
            if (!a)
                continue;
            float hue = atan2f(cy, cx) / DT_2PI_F;
            if (hue < 0.f)
                hue += 1.f;
            dt_aligned_pixel_t HSV = {hue, 1.f, 1.f, 0.f}, ryb, rgb;
            dt_HSV_2_RGB(HSV, ryb);
            // _ryb2rgb (152-161)
            dt_RGB_2_HSV(ryb, HSV);
            HSV[0] = ryb2rgb ? interpolate_val(7, (float *)dt_color_ryb_x_vtx, HSV[0], (float *)vs_rgb_y_vtx, ryb2rgb,
                                              CUBIC_SPLINE)
                             : HSV[0];
            dt_HSV_2_RGB(HSV, rgb);
            const int r = CLAMP((int)(rgb[0] * a), 0, 255), g = CLAMP((int)(rgb[1] * a), 0, 255),
                      b = CLAMP((int)(rgb[2] * a), 0, 255);
            *px = ((uint32_t)a << 24) | ((uint32_t)r << 16) | ((uint32_t)g << 8) | (uint32_t)b;
        }
    cairo_surface_mark_dirty(surface);
    free(ryb2rgb);
    GByteArray *png = g_byte_array_new();
    cairo_surface_write_to_png_stream(surface, png_append, png);
    cairo_surface_destroy(surface);
    JsonObject *out = json_object_new();
    json_object_set_int_member(out, "diameter", diam_px);
    gchar *base64 = g_base64_encode(png->data, png->len);
    json_object_set_string_member(out, "png", base64);
    g_free(base64);
    g_byte_array_free(png, TRUE);
    g_free(graph);
    json_object_set_object_member(ctx->extra, "vectorscope", out);
    dt_color_harmony_guide_t guide;
    image_guide(ctx, &guide);
    add_guide(ctx->extra, &guide);
    return 0;
}

// _color_harmony_toggled / _vec_eventbox_scroll end in _color_harmony_changed_record
// (1060-1085): the conf keys (rotation and width only with a guide) and the image's guide.
static int harmony_guide(OmToolContext *ctx) {
    dt_color_harmony_guide_t guide;
    image_guide(ctx, &guide);
    guide.type = CLAMP((int)om_tool_gui(ctx, "type", guide.type), 0, DT_COLOR_HARMONY_N - 1);
    guide.rotation = (((int)om_tool_gui(ctx, "rotation", guide.rotation)) % 360 + 360) % 360;
    guide.width = CLAMP((int)om_tool_gui(ctx, "width", guide.width), 0, DT_COLOR_HARMONY_WIDTH_N - 1);
    dt_conf_set_string("plugins/darkroom/histogram/vectorscope/harmony_type", harmony_names[guide.type]);
    if (guide.type != DT_COLOR_HARMONY_NONE) {
        dt_conf_set_int("plugins/darkroom/histogram/vectorscope/harmony_width", guide.width);
        dt_conf_set_int("plugins/darkroom/histogram/vectorscope/harmony_rotation", guide.rotation);
    }
    dt_image_t *img = dt_image_cache_get(ctx->dev->image_storage.id, 'w');
    if (img) {
        img->color_harmony_guide = guide;
        dt_image_cache_write_release_info(img, DT_IMAGE_CACHE_RELAXED, "omalux harmony guide");
    }
    add_guide(ctx->extra, &guide);
    return 0;
}

// colorharmonizer.c _set_from_vectorscope_callback (1380-1390) with _apply_harmony_guide
// (861-877): the guide's harmony becomes the rule (type − 1) and its rotation the anchor hue,
// converted from the RYB angle (the "@anchor_hue" conversion of module_values.c,
// _ryb_to_ucs_fast). Without a guide nothing changes.
static int harmonizer_from_vectorscope(OmToolContext *ctx) {
    dt_iop_module_t *m = ctx->module;
    if (!m->get_f || !m->get_f("rule") || !m->get_f("anchor_hue"))
        return 3;
    dt_color_harmony_guide_t guide;
    image_guide(ctx, &guide);
    add_guide(ctx->extra, &guide);
    if (guide.type == DT_COLOR_HARMONY_NONE)
        return 0;
    *(int *)m->get_p(m->params, "rule") = guide.type - 1;
    const char *path = "@anchor_hue";
    const double value = guide.rotation;
    const char *text = NULL;
    return om_module_set_values(ctx->engine, m, 1, &path, &value, &text) ? 5 : 0;
}

const OmToolSpec om_tools_vectorscope[] = {
    {"colorharmonizer", "set_from_vectorscope", OM_TOOL_BUTTON, -1, 0, harmonizer_from_vectorscope},
    {"gamma", "vectorscope", OM_TOOL_BUTTON, -1, OM_TOOL_CAPTURE | OM_TOOL_KEEP_OFF, vectorscope},
    {"gamma", "harmony_guide", OM_TOOL_BUTTON, -1, OM_TOOL_KEEP_OFF, harmony_guide},
    {NULL, NULL, 0, 0, 0, NULL},
};
