// SPDX-License-Identifier: GPL-3.0-or-later
// The colour pickers of colorize, split-toning, graduated density, monochrome, framing,
// watermark, invert, relight, color equalizer, color look up table and retouch, ported from
// darktable 5.6.1 (src/iop/*.c, color_picker_apply) without their GTK code. Parameters are
// reached through darktable's introspection. The guards darktable uses to "interrupt infinite
// loops" are kept: an unchanged colour writes nothing (and so records no history item).
#include "module_tools_internal.h"
#include "common/chromatic_adaptation.h"
#include "common/colorspaces.h"
#include "common/colorspaces_inline_conversions.h"
#include "common/math.h"

#define P(module, name) ((float *)(module)->get_p((module)->params, (name)))
#define PI(module, name) ((int *)(module)->get_p((module)->params, (name)))

static gboolean has(dt_iop_module_t *module, const char *name) {
    return module->get_f && module->get_f(name);
}

// A band of picked values for a display marker (relight's gradient slider, color equalizer's
// graph): min, mean and max in 0..1 of the axis darktable draws them on.
static void report_band(OmToolContext *ctx, const char *axis, float lo, float mean, float hi) {
    JsonObject *band = json_object_new();
    json_object_set_string_member(band, "axis", axis);
    json_object_set_double_member(band, "min", lo);
    json_object_set_double_member(band, "mean", mean);
    json_object_set_double_member(band, "max", hi);
    json_object_set_object_member(ctx->extra, "band", band);
}

// Hue and saturation from the picked colour, the way split-toning, graduated density and
// colorize turn it into their two sliders. Returns FALSE when nothing changes (darktable's
// "interrupt infinite loops" guard).
static gboolean set_hue_saturation(float *hue, float *saturation, const dt_aligned_pixel_t rgb) {
    float H = .0f, S = .0f, L = .0f;
    rgb2hsl(rgb, &H, &S, &L);
    if (fabsf(*hue - H) < 0.0001f && fabsf(*saturation - S) < 0.0001f)
        return FALSE;
    *hue = H;
    *saturation = S;
    return TRUE;
}

// colorize.c color_picker_apply (lines 236-266): the point picker on "hue", picked in Lab,
// converted to sRGB before HSL.
static int colorize_hue(OmToolContext *ctx) {
    dt_iop_module_t *m = ctx->module;
    if (!has(m, "hue") || !has(m, "saturation"))
        return 3;
    dt_aligned_pixel_t XYZ, rgb;
    dt_Lab_to_XYZ(ctx->picked.in[DT_PICK_MEAN], XYZ);
    dt_XYZ_to_sRGB(XYZ, rgb);
    set_hue_saturation(P(m, "hue"), P(m, "saturation"), rgb);
    return 0;
}

// splittoning.c color_picker_apply (lines 354-400): the point pickers on the two hue sliders,
// HSL of the picked module input (RGB).
static int splittoning_pick(OmToolContext *ctx, const char *hue, const char *saturation) {
    dt_iop_module_t *m = ctx->module;
    if (!has(m, hue) || !has(m, saturation))
        return 3;
    set_hue_saturation(P(m, hue), P(m, saturation), ctx->picked.in[DT_PICK_MEAN]);
    return 0;
}
static int splittoning_shadow(OmToolContext *ctx) {
    return splittoning_pick(ctx, "shadow_hue", "shadow_saturation");
}
static int splittoning_highlight(OmToolContext *ctx) {
    return splittoning_pick(ctx, "highlight_hue", "highlight_saturation");
}

// graduatednd.c color_picker_apply (lines 453-475): the point picker on "hue".
static int graduatednd_hue(OmToolContext *ctx) {
    dt_iop_module_t *m = ctx->module;
    if (!has(m, "hue") || !has(m, "saturation"))
        return 3;
    set_hue_saturation(P(m, "hue"), P(m, "saturation"), ctx->picked.in[DT_PICK_MEAN]);
    return 0;
}

// monochrome.c color_picker_apply (lines 441-461): the area picker on "highlights" sets the
// filter's a, b and size from the picked Lab mean and spread.
static int monochrome_highlights(OmToolContext *ctx) {
    dt_iop_module_t *m = ctx->module;
    if (!has(m, "a") || !has(m, "b") || !has(m, "size"))
        return 3;
    const float *mean = ctx->picked.in[DT_PICK_MEAN];
    float *a = P(m, "a"), *b = P(m, "b");
    if (fabsf(*a - mean[1]) < 0.0001f && fabsf(*b - mean[2]) < 0.0001f)
        return 0;
    *a = mean[1];
    *b = mean[2];
    const float da = ctx->picked.in[DT_PICK_MAX][1] - ctx->picked.in[DT_PICK_MIN][1];
    const float db = ctx->picked.in[DT_PICK_MAX][2] - ctx->picked.in[DT_PICK_MIN][2];
    *P(m, "size") = CLAMP((da + db) / 128.0, .5, 3.0);
    return 0;
}

static gboolean same_rgb(const float *a, const float *b) {
    return fabsf(a[0] - b[0]) < 0.0001f && fabsf(a[1] - b[1]) < 0.0001f && fabsf(a[2] - b[2]) < 0.0001f;
}

// borders.c color_picker_apply (lines 722-770): the point pickers beside "border color" and
// "frame line color". darktable stops when the picked colour equals either of the two colours.
static int borders_pick(OmToolContext *ctx, const char *target) {
    dt_iop_module_t *m = ctx->module;
    if (!has(m, "color") || !has(m, "frame_color"))
        return 3;
    const float *picked = ctx->picked.in[DT_PICK_MEAN];
    if (same_rgb(P(m, "color"), picked) || same_rgb(P(m, "frame_color"), picked))
        return 0;
    float *color = P(m, target);
    for (int c = 0; c < 3; ++c)
        color[c] = picked[c];
    return 0;
}
static int borders_color(OmToolContext *ctx) {
    return borders_pick(ctx, "color");
}
static int borders_frame_color(OmToolContext *ctx) {
    return borders_pick(ctx, "frame_color");
}

// watermark.c color_picker_apply (lines 1114-1137): the point picker beside "color".
static int watermark_color(OmToolContext *ctx) {
    dt_iop_module_t *m = ctx->module;
    if (!has(m, "color"))
        return 3;
    const float *picked = ctx->picked.in[DT_PICK_MEAN];
    float *color = P(m, "color");
    if (same_rgb(color, picked))
        return 0;
    for (int c = 0; c < 3; ++c)
        color[c] = picked[c];
    return 0;
}

// invert.c color_picker_apply (lines 178-198): the area picker sets the colour of the film
// material from the module input (camera RGB for a raw), all four channels.
static int invert_color(OmToolContext *ctx) {
    dt_iop_module_t *m = ctx->module;
    if (!has(m, "color"))
        return 3;
    float *color = P(m, "color");
    for_four_channels(k) color[k] = ctx->picked.in[DT_PICK_MEAN][k];
    return 0;
}

// relight.c color_picker_apply (lines 224-243): the point-or-area picker marks the picked
// lightness (mean, min, max of L / 100) on the "center" gradient slider. Nothing is written.
static int relight_center(OmToolContext *ctx) {
    const float *mean = ctx->picked.in[DT_PICK_MEAN], *lo = ctx->picked.in[DT_PICK_MIN],
                *hi = ctx->picked.in[DT_PICK_MAX];
    if (hi[0] < 0.0f)
        return 0;
    report_band(ctx, "lightness", fminf(fmaxf(lo[0] / 100.0f, 0.0f), 1.0f),
                fminf(fmaxf(mean[0] / 100.0f, 0.0f), 1.0f), fminf(fmaxf(hi[0] / 100.0f, 0.0f), 1.0f));
    return 0;
}

// colorequal.c _pipe_RGB_to_Ych (lines 2517-2539).
static void colorequal_RGB_to_Ych(OmToolContext *ctx, const dt_aligned_pixel_t RGB, dt_aligned_pixel_t Ych) {
    const dt_iop_order_iccprofile_info_t *const work_profile =
        dt_ioppr_get_pipe_current_profile_info(ctx->module, &ctx->capture->pipe);
    if (!work_profile)
        return;
    dt_aligned_pixel_t XYZ_D50 = {0.0f}, XYZ_D65 = {0.0f};
    dt_ioppr_rgb_matrix_to_xyz(RGB, XYZ_D50, work_profile->matrix_in_transposed, work_profile->lut_in,
                               work_profile->unbounded_coeffs_in, work_profile->lutsize, work_profile->nonlinearlut);
    XYZ_D50_to_D65(XYZ_D50, XYZ_D65);
    XYZ_to_Ych(XYZ_D65, Ych);
    if (Ych[2] < 0.f)
        Ych[2] = DT_2PI_F + Ych[2];
}

// colorequal.c color_picker_apply (lines 2541-2562): the area picker on "white level" sets it
// to log2 of the brightest picked luminance.
static int colorequal_white_level(OmToolContext *ctx) {
    dt_iop_module_t *m = ctx->module;
    if (!has(m, "white_level"))
        return 3;
    dt_aligned_pixel_t max_Ych = {0.0f, 0.0f, 0.0f, 0.0f};
    colorequal_RGB_to_Ych(ctx, ctx->picked.in[DT_PICK_MAX], max_Ych);
    *P(m, "white_level") = log2f(max_Ych[0]);
    return 0;
}

// colorequal.c _draw_color_picker (lines 2250-2302): the point-or-area picker on "node
// placement" (JzCzhz, denoised) marks the picked hue on the graph; nothing is written. The band
// is reported on darktable's graph axis (hue shifted by ANGLE_SHIFT, 20°, line 437).
static float colorequal_hueval(const float hue) {
    const float b = hue - 20.0f / 360.0f;
    return b < 0.0f ? b + 1.0f : b;
}
static int colorequal_hue(OmToolContext *ctx) {
    const float *mean = ctx->picked.in[DT_PICK_MEAN], *lo = ctx->picked.in[DT_PICK_MIN],
                *hi = ctx->picked.in[DT_PICK_MAX];
    if (mean[0] < 0.0001f || mean[1] < 0.0001f)
        return 0;
    float hav = mean[2], hmax = hi[2], hmin = lo[2];
    const float hava = mean[3], hmina = lo[3], hmaxa = hi[3];
    if (hmax - hmin > hmaxa - hmina) {
        hmax = hmaxa < 0.5f ? hmaxa + 0.5f : hmaxa - 0.5f;
        hmin = hmina < 0.5f ? hmina + 0.5f : hmina - 0.5f;
        hav = hava < 0.5f ? hava + 0.5f : hava - 0.5f;
    }
    report_band(ctx, "colorequal_hue", colorequal_hueval(hmin), colorequal_hueval(hav), colorequal_hueval(hmax));
    om_tool_set_gui(ctx, "picked_hue", hav * 360.0f);
    return 0;
}

// colorchecker.c color_picker_apply (lines 1091-1128): the point-or-area picker beside "patch"
// selects the patch whose source colour is nearest to the picked Lab mean (GUI state only).
static int colorchecker_patch(OmToolContext *ctx) {
    dt_iop_module_t *m = ctx->module;
    if (!has(m, "source_L") || !has(m, "num_patches"))
        return 3;
    const int num = *PI(m, "num_patches");
    if (num <= 0)
        return 0;
    const float *L = P(m, "source_L"), *a = P(m, "source_a"), *b = P(m, "source_b");
    const float *picked = ctx->picked.in[DT_PICK_MEAN];
    int best = 0;
    for (int patch = 1; patch < num; patch++)
        if (sqf(picked[0] - L[patch]) + sqf(picked[1] - a[patch]) + sqf(picked[2] - b[patch]) <
            sqf(picked[0] - L[best]) + sqf(picked[1] - a[best]) + sqf(picked[2] - b[best]))
            best = patch;
    om_tool_set_gui(ctx, "@patch", best);
    return 0;
}

// retouch.c color_picker_apply (lines 1589-1622): the point picker beside "fill color" reads the
// module output (DT_COLOR_PICKER_IO). darktable also recolours the selected fill shape; shapes
// are selected on the image, so here only the fill colour for new shapes is set.
static int retouch_fill_color(OmToolContext *ctx) {
    dt_iop_module_t *m = ctx->module;
    if (!has(m, "fill_color") || !ctx->picked.output_valid)
        return 3;
    const float *picked = ctx->picked.out[DT_PICK_MEAN];
    float *fill = P(m, "fill_color");
    if (same_rgb(fill, picked))
        return 0;
    for (int c = 0; c < 3; ++c)
        fill[c] = picked[c];
    return 0;
}

const OmToolSpec om_tools_effects[] = {
    // colorize.c:345 point picker on "hue", the module's colour space (Lab)
    {"colorize", "hue", OM_TOOL_POINT, -1, 0, colorize_hue},
    // splittoning.c:469 point pickers on both hue sliders (RGB)
    {"splittoning", "shadow_hue", OM_TOOL_POINT, -1, 0, splittoning_shadow},
    {"splittoning", "highlight_hue", OM_TOOL_POINT, -1, 0, splittoning_highlight},
    // graduatednd.c:1045 point picker on "hue" (RGB)
    {"graduatednd", "hue", OM_TOOL_POINT, -1, 0, graduatednd_hue},
    // monochrome.c:584 area picker on "highlights" (Lab)
    {"monochrome", "highlights", OM_TOOL_AREA, -1, 0, monochrome_highlights},
    // borders.c:1016, 1030 point pickers beside the two colours (RGB)
    {"borders", "color", OM_TOOL_POINT, -1, 0, borders_color},
    {"borders", "frame_color", OM_TOOL_POINT, -1, 0, borders_frame_color},
    // watermark.c:1460 point picker beside "color"
    {"watermark", "color", OM_TOOL_POINT, -1, 0, watermark_color},
    // invert.c:498 area picker (raw data for a raw image)
    {"invert", "color", OM_TOOL_AREA, -1, 0, invert_color},
    // relight.c:260 point-or-area picker beside the center slider (Lab); marks only 
    // (activating a picker switches the module on, gui/color_picker_proxy.c:174)
    {"relight", "center", OM_TOOL_AREA, -1, 0, relight_center},
    // colorequal.c:3003 hue picker in JzCzhz with denoising, marks only; :3094 white level
    {"colorequal", "hue_shift", OM_TOOL_AREA, IOP_CS_JZCZHZ, OM_TOOL_DENOISE, colorequal_hue},
    {"colorequal", "white_level", OM_TOOL_AREA, -1, 0, colorequal_white_level},
    // colorchecker.c:1559 point-or-area picker beside "patch" (Lab); selects a patch only
    {"colorchecker", "patch", OM_TOOL_AREA, -1, 0, colorchecker_patch},
    // retouch.c:2645 point picker with the module output (DT_COLOR_PICKER_IO)
    {"retouch", "fill_color", OM_TOOL_POINT, -1, OM_TOOL_OUTPUT, retouch_fill_color},
    {NULL, NULL, 0, 0, 0, NULL},
};
