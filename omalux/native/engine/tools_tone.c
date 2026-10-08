// SPDX-License-Identifier: GPL-3.0-or-later
// Pickers and buttons of the base and tone modules, ported from darktable 5.6.1
// (src/iop/*.c, color_picker_apply and the button callbacks) without their GTK code.
// Parameters are reached through darktable's introspection, never through copied structs.
#include "module_tools_internal.h"
#include "common/colorspaces_inline_conversions.h"
#include "common/math.h"
#include "common/rgb_norms.h"
#include "develop/openmp_maths.h"

#define P(module, name) ((float *)(module)->get_p((module)->params, (name)))
#define PI(module, name) ((int *)(module)->get_p((module)->params, (name)))

static gboolean has(dt_iop_module_t *module, const char *name) {
    return module->get_f && module->get_f(name);
}

// ---- white balance --------------------------------------------------------------------------
// temperature.c color_picker_apply (lines 1948-1967): coefficients from the picked grey,
// normalised to green; the preset becomes "from image area".
static int temperature_area(OmToolContext *ctx) {
    dt_iop_module_t *m = ctx->module;
    if (!has(m, "red") || !has(m, "preset"))
        return 3;
    const float *grayrgb = ctx->picked.in[DT_PICK_MEAN];
    float *pcoeffs = P(m, "red"); // red, green, blue, various are consecutive floats
    const float gnormal = grayrgb[1] > 0.001f ? 1.0f / grayrgb[1] : 1.0f;
    for (int c = 0; c < 4; ++c)
        pcoeffs[c] = fmaxf(0.0f, fminf(8.0f, (grayrgb[c] > 0.001f ? 1.0f / grayrgb[c] : 1.0f) / gnormal));
    pcoeffs[1] = 1.0f;
    *PI(m, "preset") = 1; // DT_IOP_TEMP_SPOT (temperature.c:61), as _update_preset sets it
    return 0;
}

// ---- exposure ---------------------------------------------------------------------------------
#define exposure2white(x) exp2f(-(x))
#define white2exposure(x) -dt_log2f(fmaxf(1e-20f, x))

// exposure.c _get_exposure_bias and _get_highlight_bias (lines 569-602).
static float exposure_bias(const dt_develop_t *dev) {
    const float bias = dev->image_storage.exif_exposure_bias;
    return bias != DT_EXIF_TAG_UNINITIALIZED ? CLAMP(bias, -5.0f, 5.0f) : 0.0f;
}
static float highlight_bias(const dt_develop_t *dev) {
    float bias = 0.0f;
    if (dev->image_storage.exif_highlight_preservation > 0.0f)
        bias = dev->image_storage.exif_highlight_preservation;
    return bias != DT_EXIF_TAG_UNINITIALIZED ? CLAMP(bias, -1.0f, 4.0f) : 0.0f;
}

// exposure.c _auto_set_exposure (lines 865-963) with _exposure_set_white (763-780). The
// "area exposure mapping" settings are GUI state: "@area_mode" 0 correction / 1 measure and
// "@lightness" the target in % (darktable keeps it in darkroom/modules/exposure/lightness).
static int exposure_spot(OmToolContext *ctx) {
    dt_iop_module_t *m = ctx->module;
    if (!has(m, "exposure") || !has(m, "black"))
        return 3;
    const float *RGB = ctx->picked.in[DT_PICK_MEAN];
    const dt_iop_order_iccprofile_info_t *const input_profile =
        dt_ioppr_get_pipe_input_profile_info(&ctx->capture->pipe);
    if (!input_profile)
        return 4;
    dt_aligned_pixel_t XYZ, Lab, Lch;
    dot_product(RGB, input_profile->matrix_in, XYZ);
    dt_XYZ_to_Lab(XYZ, Lab);
    Lab[1] = Lab[2] = Lab[3] = 0.f;
    dt_Lab_to_XYZ(Lab, XYZ);
    dt_Lab_2_LCH(Lab, Lch);
    om_tool_set_gui(ctx, "input_lightness", Lch[0]);
    float *exposure = P(m, "exposure"), *black = P(m, "black");
    const gboolean bias = *PI(m, "compensate_exposure_bias") != 0;
    const gboolean hilite = has(m, "compensate_hilite_pres") && *PI(m, "compensate_hilite_pres") != 0;
    if ((int)om_tool_gui(ctx, "@area_mode", 0) == 1) {
        float expo = *exposure;
        if (bias)
            expo -= exposure_bias(ctx->dev);
        if (hilite)
            expo += highlight_bias(ctx->dev);
        const float white = exposure2white(-expo);
        dt_aligned_pixel_t XYZ_out = {0.0f}, Lab_out;
        for (int c = 0; c < 3; c++)
            XYZ_out[c] = XYZ[c] * white;
        dt_XYZ_to_Lab(XYZ_out, Lab_out);
        om_tool_set_gui(ctx, "@lightness", Lab_out[0]);
        return 0;
    }
    dt_aligned_pixel_t Lch_target = {(float)om_tool_gui(ctx, "@lightness", 50.0), 0.f, 0.f, 0.f};
    dt_aligned_pixel_t Lab_target = {0.f}, XYZ_target;
    dt_LCH_2_Lab(Lch_target, Lab_target);
    dt_Lab_to_XYZ(Lab_target, XYZ_target);
    float white = XYZ[1] / XYZ_target[1];
    float expo = -white2exposure(white);
    if (bias)
        expo -= exposure_bias(ctx->dev);
    if (hilite)
        expo += highlight_bias(ctx->dev);
    white = exposure2white(-expo);
    const float value = white2exposure(white);
    if (*exposure == value)
        return 0;
    *exposure = value;
    // _exposure_set_black (exposure.c): the black level stays below the white point.
    if (*black >= white)
        *black = white - 0.01f;
    return 0;
}

// ---- rgb levels ---------------------------------------------------------------------------------
// The channel darktable edits: the notebook page in independent mode, else the first.
static int rgblevels_channel(OmToolContext *ctx) {
    const int autoscale = *PI(ctx->module, "autoscale");
    return autoscale == 1 ? CLAMP((int)om_tool_gui(ctx, "@tab", 0), 0, 2) : 0; // DT_IOP_RGBLEVELS_INDEPENDENT_CHANNELS
}

// rgblevels.c color_picker_apply (lines 774-840): the first picked component, kept between
// its neighbours.
static int rgblevels_pick(OmToolContext *ctx, int which) {
    dt_iop_module_t *m = ctx->module;
    if (!has(m, "levels"))
        return 3;
    float(*levels)[3] = (float(*)[3])P(m, "levels");
    const int channel = rgblevels_channel(ctx);
    const float mean_picked_color = ctx->picked.in[DT_PICK_MEAN][0];
    float *l = levels[channel];
    if (which == 0)
        l[0] = mean_picked_color > l[1] ? l[1] - FLT_EPSILON : mean_picked_color;
    else if (which == 1) {
        if (!(mean_picked_color < l[0] || mean_picked_color > l[2]))
            l[1] = mean_picked_color;
    } else
        l[2] = mean_picked_color < l[1] ? l[1] + FLT_EPSILON : mean_picked_color;
    return 0;
}
static int rgblevels_black(OmToolContext *ctx) {
    return rgblevels_pick(ctx, 0);
}
static int rgblevels_gray(OmToolContext *ctx) {
    return rgblevels_pick(ctx, 1);
}
static int rgblevels_white(OmToolContext *ctx) {
    return rgblevels_pick(ctx, 2);
}

// rgblevels.c _auto_levels (lines 1180-1257) on the module input, over the whole image ("auto")
// or the drawn region ("auto region", _get_selected_area).
static int rgblevels_auto_box(OmToolContext *ctx, gboolean region) {
    dt_iop_module_t *m = ctx->module;
    OmCapture *c = ctx->capture;
    if (!has(m, "levels") || !c || c->dsc.channels != 4)
        return 3;
    float(*levels)[3] = (float(*)[3])P(m, "levels");
    const int autoscale = *PI(m, "autoscale"), preserve_colors = *PI(m, "preserve_colors");
    const int channel = rgblevels_channel(ctx);
    const int width = c->roi.width, height = c->roi.height;
    int y_from = 0, y_to = height - 1, x_from = 0, x_to = width - 1;
    if (region && c->box_valid && c->box[2] - 1 > c->box[0] && c->box[3] - 1 > c->box[1]) {
        x_from = c->box[0];
        y_from = c->box[1];
        x_to = MIN(width - 1, c->box[2] - 1);
        y_to = MIN(height - 1, c->box[3] - 1);
    }
    float max = -FLT_MAX, min = FLT_MAX;
    for (int y = y_from; y <= y_to; y++) {
        const float *const in = c->input + (size_t)4 * width * y;
        for (int x = x_from; x <= x_to; x++) {
            const float *const pixel = in + (size_t)x * 4;
            if (autoscale == 1 || preserve_colors == DT_RGB_NORM_NONE) {
                if (autoscale == 1) {
                    if (pixel[channel] >= 0.f) {
                        max = fmaxf(max, pixel[channel]);
                        min = fminf(min, pixel[channel]);
                    }
                } else
                    for (int k = 0; k < 3; k++)
                        if (pixel[k] >= 0.f) {
                            max = fmaxf(max, pixel[k]);
                            min = fminf(min, pixel[k]);
                        }
            } else {
                const float lum = dt_rgb_norm(pixel, preserve_colors, c->work_profile);
                if (lum >= 0.f) {
                    max = fmaxf(max, lum);
                    min = fminf(min, lum);
                }
            }
        }
    }
    levels[channel][0] = CLAMP(min, 0.f, 1.f);
    levels[channel][2] = CLAMP(max, 0.f, 1.f);
    levels[channel][1] = (levels[channel][2] + levels[channel][0]) / 2.f;
    return 0;
}
static int rgblevels_auto(OmToolContext *ctx) {
    return rgblevels_auto_box(ctx, FALSE);
}
static int rgblevels_region(OmToolContext *ctx) {
    return rgblevels_auto_box(ctx, TRUE);
}

// ---- levels (deprecated) --------------------------------------------------------------------------
// levels.c dt_iop_levels_autoadjust_callback with dt_iop_levels_compute_levels_manual
// (lines 186-208, 978-990) on the module's 256-bin input histogram.
static int levels_auto(OmToolContext *ctx) {
    dt_iop_module_t *m = ctx->module;
    if (!has(m, "levels"))
        return 3;
    uint32_t max[4] = {0};
    uint32_t *histogram = om_tool_histogram(ctx, 256, max);
    if (!histogram)
        return 4;
    float *levels = P(m, "levels");
    for (int k = 0; k <= 4 * 255; k += 4)
        if (histogram[k] > 1) {
            levels[0] = ((float)(k) / (4 * 256));
            break;
        }
    for (int k = 4 * 255; k >= 0; k -= 4)
        if (histogram[k] > 1) {
            levels[2] = ((float)(k) / (4 * 256));
            break;
        }
    levels[1] = levels[0] / 2 + levels[2] / 2;
    dt_free_align(histogram);
    return 0;
}


// ---- filmic rgb ---------------------------------------------------------------------------------
// filmicrgb.c pixel_rgb_norm_power and get_pixel_norm (lines 826-885).
static float filmic_norm(const dt_aligned_pixel_t pixel, int variant,
                         const dt_iop_order_iccprofile_info_t *const work_profile) {
    switch (variant) {
    case 1: // DT_FILMIC_METHOD_MAX_RGB
        return max3f(pixel);
    case 3: { // DT_FILMIC_METHOD_POWER_NORM
        float numerator = 0.0f, denominator = 0.0f;
        for (int c = 0; c < 3; c++) {
            const float value = fabsf(pixel[c]);
            const float RGB_square = value * value;
            numerator += RGB_square * value;
            denominator += RGB_square;
        }
        return numerator / fmaxf(denominator, 1e-12f);
    }
    case 4: // DT_FILMIC_METHOD_EUCLIDEAN_NORM_V1
        return sqrtf(sqf(pixel[0]) + sqf(pixel[1]) + sqf(pixel[2]));
    case 5: // DT_FILMIC_METHOD_EUCLIDEAN_NORM_V2
        return sqrtf(sqf(pixel[0]) + sqf(pixel[1]) + sqf(pixel[2])) * 0.5773502691896258f;
    default: // luminance
        return work_profile ? dt_ioppr_get_rgb_matrix_luminance(pixel, work_profile->matrix_in, work_profile->lut_in,
                                                                work_profile->unbounded_coeffs_in,
                                                                work_profile->lutsize, work_profile->nonlinearlut)
                            : dt_camera_rgb_luminance(pixel);
    }
}

// filmicrgb.c _compute_output_power (lines 2570-2580).
static void filmicrgb_output_power(dt_iop_module_t *m) {
    const dt_introspection_field_t *f = m->get_f("output_power");
    *P(m, "output_power") = CLAMPF(logf(*P(m, "grey_point_target") / 100.0f) /
                                       logf(-*P(m, "black_point_source") /
                                            (*P(m, "white_point_source") - *P(m, "black_point_source"))),
                                   f->Float.Min, f->Float.Max);
}

// filmicrgb.c apply_auto_grey, apply_auto_black, apply_auto_white_point_source and
// apply_autotune (lines 2582-2697). The work profile is the one colorin hands to filmic
// (dt_ioppr_get_iop_work_profile_info).
static int filmicrgb_apply(OmToolContext *ctx, int which) {
    dt_iop_module_t *m = ctx->module;
    if (!has(m, "grey_point_source") || !has(m, "output_power"))
        return 3;
    const dt_iop_order_iccprofile_info_t *const work_profile = dt_ioppr_get_iop_work_profile_info(m, ctx->dev->iop);
    float *grey_src = P(m, "grey_point_source"), *black_src = P(m, "black_point_source"),
          *white_src = P(m, "white_point_source");
    const float security = *P(m, "security_factor");
    const int preserve = *PI(m, "preserve_color");
    const float *picked = ctx->picked.in[DT_PICK_MEAN], *pmin = ctx->picked.in[DT_PICK_MIN],
                *pmax = ctx->picked.in[DT_PICK_MAX];
    if (which == 0) { // grey
        const float grey = filmic_norm(picked, preserve, work_profile) / 2.0f;
        const float prev_grey = *grey_src;
        *grey_src = CLAMP(100.f * grey, 0.001f, 100.0f);
        const float grey_var = log2f(prev_grey / *grey_src);
        *black_src = *black_src - grey_var;
        *white_src = *white_src + grey_var;
    }
    if (which == 3 && *PI(m, "custom_grey")) {
        const float grey = filmic_norm(picked, preserve, work_profile) / 2.0f;
        *grey_src = CLAMP(100.f * grey, 0.001f, 100.0f);
    }
    if (which == 1 || which == 3) { // black
        const float black = filmic_norm(pmin, 1, work_profile);
        float EVmin = CLAMP(log2f(black / (*grey_src / 100.0f)), -16.0f, -1.0f);
        EVmin *= (1.0f + security / 100.0f);
        if (which == 1)
            *black_src = fmaxf(EVmin, -16.0f);
        else {
            const float white = filmic_norm(pmax, 1, work_profile);
            float EVmax = CLAMP(log2f(white / (*grey_src / 100.0f)), 1.0f, 16.0f);
            EVmax *= (1.0f + security / 100.0f);
            *black_src = fmaxf(EVmin, -16.0f);
            *white_src = EVmax;
        }
    }
    if (which == 2) { // white
        const float white = filmic_norm(pmax, 1, work_profile);
        float EVmax = CLAMP(log2f(white / (*grey_src / 100.0f)), 1.0f, 16.0f);
        EVmax *= (1.0f + security / 100.0f);
        *white_src = EVmax;
    }
    filmicrgb_output_power(m);
    return 0;
}
static int filmicrgb_grey(OmToolContext *ctx) {
    return filmicrgb_apply(ctx, 0);
}
static int filmicrgb_black(OmToolContext *ctx) {
    return filmicrgb_apply(ctx, 1);
}
static int filmicrgb_white(OmToolContext *ctx) {
    return filmicrgb_apply(ctx, 2);
}
static int filmicrgb_auto(OmToolContext *ctx) {
    return filmicrgb_apply(ctx, 3);
}

// ---- filmic (deprecated) ------------------------------------------------------------------------
// filmic.c sanitize_latitude, apply_auto_grey, apply_auto_black, apply_auto_white_point_source
// and apply_autotune (lines 593-757); the picker works in Lab, the luminance is XYZ Y.
static int filmic_apply(OmToolContext *ctx, int which) {
    dt_iop_module_t *m = ctx->module;
    if (!has(m, "grey_point_source") || !has(m, "latitude_stops"))
        return 3;
    float *grey_src = P(m, "grey_point_source"), *black_src = P(m, "black_point_source"),
          *white_src = P(m, "white_point_source");
    const float security = *P(m, "security_factor");
    const float noise = powf(2.0f, -16.0f);
    dt_aligned_pixel_t XYZ = {0.0f};
    if (which == 0) {
        dt_Lab_to_XYZ(ctx->picked.in[DT_PICK_MEAN], XYZ);
        const float prev_grey = *grey_src;
        *grey_src = 100.f * XYZ[1];
        const float grey_var = Log2(prev_grey / *grey_src);
        *black_src = *black_src - grey_var;
        *white_src = *white_src + grey_var;
        return 0;
    }
    if (which == 3) {
        dt_Lab_to_XYZ(ctx->picked.in[DT_PICK_MEAN], XYZ);
        *grey_src = 100.f * XYZ[1];
    }
    if (which == 1 || which == 3) {
        dt_Lab_to_XYZ(ctx->picked.in[DT_PICK_MIN], XYZ);
        float EVmin = Log2Thres(XYZ[1] / (*grey_src / 100.0f), noise);
        EVmin *= (1.0f + security / 100.0f);
        *black_src = EVmin;
    }
    if (which == 2 || which == 3) {
        dt_Lab_to_XYZ(ctx->picked.in[DT_PICK_MAX], XYZ);
        float EVmax = Log2Thres(XYZ[1] / (*grey_src / 100.0f), noise);
        EVmax *= (1.0f + security / 100.0f);
        *white_src = EVmax;
    }
    float *latitude = P(m, "latitude_stops");
    if (*latitude > (*white_src - *black_src) * 0.99f)
        *latitude = (*white_src - *black_src) * 0.99f;
    return 0;
}
static int filmic_grey(OmToolContext *ctx) {
    return filmic_apply(ctx, 0);
}
static int filmic_black(OmToolContext *ctx) {
    return filmic_apply(ctx, 1);
}
static int filmic_white(OmToolContext *ctx) {
    return filmic_apply(ctx, 2);
}
static int filmic_auto(OmToolContext *ctx) {
    return filmic_apply(ctx, 3);
}

// ---- unbreak input profile ---------------------------------------------------------------------
// profile_gamma.c apply_auto_grey, apply_auto_black, apply_auto_dynamic_range and
// apply_autotune (lines 322-416).
static int profile_gamma_apply(OmToolContext *ctx, int which) {
    dt_iop_module_t *m = ctx->module;
    if (!has(m, "grey_point") || !has(m, "dynamic_range"))
        return 3;
    float *grey_point = P(m, "grey_point"), *shadows = P(m, "shadows_range"), *range = P(m, "dynamic_range");
    const float security = *P(m, "security_factor");
    const float noise = powf(2.0f, -16.0f);
    if (which == 0 || which == 3)
        *grey_point = 100.f * max3f(ctx->picked.in[DT_PICK_MEAN]);
    if (which == 0)
        return 0;
    float EVmin = *shadows;
    if (which == 1 || which == 3) {
        EVmin = Log2Thres(max3f(ctx->picked.in[DT_PICK_MIN]) / (*grey_point / 100.0f), noise);
        EVmin *= (1.0f + security / 100.0f);
    }
    if (which == 1) {
        *shadows = EVmin;
        return 0;
    }
    float EVmax = Log2Thres(max3f(ctx->picked.in[DT_PICK_MAX]) / (*grey_point / 100.0f), noise);
    EVmax *= (1.0f + security / 100.0f);
    if (which == 3)
        *shadows = EVmin;
    *range = EVmax - EVmin;
    return 0;
}
static int profile_gamma_grey(OmToolContext *ctx) {
    return profile_gamma_apply(ctx, 0);
}
static int profile_gamma_black(OmToolContext *ctx) {
    return profile_gamma_apply(ctx, 1);
}
static int profile_gamma_range(OmToolContext *ctx) {
    return profile_gamma_apply(ctx, 2);
}
static int profile_gamma_auto(OmToolContext *ctx) {
    return profile_gamma_apply(ctx, 3);
}


// ---- agx ----------------------------------------------------------------------------------------
// The parameters agx's curve maths reads, loaded by introspection name (agx.c:96-150).
typedef struct {
    float range_black_relative_ev, range_white_relative_ev, dynamic_range_scaling;
    float curve_pivot_x, curve_pivot_y_linear_output, curve_contrast_around_pivot;
    float curve_linear_ratio_below_pivot, curve_linear_ratio_above_pivot, curve_toe_power, curve_shoulder_power;
    float curve_gamma, curve_target_display_black_ratio, curve_target_display_white_ratio;
    int auto_gamma;
} OmAgx;
static const char *const agx_floats[] = {"range_black_relative_ev", "range_white_relative_ev", "dynamic_range_scaling",
                                         "curve_pivot_x", "curve_pivot_y_linear_output",
                                         "curve_contrast_around_pivot", "curve_linear_ratio_below_pivot",
                                         "curve_linear_ratio_above_pivot", "curve_toe_power", "curve_shoulder_power",
                                         "curve_gamma", "curve_target_display_black_ratio",
                                         "curve_target_display_white_ratio"};
static gboolean agx_load(dt_iop_module_t *m, OmAgx *p) {
    float *out = &p->range_black_relative_ev;
    for (size_t i = 0; i < G_N_ELEMENTS(agx_floats); ++i) {
        if (!has(m, agx_floats[i]))
            return FALSE;
        out[i] = *P(m, agx_floats[i]);
    }
    if (!has(m, "auto_gamma"))
        return FALSE;
    p->auto_gamma = *PI(m, "auto_gamma");
    return TRUE;
}
static void agx_store(dt_iop_module_t *m, const OmAgx *p) {
    const float *in = &p->range_black_relative_ev;
    for (size_t i = 0; i < G_N_ELEMENTS(agx_floats); ++i)
        *P(m, agx_floats[i]) = in[i];
}

// agx.c tone_mapping_params_t (lines 162-205), only the curve part.
typedef struct {
    float black_relative_ev, white_relative_ev, range_in_ev, curve_gamma;
    float pivot_x, pivot_y, target_black, toe_power, toe_transition_x, toe_transition_y, toe_scale;
    gboolean need_convex_toe;
    float toe_fallback_coefficient, toe_fallback_power, slope, intercept;
    float target_white, shoulder_power, shoulder_transition_x, shoulder_transition_y, shoulder_scale;
    gboolean need_concave_shoulder;
    float shoulder_fallback_coefficient, shoulder_fallback_power;
} OmAgxCurve;

static const float agx_epsilon = 1E-6f;
static const float agx_default_gamma = 2.2f;

// agx.c _scale, _sigmoid, _scaled_sigmoid, _fallback_toe, _fallback_shoulder, _apply_curve
// (lines 531-619).
static float agx_scale(const float limit_x, const float limit_y, const float transition_x,
                       const float transition_y, const float slope, const float power) {
    const float projected_rise = slope * fmaxf(agx_epsilon, limit_x - transition_x);
    const float actual_rise = fmaxf(agx_epsilon, limit_y - transition_y);
    const float transformed_projected_rise = powf(projected_rise, -power);
    const float transformed_actual_rise = powf(actual_rise, -power);
    const float base = fmaxf(agx_epsilon, transformed_actual_rise - transformed_projected_rise);
    const float scale_value = powf(base, -1.f / power);
    return fminf(1e9f, scale_value);
}
static float agx_sigmoid(const float x, const float power) {
    return x / powf(1.f + powf(x, power), 1.f / power);
}
static float agx_scaled_sigmoid(const float x, const float scale, const float slope, const float power,
                                const float transition_x, const float transition_y) {
    return scale * agx_sigmoid(slope * (x - transition_x) / scale, power) + transition_y;
}
static float agx_apply_curve(const float x, const OmAgxCurve *params) {
    float result;
    if (x < params->toe_transition_x)
        result = params->need_convex_toe
                     ? (x < 0.f ? params->target_black
                                : params->target_black +
                                      fmaxf(0.f, params->toe_fallback_coefficient * powf(x, params->toe_fallback_power)))
                     : agx_scaled_sigmoid(x, params->toe_scale, params->slope, params->toe_power,
                                          params->toe_transition_x, params->toe_transition_y);
    else if (x <= params->shoulder_transition_x)
        result = params->slope * x + params->intercept;
    else
        result = params->need_concave_shoulder
                     ? (x >= 1.f ? params->target_white
                                 : params->target_white - fmaxf(0.f, params->shoulder_fallback_coefficient *
                                                                         powf(1.f - x, params->shoulder_fallback_power)))
                     : agx_scaled_sigmoid(x, params->shoulder_scale, params->slope, params->shoulder_power,
                                          params->shoulder_transition_x, params->shoulder_transition_y);
    return CLAMPF(result, params->target_black, params->target_white);
}

// agx.c _calculate_pivot_y_at_gamma, _adjust_pivot, _calculate_slope_gamma_compensation and
// _calculate_tone_mapping_params (lines 773-958), without the look part.
static float agx_pivot_y_at_gamma(const OmAgx *p, const float gamma) {
    return powf(CLAMPF(p->curve_pivot_y_linear_output, p->curve_target_display_black_ratio,
                       p->curve_target_display_white_ratio),
                1.f / gamma);
}
static OmAgxCurve agx_curve(const OmAgx *p) {
    OmAgxCurve t;
    t.white_relative_ev = p->range_white_relative_ev;
    t.black_relative_ev = p->range_black_relative_ev;
    t.range_in_ev = t.white_relative_ev - t.black_relative_ev;
    t.pivot_x = CLAMPF(p->curve_pivot_x, agx_epsilon, 1.f - agx_epsilon);
    if (p->auto_gamma)
        t.curve_gamma = t.pivot_x > 0.f && p->curve_pivot_y_linear_output > 0.f
                            ? log2f(p->curve_pivot_y_linear_output) / log2f(t.pivot_x)
                            : p->curve_gamma;
    else
        t.curve_gamma = p->curve_gamma;
    t.pivot_y = agx_pivot_y_at_gamma(p, t.curve_gamma);
    const float range_adjusted_slope = p->curve_contrast_around_pivot * (t.range_in_ev / 16.5f);
    const float pivot_y_at_default_gamma = agx_pivot_y_at_gamma(p, agx_default_gamma);
    const float derivative_at_current_gamma = t.curve_gamma * powf(fmaxf(agx_epsilon, t.pivot_y), t.curve_gamma - 1.0f);
    const float derivative_at_default_gamma =
        agx_default_gamma * powf(fmaxf(agx_epsilon, pivot_y_at_default_gamma), agx_default_gamma - 1.0f);
    t.slope = range_adjusted_slope / (derivative_at_current_gamma / derivative_at_default_gamma);
    // toe
    t.target_black = powf(p->curve_target_display_black_ratio, 1.f / t.curve_gamma);
    t.toe_power = fmaxf(0.01f, p->curve_toe_power);
    const float remaining_y_below_pivot = t.pivot_y - t.target_black;
    const float toe_length_y = remaining_y_below_pivot * p->curve_linear_ratio_below_pivot;
    float dx_linear_below_pivot = toe_length_y / t.slope;
    t.toe_transition_x = fmaxf(agx_epsilon, t.pivot_x - dx_linear_below_pivot);
    dx_linear_below_pivot = t.pivot_x - t.toe_transition_x;
    const float toe_dy_below_pivot = t.slope * dx_linear_below_pivot;
    t.toe_transition_y = t.pivot_y - toe_dy_below_pivot;
    t.toe_scale = -agx_scale(1.f, 1.f - t.target_black, 1.f - t.toe_transition_x, 1.f - t.toe_transition_y, t.slope,
                             t.toe_power);
    const float toe_length_x = t.toe_transition_x;
    const float toe_dy_transition_to_limit = fmaxf(agx_epsilon, t.toe_transition_y - t.target_black);
    t.need_convex_toe = toe_dy_transition_to_limit / toe_length_x > t.slope;
    t.toe_fallback_power = t.slope * toe_length_x / toe_dy_transition_to_limit;
    t.toe_fallback_coefficient = toe_dy_transition_to_limit / powf(toe_length_x, t.toe_fallback_power);
    t.intercept = t.toe_transition_y - (t.slope * t.toe_transition_x);
    // shoulder
    t.target_white = powf(p->curve_target_display_white_ratio, 1.f / t.curve_gamma);
    const float remaining_y_above_pivot = t.target_white - t.pivot_y;
    const float shoulder_length_y = remaining_y_above_pivot * p->curve_linear_ratio_above_pivot;
    float dx_linear_above_pivot = shoulder_length_y / t.slope;
    t.shoulder_transition_x = fminf(1.f - agx_epsilon, t.pivot_x + dx_linear_above_pivot);
    dx_linear_above_pivot = t.shoulder_transition_x - t.pivot_x;
    t.shoulder_transition_y = t.pivot_y + t.slope * dx_linear_above_pivot;
    t.shoulder_power = fmaxf(0.01f, p->curve_shoulder_power);
    t.shoulder_scale = agx_scale(1.f, t.target_white, t.shoulder_transition_x, t.shoulder_transition_y, t.slope,
                                 t.shoulder_power);
    const float shoulder_length_x = 1.f - t.shoulder_transition_x;
    const float shoulder_dy_transition_to_limit = fmaxf(agx_epsilon, t.target_white - t.shoulder_transition_y);
    t.need_concave_shoulder = shoulder_dy_transition_to_limit / shoulder_length_x > t.slope;
    t.shoulder_fallback_power = t.slope * shoulder_length_x / shoulder_dy_transition_to_limit;
    t.shoulder_fallback_coefficient =
        shoulder_dy_transition_to_limit / powf(shoulder_length_x, t.shoulder_fallback_power);
    return t;
}

// agx.c _update_pivot_x (lines 1021-1040): the pivot keeps its exposure when the range moves.
static void agx_update_pivot_x(const float old_black_ev, const float old_white_ev, OmAgx *p) {
    const float new_range = p->range_white_relative_ev - p->range_black_relative_ev;
    const float pivot_ev = old_black_ev + (p->curve_pivot_x * (old_white_ev - old_black_ev));
    const float clamped_pivot_ev = CLAMPF(pivot_ev, p->range_black_relative_ev, p->range_white_relative_ev);
    p->curve_pivot_x = (clamped_pivot_ev - p->range_black_relative_ev) / new_range;
}

// agx.c color_picker_apply (lines 2657-2690) with _apply_auto_black_exposure,
// _apply_auto_white_exposure, _apply_auto_tune_exposure, _apply_auto_pivot_xy and
// _apply_auto_pivot_x (lines 1103-1220). Keeping the pivot on the diagonal only moves the
// displayed gamma slider in darktable, so it writes nothing here.
static int agx_apply(OmToolContext *ctx, int which) {
    dt_iop_module_t *m = ctx->module;
    OmAgx p;
    if (!agx_load(m, &p))
        return 3;
    const float old_black_ev = p.range_black_relative_ev, old_white_ev = p.range_white_relative_ev;
    if (which == 0 || which == 2) {
        const float black_norm = min3f(ctx->picked.in[DT_PICK_MIN]);
        p.range_black_relative_ev =
            CLAMPF(log2f(fmaxf(agx_epsilon, black_norm) / 0.18f) * (1.f + p.dynamic_range_scaling), -20.f, -0.1f);
    }
    if (which == 1 || which == 2) {
        const float white_norm = max3f(ctx->picked.in[DT_PICK_MAX]);
        p.range_white_relative_ev =
            CLAMPF(log2f(fmaxf(agx_epsilon, white_norm) / 0.18f) * (1.f + p.dynamic_range_scaling), 0.1f, 20.f);
    }
    if (which == 3 || which == 4) {
        const dt_iop_order_iccprofile_info_t *profile = dt_ioppr_get_pipe_work_profile_info(&ctx->capture->pipe);
        if (!profile)
            return 4;
        dt_aligned_pixel_t picked;
        copy_pixel(picked, ctx->picked.in[DT_PICK_MEAN]);
        const float luminance = dt_ioppr_get_rgb_matrix_luminance(picked, profile->matrix_in, profile->lut_in,
                                                                   profile->unbounded_coeffs_in, profile->lutsize,
                                                                   profile->nonlinearlut);
        const float picked_ev = CLAMPF(log2f(fmaxf(agx_epsilon, luminance) / 0.18f), p.range_black_relative_ev,
                                       p.range_white_relative_ev);
        const float range = p.range_white_relative_ev - p.range_black_relative_ev;
        const float picked_pivot_x = (picked_ev - p.range_black_relative_ev) / range;
        if (which == 4) {
            const OmAgxCurve curve = agx_curve(&p);
            const float target_y = agx_apply_curve(picked_pivot_x, &curve);
            p.curve_pivot_y_linear_output = powf(target_y, p.curve_gamma);
        }
        p.curve_pivot_x = picked_pivot_x;
    }
    agx_update_pivot_x(old_black_ev, old_white_ev, &p);
    agx_store(m, &p);
    return 0;
}
static int agx_black(OmToolContext *ctx) {
    return agx_apply(ctx, 0);
}
static int agx_white(OmToolContext *ctx) {
    return agx_apply(ctx, 1);
}
static int agx_range(OmToolContext *ctx) {
    return agx_apply(ctx, 2);
}
static int agx_pivot_x(OmToolContext *ctx) {
    return agx_apply(ctx, 3);
}
static int agx_pivot_xy(OmToolContext *ctx) {
    return agx_apply(ctx, 4);
}

// develop.c dt_dev_exposure_get_effective_exposure: the exposure darktable's exposure module
// commits (exposure.c commit_params, lines 605-627) for the enabled instance, preferring one
// without a mask.
static float effective_exposure(OmToolContext *ctx) {
    dt_iop_module_t *chosen = NULL;
    for (GList *it = ctx->dev->iop; it; it = g_list_next(it)) {
        dt_iop_module_t *m = it->data;
        if (!dt_iop_module_is(m, "exposure") || !m->enabled)
            continue;
        const gboolean masked = m->blend_params && m->blend_params->mask_mode != DEVELOP_MASK_DISABLED &&
                                m->blend_params->mask_mode != DEVELOP_MASK_ENABLED;
        if (!chosen || (!masked && chosen->blend_params->mask_mode != DEVELOP_MASK_DISABLED &&
                        chosen->blend_params->mask_mode != DEVELOP_MASK_ENABLED))
            chosen = m;
    }
    if (!chosen)
        return 0.0f;
    float exposure = *P(chosen, "exposure");
    if (*PI(chosen, "compensate_exposure_bias"))
        exposure -= exposure_bias(ctx->dev);
    if (has(chosen, "compensate_hilite_pres") && *PI(chosen, "compensate_hilite_pres"))
        exposure += highlight_bias(ctx->dev);
    return exposure;
}

// agx.c _read_exposure_params_callback with _adjust_relative_exposure_from_exposure_params
// (lines 1042-1055, 1158-1168).
static int agx_read_exposure(OmToolContext *ctx) {
    OmAgx p;
    if (!agx_load(ctx->module, &p))
        return 3;
    const float old_black_ev = p.range_black_relative_ev, old_white_ev = p.range_white_relative_ev;
    const float exposure = effective_exposure(ctx);
    p.range_black_relative_ev = CLAMPF((-8.f + 0.5f * exposure) * (1.f + p.dynamic_range_scaling), -20.f, -0.1f);
    p.range_white_relative_ev = CLAMPF((4.f + 0.8 * exposure) * (1.f + p.dynamic_range_scaling), 0.1f, 20.f);
    agx_update_pivot_x(old_black_ev, old_white_ev, &p);
    agx_store(ctx->module, &p);
    return 0;
}

// agx.c _set_blenderlike_primaries, _set_smooth_primaries and _set_unmodified_primaries
// (lines 317-400), offered by "reset primaries" as a menu.
static void agx_set(dt_iop_module_t *m, const char *name, float value) {
    *P(m, name) = value;
}
static int agx_primaries(OmToolContext *ctx) {
    dt_iop_module_t *m = ctx->module;
    if (!has(m, "red_inset") || !has(m, "base_primaries"))
        return 3;
    const int preset = (int)om_tool_gui(ctx, "preset", -1);
    if (preset < 0 || preset > 2)
        return 5;
    *PI(m, "disable_primaries_adjustments") = FALSE;
    *PI(m, "completely_reverse_primaries") = FALSE;
    static const char *const in[] = {"red_inset", "green_inset", "blue_inset"};
    static const char *const rot[] = {"red_rotation", "green_rotation", "blue_rotation"};
    static const char *const out[] = {"red_outset", "green_outset", "blue_outset"};
    static const char *const unrot[] = {"red_unrotation", "green_unrotation", "blue_unrotation"};
    if (preset == 0) { // blender-like
        static const float inset[] = {0.29462451f, 0.25861925f, 0.14641371f};
        static const float rotation[] = {0.03540329f, -0.02108586f, -0.06305724f};
        static const float outset[] = {0.290776401758f, 0.263155400753f, 0.045810721815f};
        *PI(m, "base_primaries") = 2; // DT_AGX_REC2020
        for (int i = 0; i < 3; ++i) {
            agx_set(m, in[i], inset[i]);
            agx_set(m, rot[i], rotation[i]);
            agx_set(m, out[i], outset[i]);
            agx_set(m, unrot[i], rotation[i]);
        }
        agx_set(m, "master_outset_ratio", 1.f);
        agx_set(m, "master_unrotation_ratio", 0.f);
    } else if (preset == 1) { // smooth
        static const float inset[] = {0.1f, 0.1f, 0.15f};
        const float rotation[] = {deg2radf(2.f), deg2radf(-1.f), deg2radf(-3.f)};
        *PI(m, "base_primaries") = 1; // DT_AGX_WORK_PROFILE
        for (int i = 0; i < 3; ++i) {
            agx_set(m, in[i], inset[i]);
            agx_set(m, rot[i], rotation[i]);
            agx_set(m, out[i], inset[i]);
            agx_set(m, unrot[i], rotation[i]);
        }
        agx_set(m, "master_outset_ratio", 0.f);
        agx_set(m, "master_unrotation_ratio", 1.f);
    } else { // unmodified
        *PI(m, "base_primaries") = 2;
        for (int i = 0; i < 3; ++i) {
            agx_set(m, in[i], 0.f);
            agx_set(m, rot[i], 0.f);
            agx_set(m, out[i], 0.f);
            agx_set(m, unrot[i], 0.f);
        }
        agx_set(m, "master_outset_ratio", 1.f);
        agx_set(m, "master_unrotation_ratio", 1.f);
    }
    return 0;
}

// agx.c _set_post_curve_primaries_from_pre_callback (lines 2123-2140).
static int agx_set_from_above(OmToolContext *ctx) {
    dt_iop_module_t *m = ctx->module;
    if (!has(m, "red_outset"))
        return 3;
    agx_set(m, "master_outset_ratio", 1.0f);
    agx_set(m, "master_unrotation_ratio", 1.0f);
    agx_set(m, "red_outset", *P(m, "red_inset"));
    agx_set(m, "green_outset", *P(m, "green_inset"));
    agx_set(m, "blue_outset", *P(m, "blue_inset"));
    agx_set(m, "red_unrotation", *P(m, "red_rotation"));
    agx_set(m, "green_unrotation", *P(m, "green_rotation"));
    agx_set(m, "blue_unrotation", *P(m, "blue_rotation"));
    return 0;
}


// ---- negadoctor ---------------------------------------------------------------------------------
#define NEGADOCTOR_THRESHOLD 2.3283064365386963e-10f // negadoctor.c:56, -32 EV

// negadoctor.c apply_auto_Dmin, apply_auto_Dmax, apply_auto_offset, apply_auto_WB_low,
// apply_auto_WB_high, apply_auto_black and apply_auto_exposure (lines 633-810).
static int negadoctor_apply(OmToolContext *ctx, int which) {
    dt_iop_module_t *m = ctx->module;
    if (!has(m, "Dmin") || !has(m, "wb_high") || !has(m, "D_max"))
        return 3;
    float *Dmin = P(m, "Dmin"), *wb_high = P(m, "wb_high"), *wb_low = P(m, "wb_low");
    float *D_max = P(m, "D_max"), *offset = P(m, "offset"), *black = P(m, "black"), *exposure = P(m, "exposure");
    const float *picked = ctx->picked.in[DT_PICK_MEAN], *pmin = ctx->picked.in[DT_PICK_MIN],
                *pmax = ctx->picked.in[DT_PICK_MAX];
    dt_aligned_pixel_t RGB = {0.f};
    switch (which) {
    case 0: // film material: Dmin
        for (int k = 0; k < 4; k++)
            Dmin[k] = picked[k];
        break;
    case 1: // D max
        for (int c = 0; c < 3; c++)
            RGB[c] = log10f(Dmin[c] / fmaxf(pmin[c], NEGADOCTOR_THRESHOLD));
        *D_max = v_maxf(RGB);
        break;
    case 2: // scan exposure bias
        for (int c = 0; c < 3; c++)
            RGB[c] = log10f(Dmin[c] / fmaxf(pmax[c], NEGADOCTOR_THRESHOLD)) / *D_max;
        *offset = v_minf(RGB);
        break;
    case 3: { // shadows colour cast
        for (int c = 0; c < 3; c++)
            RGB[c] = log10f(Dmin[c] / fmaxf(picked[c], NEGADOCTOR_THRESHOLD)) / *D_max;
        const float RGB_v_min = v_minf(RGB);
        for (int c = 0; c < 3; c++)
            wb_low[c] = RGB_v_min / RGB[c];
        wb_low[3] = 1.0f;
        break;
    }
    case 4: { // illuminant (highlights white balance)
        for (int c = 0; c < 3; c++)
            RGB[c] = fabsf(-1.0f / (*offset * wb_low[c] -
                                    log10f(Dmin[c] / fmaxf(picked[c], NEGADOCTOR_THRESHOLD)) / *D_max));
        const float RGB_v_min = v_minf(RGB);
        for (int c = 0; c < 3; c++)
            wb_high[c] = RGB[c] / RGB_v_min;
        wb_high[3] = 1.0f;
        break;
    }
    case 5: // paper black
        for (int c = 0; c < 3; c++) {
            RGB[c] = -log10f(Dmin[c] / fmaxf(pmax[c], NEGADOCTOR_THRESHOLD));
            RGB[c] *= wb_high[c] / *D_max;
            RGB[c] += wb_low[c] * *offset * wb_high[c];
            RGB[c] = 0.1f - (1.0f - fast_exp10f(RGB[c]));
        }
        *black = v_maxf(RGB);
        break;
    case 6: // print exposure adjustment
        for (int c = 0; c < 3; c++) {
            RGB[c] = -log10f(Dmin[c] / fmaxf(pmin[c], NEGADOCTOR_THRESHOLD));
            RGB[c] *= wb_high[c] / *D_max;
            RGB[c] += wb_low[c] * *offset;
            RGB[c] = 0.96f / (1.0f - fast_exp10f(RGB[c]) + *black);
        }
        *exposure = v_minf(RGB);
        break;
    }
    return 0;
}
static int negadoctor_dmin(OmToolContext *ctx) {
    return negadoctor_apply(ctx, 0);
}
static int negadoctor_dmax(OmToolContext *ctx) {
    return negadoctor_apply(ctx, 1);
}
static int negadoctor_offset(OmToolContext *ctx) {
    return negadoctor_apply(ctx, 2);
}
static int negadoctor_wb_low(OmToolContext *ctx) {
    return negadoctor_apply(ctx, 3);
}
static int negadoctor_wb_high(OmToolContext *ctx) {
    return negadoctor_apply(ctx, 4);
}
static int negadoctor_black(OmToolContext *ctx) {
    return negadoctor_apply(ctx, 5);
}
static int negadoctor_exposure(OmToolContext *ctx) {
    return negadoctor_apply(ctx, 6);
}

const OmToolSpec om_tools_tone[] = {
    // temperature.c:2088, dt_color_picker_new_with_cst(..., IOP_CS_NONE)
    {"temperature", "from_image_area", OM_TOOL_AREA, IOP_CS_NONE, 0, temperature_area},
    // exposure.c:1211, the exposure slider's area picker
    {"exposure", "spot", OM_TOOL_AREA, -1, 0, exposure_spot},
    // rgblevels.c:1069 point pickers, :1081 auto, :1086 region
    {"rgblevels", "black", OM_TOOL_POINT, -1, 0, rgblevels_black},
    {"rgblevels", "gray", OM_TOOL_POINT, -1, 0, rgblevels_gray},
    {"rgblevels", "white", OM_TOOL_POINT, -1, 0, rgblevels_white},
    {"rgblevels", "auto", OM_TOOL_BUTTON, -1, OM_TOOL_CAPTURE, rgblevels_auto},
    {"rgblevels", "auto_region", OM_TOOL_AREA, -1, 0, rgblevels_region},
    {"rgblevels", "histogram", OM_TOOL_HISTOGRAM, -1, 0, NULL},
    // levels.c:645 auto, histogram behind the graph
    {"levels", "auto", OM_TOOL_BUTTON, -1, OM_TOOL_CAPTURE, levels_auto},
    {"levels", "histogram", OM_TOOL_HISTOGRAM, -1, 0, NULL},
    // filmicrgb.c:4370-4412, area pickers with denoising on the three sliders and auto tune
    {"filmicrgb", "grey_point_source", OM_TOOL_AREA, -1, OM_TOOL_DENOISE, filmicrgb_grey},
    {"filmicrgb", "black_point_source", OM_TOOL_AREA, -1, OM_TOOL_DENOISE, filmicrgb_black},
    {"filmicrgb", "white_point_source", OM_TOOL_AREA, -1, OM_TOOL_DENOISE, filmicrgb_white},
    {"filmicrgb", "auto_tune_levels", OM_TOOL_AREA, -1, OM_TOOL_DENOISE, filmicrgb_auto},
    // filmic.c:1474-1516
    {"filmic", "grey_point_source", OM_TOOL_AREA, -1, OM_TOOL_DENOISE, filmic_grey},
    {"filmic", "black_point_source", OM_TOOL_AREA, -1, OM_TOOL_DENOISE, filmic_black},
    {"filmic", "white_point_source", OM_TOOL_AREA, -1, OM_TOOL_DENOISE, filmic_white},
    {"filmic", "auto_tune_levels", OM_TOOL_AREA, -1, OM_TOOL_DENOISE, filmic_auto},
    // profile_gamma.c:617-639
    {"profile_gamma", "grey_point", OM_TOOL_AREA, -1, 0, profile_gamma_grey},
    {"profile_gamma", "shadows_range", OM_TOOL_AREA, -1, 0, profile_gamma_black},
    {"profile_gamma", "dynamic_range", OM_TOOL_AREA, -1, 0, profile_gamma_range},
    {"profile_gamma", "auto_tune_levels", OM_TOOL_AREA, -1, 0, profile_gamma_auto},
    // agx.c:1823-2072 area pickers with denoising; :2078 read exposure; :2280 reset primaries
    // (menu: gui "preset" 0 blender-like, 1 smooth, 2 unmodified); :2349 set from above
    {"agx", "range_black_relative_ev", OM_TOOL_AREA, -1, OM_TOOL_DENOISE, agx_black},
    {"agx", "range_white_relative_ev", OM_TOOL_AREA, -1, OM_TOOL_DENOISE, agx_white},
    {"agx", "auto_tune_levels", OM_TOOL_AREA, -1, OM_TOOL_DENOISE, agx_range},
    {"agx", "curve_pivot_x", OM_TOOL_AREA, -1, OM_TOOL_DENOISE, agx_pivot_x},
    {"agx", "curve_pivot_y_linear_output", OM_TOOL_AREA, -1, OM_TOOL_DENOISE, agx_pivot_xy},
    {"agx", "read_exposure", OM_TOOL_BUTTON, -1, 0, agx_read_exposure},
    {"agx", "reset_primaries", OM_TOOL_BUTTON, -1, 0, agx_primaries},
    {"agx", "set_from_above", OM_TOOL_BUTTON, -1, 0, agx_set_from_above},
    // negadoctor.c:848-1000, area pickers
    {"negadoctor", "film_material", OM_TOOL_AREA, -1, 0, negadoctor_dmin},
    {"negadoctor", "D_max", OM_TOOL_AREA, -1, 0, negadoctor_dmax},
    {"negadoctor", "offset", OM_TOOL_AREA, -1, 0, negadoctor_offset},
    {"negadoctor", "shadows", OM_TOOL_AREA, -1, 0, negadoctor_wb_low},
    {"negadoctor", "illuminant", OM_TOOL_AREA, -1, 0, negadoctor_wb_high},
    {"negadoctor", "black", OM_TOOL_AREA, -1, 0, negadoctor_black},
    {"negadoctor", "exposure", OM_TOOL_AREA, -1, 0, negadoctor_exposure},
    {NULL, NULL, 0, 0, 0, NULL},
};
