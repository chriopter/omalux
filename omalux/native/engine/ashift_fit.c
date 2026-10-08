// SPDX-License-Identifier: GPL-3.0-or-later
// rotate and perspective (ashift): the structure detection, the fit buttons and the automatic
// crop without darktable's GUI. The computations are darktable 5.6.1's own static functions,
// extracted unchanged from src/iop/ashift.c into ashift_fit_port.inc
// (development/tools/darktable/extract_ashift_fit.py); this file drives them the way the
// module's buttons and gui_changed do:
//   tool "fit" gui {"dir": dt_iop_ashift_fitaxis_t, "method": 1 auto | 2 quad | 3 lines,
//                   "enhance": dt_iop_ashift_enhance_t}
//     _do_get_structure_auto (3197: _get_structure on the module input, _remove_outliers) or
//     _draw_retrieve_lines_from_params (3065: the lines and rectangle drawn on the photo), then
//     do_fit (3389: nmsfit, do_crop) and the history item with the crop committed
//     (_event_fit_v_button_clicked 5323 swaps the crop box in for it).
//   om_ashift_autocrop: gui_changed (5273): every parameter edit refits the automatic crop.
// darktable keeps this state in the module's GUI data; it lives here for the duration of one
// call in an OmAshiftGui, handed to the extracted code as module->gui_data and taken back before
// anything else runs. The module input comes from module_tools.c's capture (1400 × 1000 fit),
// where darktable uses its preview pipe; line positions are scaled back the same way.
#include "module_tools_internal.h"
#include "ashift_fit.h"
#include "common/bilateral.h"
#include "common/colorspaces_inline_conversions.h"
#include "common/math.h"
#include "common/matrices.h"
#include "control/control.h"

#include "ashift_fit_types.inc"
// the members of dt_iop_ashift_gui_data_t (ashift.c:351-428) the extracted functions use
#define dt_iop_ashift_gui_data_t OmAshiftGui
typedef struct OmAshiftGui {
    int fitting, isflipped;
    float rotation_range, lensshift_v_range, lensshift_h_range, shear_range;
    dt_iop_ashift_line_t *lines;
    int lines_in_width, lines_in_height, lines_x_off, lines_y_off;
    int lines_count, vertical_count, horizontal_count, lines_version;
    float vertical_weight, horizontal_weight;
    float *buf;
    int buf_width, buf_height, buf_x_off, buf_y_off;
    float buf_scale;
    float cl, cr, ct, cb;
} OmAshiftGui;
#include "ashift_fit_port.inc"
#undef dt_iop_ashift_gui_data_t

static void gui_init_state(OmAshiftGui *g) {
    memset(g, 0, sizeof(*g));
    // gui_init (5943): the soft ranges bound the fit
    g->rotation_range = ROTATION_RANGE_SOFT;
    g->lensshift_v_range = LENSSHIFT_RANGE_SOFT;
    g->lensshift_h_range = LENSSHIFT_RANGE_SOFT;
    g->shear_range = SHEAR_RANGE_SOFT;
    g->cl = 0.0f;
    g->cr = 1.0f;
    g->ct = 0.0f;
    g->cb = 1.0f;
}

// process (3449-3473): is the final image flipped relative to this module?
static int is_flipped(dt_develop_t *dev, dt_dev_pixelpipe_t *pipe, dt_iop_module_t *module,
                      const dt_dev_pixelpipe_iop_t *piece) {
    dt_boundingbox_t points = {0.0f, 0.0f, (float)piece->buf_in.width, (float)piece->buf_in.height};
    const float ivec[2] = {points[2] - points[0], points[3] - points[1]};
    const float ivecl = dt_fast_hypotf(ivec[0], ivec[1]);
    dt_dev_distort_backtransform_plus(dev, pipe, module->iop_order, DT_DEV_TRANSFORM_DIR_FORW_EXCL, points, 2);
    const float ovec[2] = {points[2] - points[0], points[3] - points[1]};
    const float ovecl = dt_fast_hypotf(ovec[0], ovec[1]);
    const float alpha =
        acosf(CLAMP((ivec[0] * ovec[0] + ivec[1] * ovec[1]) / (ivecl * ovecl), -1.0f, 1.0f));
    return fabsf(fmodf(alpha + M_PI_F, M_PI_F) - M_PI_2f) < M_PI_4f;
}

// _draw_retrieve_lines_from_params (3065-3170) on the given pipe instead of the preview pipe.
static gboolean drawn_lines(dt_iop_module_t *self, dt_dev_pixelpipe_t *pipe, const dt_iop_ashift_method_t method) {
    OmAshiftGui *g = self->gui_data;
    dt_iop_ashift_params_t *p = self->params;
    const dt_dev_pixelpipe_iop_t *piece = dt_dev_distort_get_iop_pipe(self->dev, pipe, self);
    if (!piece)
        return FALSE;
    if (method == ASHIFT_METHOD_QUAD && p->last_quad_lines[0] > 0.0f && p->last_quad_lines[1] > 0.0f &&
        p->last_quad_lines[2] > 0.0f && p->last_quad_lines[3] > 0.0f) {
        float pts[8];
        memcpy(pts, p->last_quad_lines, sizeof(pts));
        if (dt_dev_distort_transform_plus(self->dev, pipe, self->iop_order, DT_DEV_TRANSFORM_DIR_BACK_EXCL, pts, 4)) {
            free(g->lines);
            g->lines = calloc(4, sizeof(dt_iop_ashift_line_t));
            _draw_basic_line(&g->lines[0], pts[0], pts[1], pts[2], pts[3], ASHIFT_LINE_VERTICAL_SELECTED);
            _draw_basic_line(&g->lines[1], pts[4], pts[5], pts[6], pts[7], ASHIFT_LINE_VERTICAL_SELECTED);
            _draw_basic_line(&g->lines[2], pts[0], pts[1], pts[4], pts[5], ASHIFT_LINE_HORIZONTAL_SELECTED);
            _draw_basic_line(&g->lines[3], pts[2], pts[3], pts[6], pts[7], ASHIFT_LINE_HORIZONTAL_SELECTED);
            g->lines_count = 4;
            g->vertical_count = g->horizontal_count = 2;
            g->vertical_weight = g->horizontal_weight = 2.0;
            g->lines_in_width = piece->iwidth;
            g->lines_in_height = piece->iheight;
            return TRUE;
        }
    }
    if (method == ASHIFT_METHOD_LINES && p->last_drawn_lines_count > 0) {
        float pts[MAX_SAVED_LINES * 4] = {0.0f};
        const int count = MIN(p->last_drawn_lines_count, MAX_SAVED_LINES);
        memcpy(pts, p->last_drawn_lines, sizeof(float) * count * 4);
        if (dt_dev_distort_transform_plus(self->dev, pipe, self->iop_order, DT_DEV_TRANSFORM_DIR_BACK_EXCL, pts,
                                          count * 2)) {
            free(g->lines);
            g->lines = calloc(count, sizeof(dt_iop_ashift_line_t));
            int vnb = 0, hnb = 0;
            for (int i = 0; i < count; i++) {
                dt_iop_ashift_linetype_t linetype = ASHIFT_LINE_VERTICAL_SELECTED;
                if (fabsf(pts[i * 4] - pts[i * 4 + 2]) > fabsf(pts[i * 4 + 1] - pts[i * 4 + 3]))
                    linetype = ASHIFT_LINE_HORIZONTAL_SELECTED;
                _draw_basic_line(&g->lines[i], pts[i * 4], pts[i * 4 + 1], pts[i * 4 + 2], pts[i * 4 + 3], linetype);
                if (linetype == ASHIFT_LINE_VERTICAL_SELECTED)
                    vnb++;
                else
                    hnb++;
            }
            g->lines_count = count;
            g->vertical_count = vnb;
            g->horizontal_count = hnb;
            g->vertical_weight = (float)vnb;
            g->horizontal_weight = (float)hnb;
            g->lines_in_width = piece->iwidth;
            g->lines_in_height = piece->iheight;
            return TRUE;
        }
    }
    return FALSE;
}

static int ashift_fit(OmToolContext *ctx) {
    dt_iop_module_t *m = ctx->module;
    OmCapture *c = ctx->capture;
    if (!m->get_f || !m->get_f("last_quad_lines") || m->params_size != sizeof(dt_iop_ashift_params_t))
        return 3;
    if (!c || !c->input || c->dsc.channels != 4)
        return 4;
    dt_iop_ashift_params_t *p = m->params;
    const dt_iop_ashift_fitaxis_t dir = (dt_iop_ashift_fitaxis_t)om_tool_gui(ctx, "dir", ASHIFT_FIT_BOTH_SHEAR);
    // the structure darktable fits: the one whose button is active; here "method", or the
    // sidebar's "auto" toggle ("@auto"), else the lines and then the rectangle drawn on the photo
    int method = (int)om_tool_gui(ctx, "method", 0);
    if (!method)
        method = om_tool_gui(ctx, "@auto", 0) > 0.5         ? ASHIFT_METHOD_AUTO
                 : p->last_drawn_lines_count > 0               ? ASHIFT_METHOD_LINES
                 : p->last_quad_lines[0] > 0.0f                ? ASHIFT_METHOD_QUAD
                                                               : ASHIFT_METHOD_AUTO;
    json_object_set_int_member(ctx->extra, "method", method);
    const dt_iop_ashift_enhance_t enhance = (dt_iop_ashift_enhance_t)om_tool_gui(ctx, "enhance", 0);
    OmAshiftGui g;
    gui_init_state(&g);
    g.buf = c->input;
    g.buf_width = c->roi.width;
    g.buf_height = c->roi.height;
    g.buf_x_off = c->roi.x;
    g.buf_y_off = c->roi.y;
    g.buf_scale = c->roi.scale;
    g.isflipped = is_flipped(ctx->dev, &c->pipe, m, c->piece);
    void *saved = m->gui_data;
    m->gui_data = &g;
    int error = 0;
    gboolean structure;
    if (method == ASHIFT_METHOD_AUTO)
        structure = _get_structure(m, enhance) && _remove_outliers(m);
    else
        structure = drawn_lines(m, &c->pipe, method == ASHIFT_METHOD_QUAD ? ASHIFT_METHOD_QUAD : ASHIFT_METHOD_LINES);
    json_object_set_int_member(ctx->extra, "vertical_lines", g.vertical_count);
    json_object_set_int_member(ctx->extra, "horizontal_lines", g.horizontal_count);
    if (!structure) {
        json_object_set_string_member(ctx->extra, "message", "could not detect structural data in image");
        error = 7;
    } else {
        // do_fit (3389-3428)
        const dt_iop_ashift_nmsresult_t res = nmsfit(m, p, dir);
        if (res == NMS_NOT_ENOUGH_LINES) {
            gchar *text = g_strdup_printf("not enough structure for automatic correction\n"
                                          "minimum %d lines in each relevant direction",
                                          MINIMUM_FITLINES);
            json_object_set_string_member(ctx->extra, "message", text);
            g_free(text);
            error = 7;
        } else if (res != NMS_SUCCESS) {
            json_object_set_string_member(ctx->extra, "message", "automatic correction failed, please correct manually");
            error = 7;
        } else {
            do_crop(m, p);
            _commit_crop_box(p, &g);
        }
    }
    free(g.lines);
    m->gui_data = saved;
    return error;
}

// gui_changed (5273-5290): the automatic crop follows every parameter change.
void om_ashift_autocrop(dt_develop_t *dev, dt_iop_module_t *module) {
    if (!dt_iop_module_is(module, "ashift") || module->params_size != sizeof(dt_iop_ashift_params_t))
        return;
    dt_dev_pixelpipe_iop_t *piece = dev->full.pipe ? dt_dev_distort_get_iop_pipe(dev, dev->full.pipe, module) : NULL;
    if (!piece || piece->buf_in.width <= 0 || piece->buf_in.height <= 0)
        return;
    OmAshiftGui g;
    gui_init_state(&g);
    g.buf_width = piece->buf_in.width;
    g.buf_height = piece->buf_in.height;
    void *saved = module->gui_data;
    module->gui_data = &g;
    do_crop(module, module->params);
    _commit_crop_box(module->params, &g);
    module->gui_data = saved;
}

const OmToolSpec om_tools_ashift[] = {
    // ashift.c:4706-4740 the fit buttons; the module input in RGB (default_colorspace)
    {"ashift", "fit", OM_TOOL_BUTTON, -1, OM_TOOL_CAPTURE, ashift_fit},
    {NULL, NULL, 0, 0, 0, NULL},
};
