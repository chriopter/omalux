// SPDX-License-Identifier: GPL-3.0-or-later
// Pickers and buttons of the colour modules, ported from darktable 5.6.1 (src/iop/*.c,
// color_picker_apply and button callbacks) without their GTK code. Parameters are reached
// through darktable's introspection, never through copied structs.
#include "module_tools_internal.h"
#include "common/colorspaces_inline_conversions.h"
#include "common/math.h"
#include "common/chromatic_adaptation.h"
#include "common/dttypes.h"
// illuminants.h defines a non-static pair_min; keep this copy private to the file.
#define pair_min om_tools_color_pair_min
#include "common/illuminants.h"
#undef pair_min
#include "common/image.h"

#define P(module, name) ((float *)(module)->get_p((module)->params, (name)))
#define PI(module, name) ((int *)(module)->get_p((module)->params, (name)))

static gboolean has(dt_iop_module_t *module, const char *name) {
    return module->get_f && module->get_f(name);
}

// ---- color balance --------------------------------------------------------------------------------
// The optimisers remember the patches picked before (colorbalance.c luma_patches and
// color_patches, flags INVALID 0 / USER_SELECTED 1 / AUTO_SELECTED 2). darktable keeps them in
// its GUI data; here they travel with the request as GUI-only values and come back with the
// result: "@luma_<lift|gamma|gain>", "@luma_<...>_flag", "@color_<...>_<0..2>", "@color_<...>_flag".
enum { CB_FACTOR = 0, CB_RED, CB_GREEN, CB_BLUE };
static const char *const cb_levels[] = {"lift", "gamma", "gain"};

static float cb_gui(OmToolContext *ctx, const char *kind, int level, const char *suffix) {
    char name[48];
    g_snprintf(name, sizeof(name), "@%s_%s%s", kind, cb_levels[level], suffix);
    return (float)om_tool_gui(ctx, name, 0.0);
}
static void cb_set_gui(OmToolContext *ctx, const char *kind, int level, const char *suffix, float value) {
    char name[48];
    g_snprintf(name, sizeof(name), "@%s_%s%s", kind, cb_levels[level], suffix);
    om_tool_set_gui(ctx, name, value);
}
static void cb_color_patch(OmToolContext *ctx, int level, const float RGB[3], int flag) {
    char suffix[8];
    for (int c = 0; c < 3; ++c) {
        g_snprintf(suffix, sizeof(suffix), "_%d", c);
        cb_set_gui(ctx, "color", level, suffix, RGB[c]);
    }
    cb_set_gui(ctx, "color", level, "_flag", flag);
}

// colorbalance.c CDL (line 329).
static inline float cb_CDL(float x, float slope, float offset, float power) {
    return powf(MAX(slope * x + offset, 0.0f), power);
}

// colorbalance.c apply_lift_neutralize, apply_gamma_neutralize, apply_gain_neutralize
// (lines 984-1102): the hue pickers neutralise the picked colour.
static int colorbalance_neutralize(OmToolContext *ctx, int level) {
    dt_iop_module_t *m = ctx->module;
    if (!has(m, "lift") || !has(m, "gain"))
        return 3;
    float *lift = P(m, "lift"), *gamma = P(m, "gamma"), *gain = P(m, "gain");
    dt_aligned_pixel_t XYZ = {0.0f}, RGB = {0.0f};
    dt_Lab_to_XYZ(ctx->picked.in[DT_PICK_MEAN], XYZ);
    dt_XYZ_to_prophotorgb(XYZ, RGB);
    cb_color_patch(ctx, level, RGB, 1);
    // darktable computes the CDL-corrected values here and then overwrites them again
    for (int c = 0; c < 3; ++c)
        RGB[c] = cb_CDL(RGB[c], gain[CB_FACTOR], lift[CB_FACTOR] - 1.0f, 2.0f - gamma[CB_FACTOR]);
    dt_XYZ_to_prophotorgb(XYZ, RGB);
    if (level == 0) {
        for (int c = 0; c < 3; ++c)
            RGB[c] = powf(XYZ[1], 1.0f / (2.0f - gamma[c + 1])) - RGB[c] * gain[c + 1];
        for (int c = 0; c < 3; ++c)
            lift[c + 1] = RGB[c] + 1.0f;
    } else if (level == 1) {
        for (int c = 0; c < 3; ++c)
            RGB[c] = logf(XYZ[1]) / logf(RGB[c] * gain[c + 1] + lift[c + 1] - 1.0f);
        for (int c = 0; c < 3; ++c)
            gamma[c + 1] = CLAMP(2.0 - RGB[c], 0.0001f, 2.0f);
    } else {
        for (int c = 0; c < 3; ++c)
            RGB[c] = (powf(XYZ[1], 1.0f / (2.0f - gamma[c + 1])) - lift[c + 1] + 1.0f) / MAX(RGB[c], 0.000001f);
        for (int c = 0; c < 3; ++c)
            gain[c + 1] = RGB[c];
    }
    return 0;
}
static int colorbalance_hue_lift(OmToolContext *ctx) {
    return colorbalance_neutralize(ctx, 0);
}
static int colorbalance_hue_gamma(OmToolContext *ctx) {
    return colorbalance_neutralize(ctx, 1);
}
static int colorbalance_hue_gain(OmToolContext *ctx) {
    return colorbalance_neutralize(ctx, 2);
}

// colorbalance.c apply_lift_auto, apply_gamma_auto, apply_gain_auto (lines 1104-1176): the
// factor pickers set luminance from the darkest, average or brightest picked value.
static int colorbalance_factor(OmToolContext *ctx, int level) {
    dt_iop_module_t *m = ctx->module;
    if (!has(m, "lift"))
        return 3;
    float *lift = P(m, "lift"), *gamma = P(m, "gamma"), *gain = P(m, "gain");
    dt_aligned_pixel_t XYZ = {0.0f};
    dt_Lab_to_XYZ(ctx->picked.in[level == 0 ? DT_PICK_MIN : level == 1 ? DT_PICK_MEAN : DT_PICK_MAX], XYZ);
    cb_set_gui(ctx, "luma", level, "", XYZ[1]);
    cb_set_gui(ctx, "luma", level, "_flag", 1);
    if (level == 0)
        lift[CB_FACTOR] = -gain[CB_FACTOR] * XYZ[1] + 1.0f;
    else if (level == 1)
        gamma[CB_FACTOR] =
            2.0f - logf(0.1842f) / logf(MAX(gain[CB_FACTOR] * XYZ[1] + lift[CB_FACTOR] - 1.0f, 0.000001f));
    else
        gain[CB_FACTOR] = lift[CB_FACTOR] / (XYZ[1]);
    return 0;
}
static int colorbalance_lift_factor(OmToolContext *ctx) {
    return colorbalance_factor(ctx, 0);
}
static int colorbalance_gamma_factor(OmToolContext *ctx) {
    return colorbalance_factor(ctx, 1);
}
static int colorbalance_gain_factor(OmToolContext *ctx) {
    return colorbalance_factor(ctx, 2);
}

// colorbalance.c apply_autogrey (lines 945-982): the contrast fulcrum from the picked grey.
static int colorbalance_grey(OmToolContext *ctx) {
    dt_iop_module_t *m = ctx->module;
    if (!has(m, "grey"))
        return 3;
    const float *l = P(m, "lift"), *g = P(m, "gamma"), *k = P(m, "gain");
    dt_aligned_pixel_t XYZ = {0.0f}, rgb = {0.0f};
    dt_Lab_to_XYZ(ctx->picked.in[DT_PICK_MEAN], XYZ);
    dt_XYZ_to_prophotorgb(XYZ, rgb);
    const dt_aligned_pixel_t lift = {(l[CB_RED] + l[CB_FACTOR] - 2.0f), (l[CB_GREEN] + l[CB_FACTOR] - 2.0f),
                                     (l[CB_BLUE] + l[CB_FACTOR] - 2.0f), 0.f};
    const dt_aligned_pixel_t gamma = {2.0f - g[CB_RED] * g[CB_FACTOR], 2.0f - g[CB_GREEN] * g[CB_FACTOR],
                                      2.0f - g[CB_BLUE] * g[CB_FACTOR], 1.f};
    const dt_aligned_pixel_t gain = {k[CB_RED] * k[CB_FACTOR], k[CB_GREEN] * k[CB_FACTOR], k[CB_BLUE] * k[CB_FACTOR],
                                     1.f};
    // colorbalance.c _apply_CDL (line 338)
    dt_aligned_pixel_t res;
    for_each_channel(c) res[c] = gain[c] * rgb[c] + lift[c];
    for_each_channel(c) res[c] = powf(MAX(res[c], 0.0f), gamma[c]);
    for_each_channel(c) rgb[c] = CLAMP(res[c], 0.0f, 1.0f);
    dt_prophotorgb_to_XYZ(rgb, XYZ);
    *P(m, "grey") = XYZ[1] * 100.0f;
    return 0;
}

// colorbalance.c apply_autoluma (lines 1300-1342).
static int colorbalance_auto_luma(OmToolContext *ctx) {
    dt_iop_module_t *m = ctx->module;
    if (!has(m, "lift"))
        return 3;
    float *lift = P(m, "lift"), *gamma = P(m, "gamma"), *gain = P(m, "gain");
    float patches[3];
    const int stats[3] = {DT_PICK_MIN, DT_PICK_MEAN, DT_PICK_MAX};
    for (int level = 0; level < 3; ++level) {
        int flag = (int)cb_gui(ctx, "luma", level, "_flag");
        patches[level] = cb_gui(ctx, "luma", level, "");
        if (flag == 0) {
            dt_aligned_pixel_t XYZ = {0.0f};
            dt_Lab_to_XYZ(ctx->picked.in[stats[level]], XYZ);
            patches[level] = XYZ[1];
            flag = 2;
        }
        cb_set_gui(ctx, "luma", level, "", patches[level]);
        cb_set_gui(ctx, "luma", level, "_flag", flag);
    }
    for (int runs = 0; runs < 100; ++runs) {
        gain[CB_FACTOR] = CLAMP(lift[CB_FACTOR] / patches[2], 0.0f, 2.0f);
        lift[CB_FACTOR] = CLAMP(-gain[CB_FACTOR] * patches[0] + 1.0f, 0.0f, 2.0f);
        gamma[CB_FACTOR] = CLAMP(
            2.0f - logf(0.1842f) / logf(MAX(gain[CB_FACTOR] * patches[1] + lift[CB_FACTOR] - 1.0f, 0.000001f)),
            0.0f, 2.0f);
    }
    return 0;
}

// colorbalance.c apply_autocolor (lines 1178-1298).
static int colorbalance_auto_color(OmToolContext *ctx) {
    dt_iop_module_t *m = ctx->module;
    if (!has(m, "lift"))
        return 3;
    float *lift = P(m, "lift"), *gamma = P(m, "gamma"), *gain = P(m, "gain");
    float patches[3][3];
    dt_aligned_pixel_t XYZ = {0.0f}, RGB = {0.0f};
    dt_Lab_to_XYZ(ctx->picked.in[DT_PICK_MEAN], XYZ);
    dt_XYZ_to_prophotorgb(XYZ, RGB);
    for (int level = 0; level < 3; ++level) {
        int flag = (int)cb_gui(ctx, "color", level, "_flag");
        char suffix[8];
        for (int c = 0; c < 3; ++c) {
            g_snprintf(suffix, sizeof(suffix), "_%d", c);
            patches[level][c] = flag ? cb_gui(ctx, "color", level, suffix) : RGB[c];
        }
        if (!flag)
            flag = 2;
        cb_color_patch(ctx, level, patches[level], flag);
    }
    dt_aligned_pixel_t samples[3] = {{0.f}, {0.f}, {0.f}};
    for (int level = 0; level < 3; ++level)
        for (int c = 0; c < 3; ++c)
            samples[level][c] =
                cb_CDL(patches[level][c], gain[CB_FACTOR], lift[CB_FACTOR] - 1.0f, 2.0f - gamma[CB_FACTOR]);
    dt_aligned_pixel_t greys = {0.0f};
    for (int level = 0; level < 3; ++level) {
        dt_prophotorgb_to_XYZ(samples[level], XYZ);
        greys[level] = XYZ[1];
    }
    dt_aligned_pixel_t RGB_lift = {lift[CB_RED] - 1.0f, lift[CB_GREEN] - 1.0f, lift[CB_BLUE] - 1.0f, 0.f};
    dt_aligned_pixel_t RGB_gamma = {gamma[CB_RED], gamma[CB_GREEN], gamma[CB_BLUE], 0.f};
    dt_aligned_pixel_t RGB_gain = {gain[CB_RED], gain[CB_GREEN], gain[CB_BLUE], 0.f};
    for (int runs = 0; runs < 1000; ++runs) {
        for (int c = 0; c < 3; ++c)
            RGB_gain[c] = CLAMP((powf(greys[2], 1.0f / (2.0f - RGB_gamma[c])) - RGB_lift[c]) / MAX(samples[2][c], 0.000001f),
                                0.75f, 1.25f);
        for (int c = 0; c < 3; ++c)
            RGB_lift[c] =
                CLAMP(powf(greys[0], 1.0f / (2.0f - RGB_gamma[c])) - samples[0][c] * RGB_gain[c], -0.025f, 0.025f);
        for (int c = 0; c < 3; ++c)
            RGB_gamma[c] = 2.0f - CLAMP(logf(MAX(greys[1], 0.000001f)) /
                                            logf(MAX(RGB_gain[c] * samples[1][c] + RGB_lift[c], 0.000001f)),
                                        0.75f, 1.25f);
    }
    for (int c = 0; c < 3; ++c) {
        lift[c + 1] = RGB_lift[c] + 1.0f;
        gamma[c + 1] = RGB_gamma[c];
        gain[c + 1] = RGB_gain[c];
    }
    return 0;
}


// ---- color calibration --------------------------------------------------------------------------
// channelmixerrgb.c _check_if_close_to_daylight (lines 1216-1283).
static void cat_close_to_daylight(const float x, const float y, float *temperature, dt_adaptation_t *adaptation) {
    float t = xy_to_CCT(x, y);
    if (t < 3000.f && t > 1667.f)
        t = CCT_reverse_lookup(x, y);
    if (temperature)
        *temperature = t;
    if (adaptation)
        *adaptation = DT_ADAPTATION_CAT16;
}

// channelmixerrgb.c _dev_is_D65_chroma and _get_white_balance_coeff (lines 593-644).
static void cat_white_balance_coeff(const dt_develop_t *dev, dt_aligned_pixel_t custom_wb) {
    const dt_dev_chroma_t *chr = &dev->chroma;
    for_four_channels(k) custom_wb[k] = 1.0f;
    if (!dt_image_is_matrix_correction_supported(&dev->image_storage))
        return;
    const gboolean d65 = chr->late_correction ? dt_dev_equal_chroma(chr->wb_coeffs, chr->as_shot)
                                              : dt_dev_equal_chroma(chr->wb_coeffs, chr->D65coeffs);
    if (d65)
        return;
    const gboolean valid_chroma = chr->D65coeffs[0] > 0.0 && chr->D65coeffs[1] > 0.0 && chr->D65coeffs[2] > 0.0;
    const gboolean changed_chroma = chr->wb_coeffs[0] > 1.0f || chr->wb_coeffs[1] > 1.0f || chr->wb_coeffs[2] > 1.0f;
    if (valid_chroma && changed_chroma)
        for_four_channels(k) custom_wb[k] = (float)chr->D65coeffs[k] / chr->wb_coeffs[k];
}

// channelmixerrgb.c color_picker_apply with _auto_set_illuminant (lines 4161-4421). The area
// mapping settings are GUI state: "@spot_mode" 0 correction / 1 measure, "@use_mixing" and
// the target "@lightness_spot" (%), "@hue_spot" (°) and "@chroma_spot" (darktable keeps them in
// darkroom/modules/channelmixerrgb/*). "measure" reports the mapped colour back into them.
static int channelmixerrgb_picker(OmToolContext *ctx) {
    dt_iop_module_t *m = ctx->module;
    if (!has(m, "red") || !has(m, "adaptation") || !has(m, "illuminant"))
        return 3;
    const float *RGB = ctx->picked.in[DT_PICK_MEAN];
    const dt_iop_order_iccprofile_info_t *const work_profile = dt_ioppr_get_pipe_work_profile_info(&ctx->capture->pipe);
    if (!work_profile)
        return 4;
    dt_aligned_pixel_t XYZ, Lab, Lch;
    dot_product(RGB, work_profile->matrix_in, XYZ);
    dt_XYZ_to_Lab(XYZ, Lab);
    dt_Lab_2_LCH(Lab, Lch);
    om_tool_set_gui(ctx, "input_lightness", Lch[0]);
    om_tool_set_gui(ctx, "input_hue", Lch[2] * 360.f);
    om_tool_set_gui(ctx, "input_chroma", Lch[1]);
    const int mode = (int)om_tool_gui(ctx, "@spot_mode", 0);
    const gboolean use_mixing = om_tool_gui(ctx, "@use_mixing", 0) > 0.5;
    const float *red = P(m, "red"), *green = P(m, "green"), *blue = P(m, "blue");
    dt_colormatrix_t MIX = {{0.f}};
    const float norm_R = *PI(m, "normalize_R") ? MAX(NORM_MIN, red[0] + red[1] + red[2]) : 1.0f;
    const float norm_G = *PI(m, "normalize_G") ? MAX(NORM_MIN, green[0] + green[1] + green[2]) : 1.0f;
    const float norm_B = *PI(m, "normalize_B") ? MAX(NORM_MIN, blue[0] + blue[1] + blue[2]) : 1.0f;
    for_three_channels(i) {
        MIX[0][i] = red[i] / norm_R;
        MIX[1][i] = green[i] / norm_G;
        MIX[2][i] = blue[i] / norm_B;
    }
    const dt_adaptation_t p_adaptation = *PI(m, "adaptation");
    if (mode == 1) { // DT_SPOT_MODE_MEASURE
        float x = *P(m, "x"), y = *P(m, "y");
        dt_adaptation_t adaptation = p_adaptation;
        dt_aligned_pixel_t custom_wb;
        cat_white_balance_coeff(ctx->dev, custom_wb);
        const dt_illuminant_t illuminant = *PI(m, "illuminant");
        illuminant_to_xy(illuminant, &ctx->dev->image_storage, custom_wb, &x, &y, *P(m, "temperature"),
                         *PI(m, "illum_fluo"), *PI(m, "illum_led"));
        if (illuminant == DT_ILLUMINANT_CAMERA)
            cat_close_to_daylight(x, y, NULL, &adaptation);
        dt_aligned_pixel_t XYZ_illuminant = {0.f}, LMS_illuminant = {0.f}, XYZ_output = {0.f};
        illuminant_xy_to_XYZ(x, y, XYZ_illuminant);
        convert_any_XYZ_to_LMS(XYZ_illuminant, LMS_illuminant, adaptation);
        const float pp = powf(0.818155f / MAX(NORM_MIN, LMS_illuminant[2]), 0.0834f);
        chroma_adapt_pixel(XYZ, XYZ_output, LMS_illuminant, adaptation, pp);
        if (use_mixing) {
            dt_aligned_pixel_t LMS_output = {0.f}, temp = {0.f};
            convert_any_XYZ_to_LMS(XYZ_output, LMS_output, adaptation);
            dot_product(LMS_output, MIX, temp);
            convert_any_LMS_to_XYZ(temp, XYZ_output, adaptation);
        }
        dt_aligned_pixel_t Lab_output = {0.f}, Lch_output = {0.f};
        dt_XYZ_to_Lab(XYZ_output, Lab_output);
        dt_Lab_2_LCH(Lab_output, Lch_output);
        om_tool_set_gui(ctx, "@lightness_spot", Lch_output[0]);
        om_tool_set_gui(ctx, "@chroma_spot", Lch_output[1]);
        om_tool_set_gui(ctx, "@hue_spot", Lch_output[2] * 360.f);
        return 0;
    }
    dt_aligned_pixel_t Lch_target = {(float)om_tool_gui(ctx, "@lightness_spot", 50.0),
                                     (float)om_tool_gui(ctx, "@chroma_spot", 0.0),
                                     (float)om_tool_gui(ctx, "@hue_spot", 0.0) / 360.f, 0.f};
    dt_aligned_pixel_t Lab_target = {0.f}, XYZ_target = {0.f}, LMS_target = {0.f};
    dt_LCH_2_Lab(Lch_target, Lab_target);
    dt_Lab_to_XYZ(Lab_target, XYZ_target);
    const float Y_target = MAX(NORM_MIN, XYZ_target[1]);
    dt_vector_div1(XYZ_target, XYZ_target, Y_target);
    convert_any_XYZ_to_LMS(XYZ_target, LMS_target, p_adaptation);
    if (use_mixing) {
        float MIX_3x3[9], MIX_INV_3x3[9];
        pack_3xSSE_to_3x3(MIX, MIX_3x3);
        matrice_pseudoinverse((float(*)[3])MIX_3x3, (float(*)[3])MIX_INV_3x3, 3);
        dt_colormatrix_t MIX_INV;
        transpose_3x3_to_3xSSE(MIX_INV_3x3, MIX_INV);
        dt_aligned_pixel_t temp = {0.0f};
        dot_product(LMS_target, MIX_INV, temp);
        convert_any_LMS_to_XYZ(temp, XYZ_target, p_adaptation);
        const float Y_mix = MAX(NORM_MIN, XYZ_target[1]);
        dt_vector_div1(XYZ_target, XYZ_target, Y_mix);
        convert_any_XYZ_to_LMS(XYZ_target, LMS_target, p_adaptation);
    }
    dt_aligned_pixel_t LMS = {0.f};
    const float Y = MAX(NORM_MIN, XYZ[1]);
    dt_vector_div1(XYZ, XYZ, Y);
    convert_any_XYZ_to_LMS(XYZ, LMS, p_adaptation);
    dt_aligned_pixel_t D50, illuminant_LMS = {0.f}, illuminant_XYZ = {0.f};
    convert_D50_to_LMS(p_adaptation, D50);
    for_three_channels(c) illuminant_LMS[c] = D50[c] * LMS[c] / LMS_target[c];
    convert_any_LMS_to_XYZ(illuminant_LMS, illuminant_XYZ, p_adaptation);
    const float sum = fmaxf(illuminant_XYZ[0] + illuminant_XYZ[1] + illuminant_XYZ[2], NORM_MIN);
    illuminant_XYZ[0] /= sum;
    illuminant_XYZ[2] = illuminant_XYZ[1];
    illuminant_XYZ[1] /= sum;
    *P(m, "x") = illuminant_XYZ[0];
    *P(m, "y") = illuminant_XYZ[1];
    *PI(m, "illuminant") = DT_ILLUMINANT_CUSTOM;
    cat_close_to_daylight(*P(m, "x"), *P(m, "y"), P(m, "temperature"), NULL);
    return 0;
}

const OmToolSpec om_tools_color[] = {
    // colorbalance.c:1884 grey, :1921 factor and :1934 hue pickers of each wheel, :1999/:2004
    // the optimisers; all area pickers in Lab (the module's default colour space)
    {"colorbalance", "grey", OM_TOOL_AREA, -1, 0, colorbalance_grey},
    {"colorbalance", "lift_factor", OM_TOOL_AREA, -1, 0, colorbalance_lift_factor},
    {"colorbalance", "gamma_factor", OM_TOOL_AREA, -1, 0, colorbalance_gamma_factor},
    {"colorbalance", "gain_factor", OM_TOOL_AREA, -1, 0, colorbalance_gain_factor},
    {"colorbalance", "hue_lift", OM_TOOL_AREA, -1, 0, colorbalance_hue_lift},
    {"colorbalance", "hue_gamma", OM_TOOL_AREA, -1, 0, colorbalance_hue_gamma},
    {"colorbalance", "hue_gain", OM_TOOL_AREA, -1, 0, colorbalance_hue_gain},
    {"colorbalance", "auto_luma", OM_TOOL_AREA, -1, 0, colorbalance_auto_luma},
    {"colorbalance", "auto_color", OM_TOOL_AREA, -1, 0, colorbalance_auto_color},
    // channelmixerrgb.c:4486, the area picker of the CAT page and the area mapping
    {"channelmixerrgb", "picker", OM_TOOL_AREA, -1, 0, channelmixerrgb_picker},
    {NULL, NULL, 0, 0, 0, NULL},
};
