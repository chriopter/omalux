// SPDX-License-Identifier: GPL-3.0-or-later
// color calibration's "calibrate with a color checker" without darktable's GUI. The patch
// extraction, the illuminant and matrix solver and the validation are darktable 5.6.1's own
// functions, extracted unchanged from src/iop/channelmixerrgb.c into checker_port.inc
// (development/tools/darktable/extract_checker.py); they read their state from the GUI data
// handed to them, here an OmCheckerGui. Tools:
//   checker_layout  gui {"@checker"}: the chart's patch centres in the unit square, its ratio and
//                   patch radius (darktable draws them through the corner homography,
//                   gui_post_expose 2682), for the overlay on the photo
//   checker         gui {"action": "profile" | "validate" | "accept", "corners": [x0, y0, ... x3, y3]
//                   (top left, top right, bottom right, bottom left; fractions of the displayed
//                   image), "@checker", "@optimize", "@safety" (the GUI-only rows, dt_conf keys
//                   darkroom/modules/channelmixerrgb/colorchecker, optimization, safety)}:
//                   profile  = recompute (_run_profile_callback → process → _extract_color_checker
//                              on the module input), the quality report
//                   validate = validate (_validate_color_checker on the module output)
//                   accept   = recompute, then _commit_profile_callback (2933): the illuminant x, y
//                              (custom, with its temperature) and the R, G, B mixing rows.
// darktable maps the corners straight into the module input of its preview pipe; here they go
// back through the modules after color calibration (dt_dev_distort_backtransform_plus), so a
// chart stays on its patches when cropping or lens correction follow.
#include "module_tools_internal.h"
#include "common/chromatic_adaptation.h"
#include "common/colorspaces_inline_conversions.h"
#include "common/dttypes.h"
#include "common/math.h"
#define pair_min om_checker_pair_min
#include "common/illuminants.h"
#undef pair_min
#include "common/colorchecker.h"
#include "chart/common.c"
#include "iop/gaussian_elimination.h"

// the members of dt_iop_channelmixer_rgb_gui_data_t (channelmixerrgb.c:139-210) the solver uses
typedef struct OmCheckerGui {
    dt_color_checker_t *checker;
    float *delta_E_in;
    gchar *delta_E_label_text;
    float homography[9], inverse_homography[9];
    dt_colormatrix_t mix;
    int optimization;
    gboolean profile_ready;
    float safety_margin;
    float xy[2];
} OmCheckerGui;
#define dt_iop_channelmixer_rgb_gui_data_t OmCheckerGui
#include "checker_port.inc"
#undef dt_iop_channelmixer_rgb_gui_data_t

static dt_color_checker_t *chart_of(OmToolContext *ctx) {
    const int chart = (int)om_tool_gui(ctx, "@checker", dt_conf_get_int("darkroom/modules/channelmixerrgb/colorchecker"));
    return dt_get_color_checker(CLAMP(chart, 0, COLOR_CHECKER_LAST - 1));
}

static int checker_layout(OmToolContext *ctx) {
    dt_color_checker_t *c = chart_of(ctx);
    JsonObject *out = json_object_new();
    json_object_set_string_member(out, "name", c->name);
    json_object_set_double_member(out, "ratio", c->ratio);
    json_object_set_double_member(out, "radius", c->radius);
    JsonArray *patches = json_array_new();
    for (size_t k = 0; k < c->patches; k++) {
        JsonArray *p = json_array_new();
        json_array_add_double_element(p, c->values[k].x);
        json_array_add_double_element(p, c->values[k].y);
        json_array_add_array_element(patches, p);
    }
    json_object_set_array_member(out, "patches", patches);
    json_object_set_object_member(ctx->extra, "chart", out);
    return 0;
}

// The corners from the displayed image to the module input buffer (pixels of the capture).
static gboolean corners_to_input(OmToolContext *ctx, point_t box[4]) {
    JsonArray *corners = ctx->gui && json_object_has_member(ctx->gui, "corners")
                             ? json_object_get_array_member(ctx->gui, "corners")
                             : NULL;
    if (!corners || json_array_get_length(corners) != 8)
        return FALSE;
    dt_dev_pixelpipe_t *pipe = ctx->dev->full.pipe;
    float pts[8];
    for (int i = 0; i < 4; i++) {
        pts[2 * i] = json_array_get_double_element(corners, 2 * i) * pipe->processed_width;
        pts[2 * i + 1] = json_array_get_double_element(corners, 2 * i + 1) * pipe->processed_height;
    }
    if (!dt_dev_distort_backtransform_plus(ctx->dev, pipe, ctx->module->iop_order, DT_DEV_TRANSFORM_DIR_FORW_EXCL,
                                           pts, 4))
        return FALSE;
    const dt_iop_roi_t *roi = &ctx->capture->roi;
    for (int i = 0; i < 4; i++) {
        box[i].x = pts[2 * i] * roi->scale - roi->x;
        box[i].y = pts[2 * i + 1] * roi->scale - roi->y;
    }
    return TRUE;
}

static void report(OmToolContext *ctx, OmCheckerGui *g) {
    if (g->delta_E_label_text) {
        // the GUI label's Pango markup, without the tags
        GString *text = g_string_new(NULL);
        gboolean tag = FALSE;
        for (const char *s = g->delta_E_label_text; *s; s++) {
            if (*s == '<')
                tag = TRUE;
            else if (*s == '>')
                tag = FALSE;
            else if (!tag)
                g_string_append_c(text, *s);
        }
        json_object_set_string_member(ctx->extra, "report", g_strstrip(text->str));
        g_string_free(text, TRUE);
    }
    JsonArray *mix = json_array_new();
    for (int i = 0; i < 3; i++)
        for (int j = 0; j < 3; j++)
            json_array_add_double_element(mix, g->mix[i][j]);
    json_object_set_array_member(ctx->extra, "mix", mix);
    om_tool_set_gui(ctx, "x", g->xy[0]);
    om_tool_set_gui(ctx, "y", g->xy[1]);
}

static int checker_run(OmToolContext *ctx) {
    dt_iop_module_t *m = ctx->module;
    OmCapture *c = ctx->capture;
    const char *action = ctx->gui ? json_object_get_string_member_with_default(ctx->gui, "action", "profile") : "profile";
    if (!m->get_f || !m->get_f("red") || !m->get_f("adaptation"))
        return 3;
    if (!c || !c->input || c->dsc.channels != 4)
        return 4;
    const gboolean validate = !strcmp(action, "validate");
    if (validate && !c->output)
        return 4;
    OmCheckerGui g = {0};
    g.checker = chart_of(ctx);
    g.optimization = (int)om_tool_gui(ctx, "@optimize", dt_conf_get_int("darkroom/modules/channelmixerrgb/optimization"));
    g.safety_margin = (float)om_tool_gui(ctx, "@safety", dt_conf_get_float("darkroom/modules/channelmixerrgb/safety"));
    point_t box[4];
    if (!corners_to_input(ctx, box)) {
        return 5;
    }
    // _init_bounding_box / _update_bounding_box (2441, 2460)
    const point_t ideal[4] = {{0.f, 0.f}, {1.f, 0.f}, {1.f, 1.f}, {0.f, 1.f}};
    get_homography(ideal, box, g.homography);
    get_homography(box, ideal, g.inverse_homography);
    // process (2114, the profile at 2165, validation at 2289): the pipe's working and input profiles
    const dt_iop_order_iccprofile_info_t *work = dt_ioppr_get_pipe_work_profile_info(&c->pipe);
    const dt_iop_order_iccprofile_info_t *input = dt_ioppr_get_pipe_input_profile_info(&c->pipe);
    if (!work || !input)
        return 4;
    dt_colormatrix_t RGB_to_XYZ, XYZ_to_RGB, XYZ_to_CAM;
    memcpy(RGB_to_XYZ, work->matrix_in, sizeof(RGB_to_XYZ));
    memcpy(XYZ_to_RGB, work->matrix_out, sizeof(XYZ_to_RGB));
    memcpy(XYZ_to_CAM, input->matrix_out, sizeof(XYZ_to_CAM));
    const dt_adaptation_t adaptation = *(int *)m->get_p(m->params, "adaptation");
    float *scratch = dt_alloc_align_float((size_t)c->roi.width * c->roi.height * 4);
    if (!scratch)
        return 4;
    if (validate)
        _validate_color_checker(c->output, &c->roi, &g, RGB_to_XYZ, XYZ_to_RGB, XYZ_to_CAM);
    else
        _extract_color_checker(c->input, scratch, &c->roi, &g, RGB_to_XYZ, XYZ_to_RGB, XYZ_to_CAM, adaptation);
    dt_free_align(scratch);
    report(ctx, &g);
    int error = 0;
    if (!strcmp(action, "accept") && g.profile_ready) {
        // _commit_profile_callback (2933-2985)
        float *x = m->get_p(m->params, "x"), *y = m->get_p(m->params, "y");
        float *red = m->get_p(m->params, "red"), *green = m->get_p(m->params, "green"),
              *blue = m->get_p(m->params, "blue");
        *x = g.xy[0];
        *y = g.xy[1];
        *(int *)m->get_p(m->params, "illuminant") = DT_ILLUMINANT_CUSTOM;
        _check_if_close_to_daylight(*x, *y, m->get_p(m->params, "temperature"), NULL, NULL);
        for (int k = 0; k < 3; k++) {
            red[k] = g.mix[0][k];
            green[k] = g.mix[1][k];
            blue[k] = g.mix[2][k];
        }
    } else if (!strcmp(action, "accept")) {
        // darktable keeps "accept" insensitive until a profile is ready
        json_object_set_string_member(ctx->extra, "message", "the patches gave no profile");
        error = 7;
    }
    dt_free_align(g.delta_E_in);
    g_free(g.delta_E_label_text);
    return error;
}

const OmToolSpec om_tools_checker[] = {
    // channelmixerrgb.c:4669-4740 the "calibrate with a color checker" section
    {"channelmixerrgb", "checker_layout", OM_TOOL_BUTTON, -1, OM_TOOL_KEEP_OFF, checker_layout},
    {"channelmixerrgb", "checker", OM_TOOL_BUTTON, -1, OM_TOOL_CAPTURE | OM_TOOL_OUTPUT, checker_run},
    {NULL, NULL, 0, 0, 0, NULL},
};
