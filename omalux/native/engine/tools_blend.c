// SPDX-License-Identifier: GPL-3.0-or-later
// The two colour pickers of darktable's parametric blend mask, for every module that blends
// (develop/blend_gui.c blend_color_picker_apply, _update_gradient_slider_pickers and their
// helpers, lines 326-500 and 1050-1910), without the GTK code. The request's GUI values name
// the shown channel: "tab" (the channel tab, blend_gui.c _channel_indexes), "channel_in" and
// "channel_out" (its parameter channels for input and output) and "@picker_modifier" (Ctrl:
// use the module output, darktable's ctrl+drag, only while the output sliders are shown, which
// "outputs_shown" says).
#include "module_tools_internal.h"
#include "common/colorspaces_inline_conversions.h"

static int blend_csp(dt_iop_module_t *m) {
    const int csp = m->blend_params ? m->blend_params->blend_cst : 0;
    return csp != DEVELOP_BLEND_CS_NONE ? csp : dt_develop_blend_default_module_blend_colorspace(m);
}

// _blendop_blendif_get_picker_colorspace (lines 1050-1077).
static int picker_colorspace(int csp, int tab) {
    if (csp == DEVELOP_BLEND_CS_RGB_DISPLAY)
        return tab < 4 ? IOP_CS_RGB : IOP_CS_HSL;
    if (csp == DEVELOP_BLEND_CS_RGB_SCENE)
        return tab < 4 ? IOP_CS_RGB : IOP_CS_JZCZHZ;
    if (csp == DEVELOP_BLEND_CS_LAB)
        return tab < 3 ? IOP_CS_LAB : IOP_CS_LCH;
    return IOP_CS_NONE;
}

// _blendif_scale and _blendif_cook (lines 381-480). boost[] are the exp2 boost factors of the
// tab's parameter channel, indexed by tab like darktable's _get_boost_factor.
static void blendif_scale(int cst, const float *in, float *out, const float boost,
                          const dt_iop_order_iccprofile_info_t *work_profile) {
    for (int i = 0; i < 8; ++i)
        out[i] = -1.0f;
    dt_aligned_pixel_t pixel = {in[0], in[1], in[2], in[3]};
    switch (cst) {
    case IOP_CS_LAB:
        out[0] = (in[0] / boost) / 100.0f;
        out[1] = ((in[1] / boost) + 128.0f) / 256.0f;
        out[2] = ((in[2] / boost) + 128.0f) / 256.0f;
        break;
    case IOP_CS_RGB:
        out[0] = (work_profile ? dt_ioppr_get_rgb_matrix_luminance(pixel, work_profile->matrix_in, work_profile->lut_in,
                                                                   work_profile->unbounded_coeffs_in,
                                                                   work_profile->lutsize, work_profile->nonlinearlut)
                               : 0.3f * in[0] + 0.59f * in[1] + 0.11f * in[2]) /
                 boost;
        out[1] = in[0] / boost;
        out[2] = in[1] / boost;
        out[3] = in[2] / boost;
        break;
    case IOP_CS_LCH:
        out[3] = (in[1] / boost) / (128.0f * M_SQRT2_F);
        out[4] = in[2] / boost;
        break;
    case IOP_CS_HSL:
        out[4] = in[0] / boost;
        out[5] = in[1] / boost;
        out[6] = in[2] / boost;
        break;
    case IOP_CS_JZCZHZ:
        out[4] = in[0] / boost;
        out[5] = in[1] / boost;
        out[6] = in[2] / boost;
        break;
    default:
        break;
    }
}
static void blendif_cook(int cst, const float *in, float *out, const dt_iop_order_iccprofile_info_t *work_profile) {
    for (int i = 0; i < 8; ++i)
        out[i] = -1.0f;
    dt_aligned_pixel_t pixel = {in[0], in[1], in[2], in[3]};
    switch (cst) {
    case IOP_CS_LAB:
        out[0] = in[0];
        out[1] = in[1];
        out[2] = in[2];
        break;
    case IOP_CS_RGB:
        out[0] = (work_profile ? dt_ioppr_get_rgb_matrix_luminance(pixel, work_profile->matrix_in, work_profile->lut_in,
                                                                   work_profile->unbounded_coeffs_in,
                                                                   work_profile->lutsize, work_profile->nonlinearlut)
                               : 0.3f * in[0] + 0.59f * in[1] + 0.11f * in[2]) *
                 100.0f;
        out[1] = in[0] * 100.0f;
        out[2] = in[1] * 100.0f;
        out[3] = in[2] * 100.0f;
        break;
    case IOP_CS_LCH:
        out[3] = in[1] / (128.0f * M_SQRT2_F) * 100.0f;
        out[4] = in[2] * 360.0f;
        break;
    case IOP_CS_HSL:
    case IOP_CS_JZCZHZ:
        out[4] = in[0] * (cst == IOP_CS_HSL ? 360.0f : 100.0f);
        out[5] = in[1] * 100.0f;
        out[6] = in[2] * (cst == IOP_CS_HSL ? 100.0f : 360.0f);
        break;
    default:
        break;
    }
}

typedef struct {
    int csp, cst, tab, ch[2];
    const dt_iop_order_iccprofile_info_t *work_profile;
} OmBlendPick;

static int blend_prepare(OmToolContext *ctx, OmBlendPick *b) {
    dt_iop_module_t *m = ctx->module;
    if (!m->blend_params || !(m->flags() & IOP_FLAGS_SUPPORTS_BLENDING))
        return 3;
    b->csp = blend_csp(m);
    b->tab = CLAMP((int)om_tool_gui(ctx, "tab", 0), 0, 7);
    b->ch[0] = CLAMP((int)om_tool_gui(ctx, "channel_in", 0), 0, DEVELOP_BLENDIF_SIZE - 1);
    b->ch[1] = CLAMP((int)om_tool_gui(ctx, "channel_out", 4), 0, DEVELOP_BLENDIF_SIZE - 1);
    // _blendif_colorpicker_cst: the active picker's colour space, set per tab.
    b->cst = picker_colorspace(b->csp, b->tab);
    if (b->cst == IOP_CS_NONE)
        return 3;
    b->work_profile = b->csp == DEVELOP_BLEND_CS_RGB_SCENE ? dt_ioppr_get_pipe_current_profile_info(m, &ctx->capture->pipe)
                                                          : dt_ioppr_get_iop_work_profile_info(m, ctx->dev->iop);
    ctx->picker_cst = b->cst;
    return om_tool_pick(ctx, b->cst, FALSE, &ctx->picked) ? 0 : 6;
}

// _update_gradient_slider_pickers: the picked mean, min and max per slider, and the label.
static void report_markers(OmToolContext *ctx, const OmBlendPick *b) {
    JsonObject *markers = json_object_new();
    for (int in_out = 0; in_out < 2; ++in_out) {
        if (in_out && !ctx->picked.output_valid)
            continue;
        const lib_colorpicker_stats *s = in_out ? &ctx->picked.out : &ctx->picked.in;
        const float boost = exp2f(ctx->module->blend_params->blendif_boost_factors[b->ch[in_out]]);
        float mean[8], min[8], max[8], cooked[8];
        blendif_scale(b->cst, (*s)[DT_PICK_MEAN], mean, boost, b->work_profile);
        blendif_scale(b->cst, (*s)[DT_PICK_MIN], min, boost, b->work_profile);
        blendif_scale(b->cst, (*s)[DT_PICK_MAX], max, boost, b->work_profile);
        blendif_cook(b->cst, (*s)[DT_PICK_MEAN], cooked, b->work_profile);
        JsonObject *o = json_object_new();
        json_object_set_double_member(o, "mean", CLAMP(mean[b->tab], 0.0f, 1.0f));
        json_object_set_double_member(o, "min", CLAMP(min[b->tab], 0.0f, 1.0f));
        json_object_set_double_member(o, "max", CLAMP(max[b->tab], 0.0f, 1.0f));
        gchar *text = g_strdup_printf("(%.*f)", cooked[b->tab] < 10.0f ? 2 : 1, cooked[b->tab]);
        json_object_set_string_member(o, "text", text);
        g_free(text);
        json_object_set_object_member(markers, in_out ? "output" : "input", o);
    }
    json_object_set_object_member(ctx->extra, "blendMarker", markers);
}

// The "show color" picker (bd->colorpicker): markers only, a keep-active picker.
static int blend_show(OmToolContext *ctx) {
    OmBlendPick b;
    const int error = blend_prepare(ctx, &b);
    if (error)
        return error;
    report_markers(ctx, &b);
    return 0;
}

// The "set range" picker (blend_color_picker_apply, lines 1763-1897).
static int blend_set_range(OmToolContext *ctx) {
    OmBlendPick b;
    const int error = blend_prepare(ctx, &b);
    if (error)
        return error;
    dt_develop_blend_params_t *bp = ctx->module->blend_params;
    const int in_out = (int)om_tool_gui(ctx, "@picker_modifier", 0) > 0 && om_tool_gui(ctx, "outputs_shown", 0) > 0.5 &&
                       ctx->picked.output_valid;
    dt_aligned_pixel_t raw_min, raw_max;
    const lib_colorpicker_stats *s = in_out ? &ctx->picked.out : &ctx->picked.in;
    for (int i = 0; i < 4; i++) {
        raw_min[i] = (*s)[DT_PICK_MIN][i];
        raw_max[i] = (*s)[DT_PICK_MAX][i];
    }
    const int ch = b.ch[in_out];
    float *parameters = &bp->blendif_parameters[4 * ch];
    gboolean reverse_hues = FALSE;
    if (b.cst == IOP_CS_HSL && b.tab == 4) {
        if ((raw_max[3] - raw_min[3]) < (raw_max[0] - raw_min[0]) && raw_min[3] < 0.5f && raw_max[3] > 0.5f) {
            raw_max[0] = raw_max[3] < 0.5f ? raw_max[3] + 0.5f : raw_max[3] - 0.5f;
            raw_min[0] = raw_min[3] < 0.5f ? raw_min[3] + 0.5f : raw_min[3] - 0.5f;
            reverse_hues = TRUE;
        }
    } else if ((b.cst == IOP_CS_LCH && b.tab == 4) || (b.cst == IOP_CS_JZCZHZ && b.tab == 6)) {
        if ((raw_max[3] - raw_min[3]) < (raw_max[2] - raw_min[2]) && raw_min[3] < 0.5f && raw_max[3] > 0.5f) {
            raw_max[2] = raw_max[3] < 0.5f ? raw_max[3] + 0.5f : raw_max[3] - 0.5f;
            raw_min[2] = raw_min[3] < 0.5f ? raw_min[3] + 0.5f : raw_min[3] - 0.5f;
            reverse_hues = TRUE;
        }
    }
    const float boost = exp2f(bp->blendif_boost_factors[ch]);
    float picker_min[8], picker_max[8];
    blendif_scale(b.cst, raw_min, picker_min, boost, b.work_profile);
    blendif_scale(b.cst, raw_max, picker_max, boost, b.work_profile);
    const float feather = 0.01f;
    const int tab = b.tab;
    if (picker_min[tab] > picker_max[tab]) {
        const float tmp = picker_min[tab];
        picker_min[tab] = picker_max[tab];
        picker_max[tab] = tmp;
    }
    float values[4];
    values[0] = CLAMP(picker_min[tab] - feather, 0.f, 1.f);
    values[1] = CLAMP(picker_min[tab] + feather, 0.f, 1.f);
    values[2] = CLAMP(picker_max[tab] - feather, 0.f, 1.f);
    values[3] = CLAMP(picker_max[tab] + feather, 0.f, 1.f);
    if (values[1] > values[2]) {
        values[1] = CLAMP(picker_min[tab], 0.f, 1.f);
        values[2] = CLAMP(picker_max[tab], 0.f, 1.f);
    }
    values[0] = CLAMP(values[0], 0.f, values[1]);
    values[3] = CLAMP(values[3], values[2], 1.f);
    for (int k = 0; k < 4; k++)
        parameters[k] = values[k];
    if (parameters[1] == 0.0f && parameters[2] == 1.0f)
        bp->blendif &= ~(1u << ch);
    else
        bp->blendif |= (1u << ch);
    if (reverse_hues == ((bp->mask_combine & DEVELOP_COMBINE_INV) == DEVELOP_COMBINE_INV))
        bp->blendif &= ~(1u << (16 + ch));
    else
        bp->blendif |= 1u << (16 + ch);
    report_markers(ctx, &b);
    return 0;
}

const OmToolSpec om_tools_blend[] = {
    // blend_gui.c:2578 show color (point or area, input and output, keep-active) and :2588 set
    // range (area, input and output); "*": every module with a blend section.
    {"*", "blend_show", OM_TOOL_BUTTON, -1, OM_TOOL_CAPTURE | OM_TOOL_OUTPUT, blend_show},
    {"*", "blend_set_range", OM_TOOL_BUTTON, -1, OM_TOOL_CAPTURE | OM_TOOL_OUTPUT, blend_set_range},
    {NULL, NULL, 0, 0, 0, NULL},
};
