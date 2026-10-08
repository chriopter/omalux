// SPDX-License-Identifier: GPL-3.0-or-later
// The pickers of the curve modules (tone curve, rgb curve, color zones), ported from
// darktable 5.6.1 without their GTK code. "show color" pickers only report the picked input
// and output for the graph; "create curve" pickers replace the current curve.
#include "module_tools_internal.h"
#include "common/colorspaces_inline_conversions.h"

// Nodes are {float x, y} pairs in arrays [channels][nodes]; dimensions come from introspection.
typedef struct {
    float *nodes;    // the channel's first node, x at [2k], y at [2k+1]
    int *count;      // nodes in use
    int *type;       // interpolation
    int max_nodes;
    const float *default_nodes;
    const int *default_count, *default_type;
} OmCurve;

static gboolean curve_at(dt_iop_module_t *m, const char *nodes, const char *count, const char *type, int channel,
                         OmCurve *c) {
    const dt_introspection_field_t *f = m->get_f ? m->get_f(nodes) : NULL;
    if (!f || f->header.type != DT_INTROSPECTION_TYPE_ARRAY || !m->get_f(count) || !m->get_f(type))
        return FALSE;
    const dt_introspection_field_t *inner = f->Array.field;
    if (!inner || inner->header.type != DT_INTROSPECTION_TYPE_ARRAY || channel < 0 || channel >= f->Array.count)
        return FALSE;
    c->max_nodes = inner->Array.count;
    c->nodes = (float *)m->get_p(m->params, nodes) + (size_t)channel * c->max_nodes * 2;
    c->count = (int *)m->get_p(m->params, count) + channel;
    c->type = (int *)m->get_p(m->params, type) + channel;
    c->default_nodes = (const float *)m->get_p(m->default_params, nodes) + (size_t)channel * c->max_nodes * 2;
    c->default_count = (const int *)m->get_p(m->default_params, count) + channel;
    c->default_type = (const int *)m->get_p(m->default_params, type) + channel;
    return TRUE;
}
static void curve_reset(OmCurve *c) {
    *c->count = *c->default_count;
    *c->type = *c->default_type;
    memcpy(c->nodes, c->default_nodes, sizeof(float) * 2 * c->max_nodes);
}
// rgbcurve.c _add_node (lines 478-505) and, with min_distance > 0, colorzones.c _add_node
// (lines 1817-1856), which also refuses nodes closer than DT_IOP_COLORZONES_MIN_X_DISTANCE.
static int curve_add(OmCurve *c, float x, float y, float min_distance) {
    if (*c->count >= c->max_nodes)
        return -1;
    float *n = c->nodes;
    int selected = -1;
    if (n[0] > x)
        selected = 0;
    else
        for (int k = 1; k < *c->count; k++)
            if (n[2 * k] > x) {
                selected = k;
                break;
            }
    if (selected == -1)
        selected = *c->count;
    if (min_distance > 0.f && ((selected > 0 && x - n[2 * (selected - 1)] <= min_distance) ||
                               (selected < *c->count && n[2 * selected] - x <= min_distance)))
        return -2;
    for (int i = *c->count; i > selected; i--) {
        n[2 * i] = n[2 * (i - 1)];
        n[2 * i + 1] = n[2 * (i - 1) + 1];
    }
    n[2 * selected] = x;
    n[2 * selected + 1] = y;
    (*c->count)++;
    return selected;
}

// The "flat / ctrl positive / shift negative" variant of darktable's create-curve pickers,
// passed as "@picker_modifier" 0 / 1 / -1 (dt_key_modifier_state at apply time).
static int picker_modifier(OmToolContext *ctx) {
    const int m = (int)om_tool_gui(ctx, "@picker_modifier", 0);
    return m > 0 ? 1 : m < 0 ? -1 : 0;
}

// The marker darktable draws on the graph for the picked area: the min…max band and the mean in
// curve coordinates per channel, and the "input → output" text where the module shows one.
static void add_marker(OmToolContext *ctx, const float min[3], const float mean[3], const float max[3],
                       const char *const text[3]) {
    JsonObject *marker = json_object_new();
    const float *const values[3] = {min, mean, max};
    const char *const names[3] = {"min", "mean", "max"};
    for (int v = 0; v < 3; ++v) {
        JsonArray *a = json_array_new();
        for (int c = 0; c < 3; ++c)
            json_array_add_double_element(a, isfinite(values[v][c]) ? values[v][c] : 0.0);
        json_object_set_array_member(marker, names[v], a);
    }
    if (text) {
        JsonArray *a = json_array_new();
        for (int c = 0; c < 3; ++c)
            json_array_add_string_element(a, text[c] ? text[c] : "");
        json_object_set_array_member(marker, "text", a);
    }
    json_object_set_object_member(ctx->extra, "marker", marker);
}

// tonecurve.c picker_scale (lines 1357-1362) and the label of dt_iop_tonecurve_draw (line 1606).
static int tonecurve_show(OmToolContext *ctx) {
    float s[3][3];
    for (int v = 0; v < 3; ++v) {
        const float *in = ctx->picked.in[v == 0 ? DT_PICK_MIN : v == 1 ? DT_PICK_MEAN : DT_PICK_MAX];
        s[v][0] = CLAMP(in[0] / 100.0f, 0.0f, 1.0f);
        s[v][1] = CLAMP((in[1] + 128.0f) / 256.0f, 0.0f, 1.0f);
        s[v][2] = CLAMP((in[2] + 128.0f) / 256.0f, 0.0f, 1.0f);
    }
    gchar *text[3] = {NULL, NULL, NULL};
    for (int c = 0; c < 3 && ctx->picked.output_valid; ++c)
        text[c] = g_strdup_printf("%.1f → %.1f", ctx->picked.in[DT_PICK_MEAN][c], ctx->picked.out[DT_PICK_MEAN][c]);
    add_marker(ctx, s[0], s[1], s[2], (const char *const *)text);
    for (int c = 0; c < 3; ++c)
        g_free(text[c]);
    return 0;
}

// rgbcurve.c picker_scale (lines 301-343) and the label of its draw function (line 1032).
static void rgbcurve_scale(dt_iop_module_t *m, const float *in, float out[3],
                           const dt_iop_order_iccprofile_info_t *work_profile) {
    const int autoscale = *(int *)m->get_p(m->params, "curve_autoscale");
    const int compensate = *(int *)m->get_p(m->params, "compensate_middle_grey");
    if (autoscale != 0) { // DT_S_SCALE_MANUAL_RGB
        for (int c = 0; c < 3; c++)
            out[c] = compensate && work_profile ? dt_ioppr_compensate_middle_grey(in[c], work_profile) : in[c];
        return;
    }
    dt_aligned_pixel_t pixel;
    copy_pixel(pixel, in);
    const float val = work_profile ? dt_ioppr_get_rgb_matrix_luminance(pixel, work_profile->matrix_in, work_profile->lut_in,
                                                                     work_profile->unbounded_coeffs_in,
                                                                     work_profile->lutsize, work_profile->nonlinearlut)
                                   : dt_camera_rgb_luminance(pixel);
    out[0] = compensate && work_profile ? dt_ioppr_compensate_middle_grey(val, work_profile) : val;
    out[1] = out[2] = 0.f;
}
static int rgbcurve_show(OmToolContext *ctx) {
    dt_iop_module_t *m = ctx->module;
    if (!m->get_f("curve_autoscale"))
        return 3;
    const dt_iop_order_iccprofile_info_t *const work_profile = dt_ioppr_get_pipe_work_profile_info(&ctx->capture->pipe);
    float s[3][3], mean[3], out[3];
    for (int v = 0; v < 3; ++v)
        rgbcurve_scale(m, ctx->picked.in[v == 0 ? DT_PICK_MIN : v == 1 ? DT_PICK_MEAN : DT_PICK_MAX], s[v],
                       work_profile);
    gchar *text[3] = {NULL, NULL, NULL};
    if (ctx->picked.output_valid) {
        rgbcurve_scale(m, ctx->picked.in[DT_PICK_MEAN], mean, work_profile);
        rgbcurve_scale(m, ctx->picked.out[DT_PICK_MEAN], out, work_profile);
        for (int c = 0; c < 3; ++c)
            text[c] = g_strdup_printf("%.1f → %.1f", mean[c] * 255.f, out[c] * 255.f);
    }
    add_marker(ctx, s[0], s[1], s[2], (const char *const *)text);
    for (int c = 0; c < 3; ++c)
        g_free(text[c]);
    return 0;
}

// colorzones.c _draw_color_picker (lines 988-1030): the "select by" channel of the picked LCh.
static void colorzones_marker(OmToolContext *ctx) {
    dt_iop_module_t *m = ctx->module;
    const int ch_val = *(int *)m->get_p(m->params, "channel");
    float s[3][3];
    for (int v = 0; v < 3; ++v) {
        const float *in = ctx->picked.in[v == 0 ? DT_PICK_MIN : v == 1 ? DT_PICK_MEAN : DT_PICK_MAX];
        const float x = ch_val == 0 ? in[0] / 100.0f : ch_val == 1 ? in[1] / (128.0f * M_SQRT2_F) : in[2];
        s[v][0] = s[v][1] = s[v][2] = x;
    }
    add_marker(ctx, s[0], s[1], s[2], NULL);
}
static int colorzones_show(OmToolContext *ctx) {
    if (!ctx->module->get_f("channel"))
        return 3;
    colorzones_marker(ctx);
    return 0;
}

// rgbcurve.c _add_node_from_picker and color_picker_apply (lines 507-581).
static int rgbcurve_node(OmCurve *c, dt_iop_module_t *m, const float *in, float increment, int ch,
                         const dt_iop_order_iccprofile_info_t *work_profile) {
    const int autoscale = *(int *)m->get_p(m->params, "curve_autoscale");
    const int compensate = *(int *)m->get_p(m->params, "compensate_middle_grey");
    dt_aligned_pixel_t pixel;
    copy_pixel(pixel, in);
    const float val = autoscale == 0 // DT_S_SCALE_AUTOMATIC_RGB
                          ? (work_profile ? dt_ioppr_get_rgb_matrix_luminance(pixel, work_profile->matrix_in,
                                                                              work_profile->lut_in,
                                                                              work_profile->unbounded_coeffs_in,
                                                                              work_profile->lutsize,
                                                                              work_profile->nonlinearlut)
                                          : dt_camera_rgb_luminance(pixel))
                          : in[ch];
    float x, y;
    if (compensate && work_profile)
        y = x = dt_ioppr_compensate_middle_grey(val, work_profile);
    else
        y = x = val;
    x = CLIP(x - increment);
    y = CLIP(y + increment);
    return curve_add(c, x, y, 0.f);
}
static int rgbcurve_create(OmToolContext *ctx) {
    dt_iop_module_t *m = ctx->module;
    const int autoscale = m->get_f("curve_autoscale") ? *(int *)m->get_p(m->params, "curve_autoscale") : 0;
    const int ch = autoscale == 0 ? 0 : CLAMP((int)om_tool_gui(ctx, "@tab", 0), 0, 2);
    OmCurve c;
    if (!curve_at(m, "curve_nodes", "curve_num_nodes", "curve_type", ch, &c))
        return 3;
    const dt_iop_order_iccprofile_info_t *const work_profile = dt_ioppr_get_pipe_work_profile_info(&ctx->capture->pipe);
    curve_reset(&c);
    const float increment = 0.05f * picker_modifier(ctx);
    rgbcurve_node(&c, m, ctx->picked.in[DT_PICK_MIN], 0.f, ch, work_profile);
    rgbcurve_node(&c, m, ctx->picked.in[DT_PICK_MEAN], increment, ch, work_profile);
    rgbcurve_node(&c, m, ctx->picked.in[DT_PICK_MAX], 0.f, ch, work_profile);
    if (*c.count == 5) {
        const float *n = c.nodes;
        curve_add(&c, n[2] - increment + (n[6] - n[2]) / 2.f, n[3] + increment + (n[7] - n[3]) / 2.f, 0.f);
    }
    return rgbcurve_show(ctx);
}

// colorzones.c color_picker_apply (lines 2396-2478): five nodes around the picked range of the
// "select by" channel, in LCh.
static int colorzones_create(OmToolContext *ctx) {
    dt_iop_module_t *m = ctx->module;
    if (!m->get_f("channel"))
        return 3;
    const int ch_curve = CLAMP((int)om_tool_gui(ctx, "@tab", 0), 0, 2);
    const int ch_val = *(int *)m->get_p(m->params, "channel");
    OmCurve c;
    if (!curve_at(m, "curve", "curve_num_nodes", "curve_type", ch_curve, &c))
        return 3;
    curve_reset(&c);
    const float feather = 0.02f, distance = 0.0025f; // DT_IOP_COLORZONES_MIN_X_DISTANCE
    const float increment = 0.1f * picker_modifier(ctx);
    const float *const pmin = ctx->picked.in[DT_PICK_MIN], *const pmean = ctx->picked.in[DT_PICK_MEAN],
                       *const pmax = ctx->picked.in[DT_PICK_MAX];
#define OM_ZONE(v) (ch_val == 0 ? (v)[0] / 100.f : ch_val == 1 ? (v)[1] / (128.f * M_SQRT2_F) : (v)[2])
    float x = OM_ZONE(pmin) - feather;
    if (x > 0.f && x < 1.f)
        curve_add(&c, x, .5f, distance);
    x = OM_ZONE(pmin);
    if (x > 0.f && x < 1.f)
        curve_add(&c, x, .5f + increment, distance);
    x = OM_ZONE(pmean);
    if (x > 0.f && x < 1.f)
        curve_add(&c, x, .5f + 2.f * increment, distance);
    x = OM_ZONE(pmax);
    if (x > 0.f && x < 1.f)
        curve_add(&c, x, .5f + increment, distance);
    x = OM_ZONE(pmax) + feather;
    if (x > 0.f && x < 1.f)
        curve_add(&c, x, .5f, distance);
#undef OM_ZONE
    colorzones_marker(ctx);
    return 0;
}

const OmToolSpec om_tools_curves[] = {
    // tonecurve.c:1275, point or area, input and output (Lab)
    {"tonecurve", "pick_color", OM_TOOL_AREA, -1, OM_TOOL_OUTPUT, tonecurve_show},
    // rgbcurve.c:1496 show color, :1505 create curve (RGB, input and output)
    {"rgbcurve", "show_color", OM_TOOL_AREA, -1, OM_TOOL_OUTPUT, rgbcurve_show},
    {"rgbcurve", "create_curve", OM_TOOL_AREA, -1, OM_TOOL_OUTPUT, rgbcurve_create},
    // colorzones.c:2635 show color, :2643 create curve, both in LCh
    {"colorzones", "show_color", OM_TOOL_AREA, IOP_CS_LCH, 0, colorzones_show},
    {"colorzones", "create_curve", OM_TOOL_AREA, IOP_CS_LCH, 0, colorzones_create},
    {NULL, NULL, 0, 0, 0, NULL},
};
