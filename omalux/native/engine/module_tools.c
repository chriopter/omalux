// SPDX-License-Identifier: GPL-3.0-or-later
// darktable's colour pickers and module buttons without its GUI.
//
// In darktable the picker samples the module input on the preview pipe while the module has
// focus (develop/pixelpipe_hb.c, _request_color_pick and _pixelpipe_picker): the box drawn on
// the image is transformed back to the module's input coordinates (common/color_picker.c,
// dt_color_picker_box), the input is converted to the module's input colour space (or to the
// blend colour space while a mask is active) and dt_color_picker_helper averages it in the
// picker's colour space. The module's color_picker_apply then writes its parameters.
//
// Headless there is no preview pipe with a focused module, so this file renders the module
// input with a private export-type pipe of the editor's develop state: every node from the
// module on is disabled, the result is the module input. The picked statistics come from the
// same darktable helper; the per-module apply functions are ports in tools_*.c.
#include "module_tools_internal.h"
#include "common/histogram.h"
#include "imageio/imageio_common.h"

typedef struct {
    dt_iop_module_t *module;
    void *params;
    dt_develop_blend_params_t blend; // the blend pickers write the blend parameters
    gboolean enabled, force_enable;
} OmTouched;

static const OmToolSpec *const tables[] = {om_tools_tone, om_tools_color, om_tools_geometry, om_tools_basicadj,
                                            om_tools_curves, om_tools_harmonizer, om_tools_blend,
                                            om_tools_effects, om_tools_toneequal, om_tools_colormapping,
                                            om_tools_vectorscope, om_tools_blend_display};

static const OmToolSpec *find_spec(const char *operation, const char *tool) {
    for (size_t t = 0; t < G_N_ELEMENTS(tables); ++t)
        for (const OmToolSpec *s = tables[t]; s->operation; ++s)
            if ((!strcmp(s->operation, operation) || !strcmp(s->operation, "*")) && !strcmp(s->tool, tool))
                return s;
    return NULL;
}

char *om_engine_module_tool_list(void) {
    JsonBuilder *b = json_builder_new();
    json_builder_begin_array(b);
    for (size_t t = 0; t < G_N_ELEMENTS(tables); ++t)
        for (const OmToolSpec *s = tables[t]; s->operation; ++s) {
            json_builder_begin_object(b);
            json_builder_set_member_name(b, "operation");
            json_builder_add_string_value(b, s->operation);
            json_builder_set_member_name(b, "tool");
            json_builder_add_string_value(b, s->tool);
            json_builder_end_object(b);
        }
    json_builder_end_array(b);
    JsonNode *root = json_builder_get_root(b);
    char *text = json_to_string(root, FALSE);
    json_node_unref(root);
    g_object_unref(b);
    return text;
}

double om_tool_gui(const OmToolContext *ctx, const char *name, double fallback) {
    if (!ctx->gui || !json_object_has_member(ctx->gui, name))
        return fallback;
    JsonNode *node = json_object_get_member(ctx->gui, name);
    if (!JSON_NODE_HOLDS_VALUE(node))
        return fallback;
    const GType type = json_node_get_value_type(node);
    if (type == G_TYPE_DOUBLE || type == G_TYPE_INT64)
        return json_node_get_double(node);
    if (type == G_TYPE_BOOLEAN)
        return json_node_get_boolean(node);
    return fallback;
}

void om_tool_set_gui(OmToolContext *ctx, const char *name, double value) {
    json_object_set_double_member(ctx->gui_out, name, value);
}

static void touch(OmToolContext *ctx, dt_iop_module_t *module, gboolean force_enable) {
    for (guint i = 0; i < ctx->touched->len; ++i)
        if (((OmTouched *)ctx->touched->pdata[i])->module == module)
            return;
    OmTouched *t = g_new0(OmTouched, 1);
    t->module = module;
    t->params = g_memdup2(module->params, module->params_size);
    if (module->blend_params)
        t->blend = *module->blend_params;
    t->enabled = module->enabled;
    t->force_enable = force_enable;
    g_ptr_array_add(ctx->touched, t);
}

void om_tool_also(OmToolContext *ctx, dt_iop_module_t *module) {
    if (module)
        touch(ctx, module, FALSE);
}

dt_iop_module_t *om_tool_module(OmToolContext *ctx, const char *operation) {
    return dt_iop_get_module_by_op_priority(ctx->dev->iop, operation, 0);
}

static void capture_free(OmCapture *c) {
    if (!c)
        return;
    if (c->pipe_ready)
        dt_dev_pixelpipe_cleanup(&c->pipe);
    if (c->buf_ready)
        dt_mipmap_cache_release(&c->buf);
    dt_free_align(c->input);
    dt_free_align(c->output);
    g_free(c);
}

// darktable's coordinate sort for the four back-transformed corners
// (common/color_picker.c, _sort_coordinates).
static void sort_coordinates(float *fbox) {
    float tmp;
#define OM_SWAP(a, b)                                                                                        \
    {                                                                                                        \
        tmp = (a);                                                                                           \
        (a) = (b);                                                                                           \
        (b) = tmp;                                                                                           \
    }
    if (fbox[0] > fbox[2])
        OM_SWAP(fbox[0], fbox[2]);
    if (fbox[1] > fbox[3])
        OM_SWAP(fbox[1], fbox[3]);
    if (fbox[4] > fbox[6])
        OM_SWAP(fbox[4], fbox[6]);
    if (fbox[5] > fbox[7])
        OM_SWAP(fbox[5], fbox[7]);
    if (fbox[0] > fbox[4])
        OM_SWAP(fbox[0], fbox[4]);
    if (fbox[1] > fbox[5])
        OM_SWAP(fbox[1], fbox[5]);
    if (fbox[2] > fbox[6])
        OM_SWAP(fbox[2], fbox[6]);
    if (fbox[3] > fbox[7])
        OM_SWAP(fbox[3], fbox[7]);
    if (fbox[2] > fbox[4])
        OM_SWAP(fbox[2], fbox[4]);
    if (fbox[3] > fbox[5])
        OM_SWAP(fbox[3], fbox[5]);
#undef OM_SWAP
}

static float *copy_backbuf(dt_dev_pixelpipe_t *pipe, int width, int height) {
    if (!pipe->backbuf || pipe->backbuf_width != width || pipe->backbuf_height != height)
        return NULL;
    const size_t floats = (size_t)width * height * pipe->dsc.channels;
    float *copy = dt_alloc_align_float(floats);
    if (copy)
        memcpy(copy, pipe->backbuf, floats * sizeof(float));
    return copy;
}

// Render the module input (and output) of the editor's current state. The box is the
// normalised displayed-image box; it is transformed back like dt_color_picker_box does.
static int capture(OmToolContext *ctx, gboolean want_output) {
    dt_develop_t *dev = ctx->dev;
    dt_iop_module_t *module = ctx->module;
    OmCapture *c = g_new0(OmCapture, 1);
    ctx->capture = c;
    dt_mipmap_cache_get(&c->buf, dev->image_storage.id, DT_MIPMAP_FULL, DT_MIPMAP_BLOCKING, 'r');
    c->buf_ready = TRUE;
    if (!c->buf.buf || c->buf.width <= 0 || c->buf.height <= 0)
        return 4;
    if (!dt_dev_pixelpipe_init_export(&c->pipe, c->buf.width, c->buf.height, IMAGEIO_RGB | IMAGEIO_FLOAT, FALSE))
        return 4;
    c->pipe_ready = TRUE;
    dt_dev_pixelpipe_set_input(&c->pipe, dev, (float *)c->buf.buf, c->buf.width, c->buf.height, c->buf.iscale);
    dt_dev_pixelpipe_create_nodes(&c->pipe, dev);
    dt_dev_pixelpipe_synch_all(&c->pipe, dev);
    int final_width = 0, final_height = 0;
    dt_dev_pixelpipe_get_dimensions(&c->pipe, dev, c->pipe.iwidth, c->pipe.iheight, &final_width, &final_height);

    // The four corners on the displayed image, back to the module's input coordinates.
    float points[8];
    for (int i = 0; i < 4; ++i) {
        points[2 * i] = final_width * (i % 3 > 0 ? ctx->box[2] : ctx->box[0]);
        points[2 * i + 1] = final_height * (i % 2 ? ctx->box[3] : ctx->box[1]);
    }
    const gboolean expanded = (module->flags() & IOP_FLAGS_EXPAND_ROI_IN) != 0;
    dt_dev_distort_backtransform_plus(dev, &c->pipe, module->iop_order - (expanded ? 1 : 0),
                                      DT_DEV_TRANSFORM_DIR_FORW_EXCL, points, 4);
    sort_coordinates(points);

    // Disable the module and everything after it: the pipe output is the module input.
    dt_dev_pixelpipe_iop_t *demosaic = NULL;
    for (GList *nodes = c->pipe.nodes; nodes; nodes = g_list_next(nodes)) {
        dt_dev_pixelpipe_iop_t *piece = nodes->data;
        if (piece->module == module)
            c->piece = piece;
        if (dt_iop_module_is(piece->module, "demosaic") && piece->enabled)
            demosaic = piece;
    }
    if (!c->piece)
        return 4;
    gboolean after = FALSE;
    for (GList *nodes = c->pipe.nodes; nodes; nodes = g_list_next(nodes)) {
        dt_dev_pixelpipe_iop_t *piece = nodes->data;
        after = after || piece == c->piece;
        if (after)
            piece->enabled = FALSE;
    }
    int width = 0, height = 0;
    dt_dev_pixelpipe_get_dimensions(&c->pipe, dev, c->pipe.iwidth, c->pipe.iheight, &width, &height);
    if (width <= 0 || height <= 0)
        return 4;
    // Sensor data before demosaic cannot be scaled by the pipe input: sample it 1:1 around the box.
    const gboolean mosaic = demosaic && module->iop_order < demosaic->module->iop_order &&
                            c->buf.width > 0 && dev->image_storage.buf_dsc.channels == 1;
    const float bx0 = 0.5f * (points[0] + points[2]), by0 = 0.5f * (points[1] + points[3]);
    const float bx1 = 0.5f * (points[4] + points[6]), by1 = 0.5f * (points[5] + points[7]);
    dt_iop_roi_t roi = {0, 0, width, height, 1.0f};
    if (mosaic) {
        if (ctx->spec->kind == OM_TOOL_AREA || ctx->spec->kind == OM_TOOL_POINT) {
            roi.x = CLAMP((int)floorf(bx0) - 4, 0, width - 1) & ~1;
            roi.y = CLAMP((int)floorf(by0) - 4, 0, height - 1) & ~1;
            roi.width = CLAMP((int)ceilf(bx1) + 4, roi.x + 2, width) - roi.x;
            roi.height = CLAMP((int)ceilf(by1) + 4, roi.y + 2, height) - roi.y;
        }
    } else {
        roi.scale = fminf(1.0f, fminf((float)OM_PREVIEW_WIDTH / width, (float)OM_PREVIEW_HEIGHT / height));
        roi.width = MAX(1, (int)floorf(width * roi.scale));
        roi.height = MAX(1, (int)floorf(height * roi.scale));
    }
    c->roi = roi;
    if (dt_dev_pixelpipe_process_no_gamma(&c->pipe, dev, roi.x, roi.y, roi.width, roi.height, roi.scale))
        return 4;
    c->input = copy_backbuf(&c->pipe, roi.width, roi.height);
    if (!c->input)
        return 4;
    c->dsc = c->pipe.dsc;
    const int cst_from = c->pipe.dsc.cst;
    const int cst_to = module->input_colorspace(module, &c->pipe, c->piece);
    c->work_profile = cst_from != IOP_CS_RAW ? dt_ioppr_get_pipe_work_profile_info(&c->pipe) : NULL;
    if (c->dsc.channels == 4) {
        dt_iop_colorspace_type_t converted = cst_from;
        dt_ioppr_transform_image_colorspace(module, c->input, c->input, roi.width, roi.height, cst_from, cst_to,
                                            &converted, c->work_profile);
        c->dsc.cst = converted;
    }
    // While a mask is active darktable picks in the blend colour space (pixelpipe_hb.c,
    // blend_picking): the input is transformed there before the module output is blended.
    const dt_develop_blend_params_t *blend = c->piece->blendop_data;
    const int out_cst = module->output_colorspace(module, &c->pipe, c->piece);
    const int blend_cst = dt_develop_blend_colorspace(c->piece, out_cst);
    const gboolean blend_picking = blend && blend->mask_mode != DEVELOP_MASK_DISABLED &&
                                   (module->flags() & IOP_FLAGS_SUPPORTS_BLENDING) && blend_cst != cst_to &&
                                   c->dsc.channels == 4;
    if (blend_picking) {
        dt_iop_colorspace_type_t converted = c->dsc.cst;
        dt_ioppr_transform_image_colorspace(module, c->input, c->input, roi.width, roi.height, c->dsc.cst,
                                            blend_cst, &converted, c->work_profile);
        c->dsc.cst = converted;
    }
    if (want_output) {
        c->piece->enabled = TRUE;
        if (!dt_dev_pixelpipe_process_no_gamma(&c->pipe, dev, roi.x, roi.y, roi.width, roi.height, roi.scale)) {
            c->output = copy_backbuf(&c->pipe, roi.width, roi.height);
            c->out_dsc = c->pipe.dsc;
            if (c->output && blend_picking && c->out_dsc.channels == 4) {
                dt_iop_colorspace_type_t converted = c->out_dsc.cst;
                dt_ioppr_transform_image_colorspace(module, c->output, c->output, roi.width, roi.height,
                                                    c->out_dsc.cst, blend_cst, &converted, c->work_profile);
                c->out_dsc.cst = converted;
            }
        }
        c->piece->enabled = FALSE;
    }
    // The picked box in buffer pixels (dt_color_picker_box: average of the two lowest and
    // highest coordinates, at least one pixel, clamped to the buffer).
    int box[4] = {(int)(bx0 * roi.scale - roi.x), (int)(by0 * roi.scale - roi.y), (int)(bx1 * roi.scale - roi.x),
                  (int)(by1 * roi.scale - roi.y)};
    box[2] = MAX(box[2], box[0] + 1);
    box[3] = MAX(box[3], box[1] + 1);
    c->box_valid = !(box[0] >= roi.width || box[1] >= roi.height || box[2] < 0 || box[3] < 0);
    box[0] = CLAMP(box[0], 0, roi.width - 1);
    box[1] = CLAMP(box[1], 0, roi.height - 1);
    box[2] = CLAMP(box[2], 1, roi.width);
    box[3] = CLAMP(box[3], 1, roi.height);
    c->box_valid = c->box_valid && box[2] - box[0] >= 1 && box[3] - box[1] >= 1;
    memcpy(c->box, box, sizeof(box));
    return 0;
}

gboolean om_tool_pick(OmToolContext *ctx, int cst, gboolean denoise, OmPicked *picked) {
    OmCapture *c = ctx->capture;
    memset(picked, 0, sizeof(*picked));
    if (!c || !c->input || !c->box_valid)
        return FALSE;
    const dt_iop_order_iccprofile_info_t *profile = dt_ioppr_get_pipe_current_profile_info(ctx->module, &c->pipe);
    dt_color_picker_helper(&c->dsc, c->input, &c->roi, c->box, denoise, picked->in, c->dsc.cst, cst, profile);
    picked->valid = picked->in[DT_PICK_MAX][0] >= picked->in[DT_PICK_MIN][0];
    if (c->output) {
        dt_color_picker_helper(&c->out_dsc, c->output, &c->roi, c->box, denoise, picked->out, c->out_dsc.cst, cst,
                               profile);
        picked->output_valid = picked->out[DT_PICK_MAX][0] >= picked->out[DT_PICK_MIN][0];
    }
    return picked->valid;
}

uint32_t *om_tool_histogram(OmToolContext *ctx, int bins, uint32_t max[4]) {
    OmCapture *c = ctx->capture;
    if (!c || !c->input || c->dsc.channels != 4)
        return NULL;
    dt_histogram_roi_t roi = {c->roi.width, c->roi.height, 0, 0, 0, 0};
    dt_dev_histogram_collection_params_t params = {&roi, (uint32_t)bins};
    dt_dev_histogram_stats_t stats = {0};
    uint32_t *histogram = NULL;
    dt_histogram_helper(&params, &stats, c->dsc.cst, ctx->module->histogram_cst, c->input, &histogram, max,
                        ctx->module->histogram_middle_grey, c->work_profile);
    return histogram;
}

static void add_stats(JsonObject *object, const char *name, const dt_aligned_pixel_t value) {
    JsonArray *array = json_array_new();
    for (int k = 0; k < 4; ++k)
        json_array_add_double_element(array, isfinite(value[k]) ? value[k] : 0.0);
    json_object_set_array_member(object, name, array);
}

// The histogram tool: darktable's per-module histogram (levels, rgb levels), 256 bins.
static void report_histogram(OmToolContext *ctx) {
    uint32_t max[4] = {0};
    uint32_t *histogram = om_tool_histogram(ctx, 256, max);
    if (!histogram)
        return;
    JsonObject *out = json_object_new();
    JsonArray *channels = json_array_new();
    for (int ch = 0; ch < 4; ++ch) {
        JsonArray *bins = json_array_new();
        for (int i = 0; i < 256; ++i)
            json_array_add_int_element(bins, histogram[4 * i + ch]);
        json_array_add_array_element(channels, bins);
    }
    json_object_set_array_member(out, "channels", channels);
    JsonArray *maxima = json_array_new();
    for (int ch = 0; ch < 4; ++ch)
        json_array_add_int_element(maxima, max[ch]);
    json_object_set_array_member(out, "max", maxima);
    json_object_set_object_member(ctx->extra, "histogram", out);
    dt_free_align(histogram);
}

static gboolean parse_request(const char *json, JsonParser *parser, const char **tool, float box[4],
                              JsonObject **gui) {
    if (!json || !json_parser_load_from_data(parser, json, -1, NULL))
        return FALSE;
    JsonNode *root = json_parser_get_root(parser);
    if (!root || !JSON_NODE_HOLDS_OBJECT(root))
        return FALSE;
    JsonObject *object = json_node_get_object(root);
    *tool = json_object_get_string_member_with_default(object, "tool", NULL);
    if (!*tool)
        return FALSE;
    // darktable's default area (libs/lib.c, dt_lib_colorpicker_reset_box_area) and point.
    box[0] = box[1] = 0.02f;
    box[2] = box[3] = 0.98f;
    if (json_object_has_member(object, "box")) {
        JsonArray *array = json_object_get_array_member(object, "box");
        if (!array || json_array_get_length(array) != 4)
            return FALSE;
        for (int i = 0; i < 4; ++i)
            box[i] = CLAMP(json_array_get_double_element(array, i), 0.0, 1.0);
        if (box[0] > box[2]) {
            const float t = box[0];
            box[0] = box[2];
            box[2] = t;
        }
        if (box[1] > box[3]) {
            const float t = box[1];
            box[1] = box[3];
            box[3] = t;
        }
    }
    *gui = json_object_has_member(object, "gui") ? json_object_get_object_member(object, "gui") : NULL;
    return TRUE;
}

int om_engine_module_tool(OmEngine *engine, const char *operation, int instance, const char *request,
                          char **result) {
    *result = NULL;
    if (!engine->loaded)
        return 1;
    dt_iop_module_t *module = dt_iop_get_module_by_op_priority(engine->dev.iop, operation, instance);
    if (!module)
        return 2;
    JsonParser *parser = json_parser_new();
    const char *tool = NULL;
    OmToolContext ctx = {.engine = engine, .dev = &engine->dev, .module = module};
    if (!parse_request(request, parser, &tool, ctx.box, &ctx.gui)) {
        g_object_unref(parser);
        return 5;
    }
    ctx.spec = find_spec(operation, tool);
    if (!ctx.spec) {
        g_object_unref(parser);
        return 3;
    }
    if (ctx.spec->kind == OM_TOOL_POINT) {
        // A point picker samples one pixel at the box centre (or the point given).
        const float x = 0.5f * (ctx.box[0] + ctx.box[2]), y = 0.5f * (ctx.box[1] + ctx.box[3]);
        ctx.box[0] = ctx.box[2] = x;
        ctx.box[1] = ctx.box[3] = y;
    }
    ctx.gui_out = json_object_new();
    ctx.extra = json_object_new();
    ctx.touched = g_ptr_array_new_with_free_func(g_free);
    ctx.picker_cst = ctx.spec->cst >= 0 ? ctx.spec->cst : module->default_colorspace(module, NULL, NULL);
    int error = 0;
    const gboolean picker = ctx.spec->kind == OM_TOOL_AREA || ctx.spec->kind == OM_TOOL_POINT;
    if (picker || ctx.spec->kind == OM_TOOL_HISTOGRAM || (ctx.spec->flags & OM_TOOL_CAPTURE)) {
        error = capture(&ctx, (ctx.spec->flags & OM_TOOL_OUTPUT) != 0);
    }
    if (!error && picker && !om_tool_pick(&ctx, ctx.picker_cst, (ctx.spec->flags & OM_TOOL_DENOISE) != 0,
                                          &ctx.picked))
        error = 6;
    if (!error && ctx.spec->kind == OM_TOOL_HISTOGRAM)
        report_histogram(&ctx);
    if (!error && ctx.spec->run) {
        touch(&ctx, module, !(ctx.spec->flags & OM_TOOL_KEEP_OFF) && ctx.spec->kind != OM_TOOL_HISTOGRAM);
        error = ctx.spec->run(&ctx);
    }
    // One history item for every module the tool changed (or switched on), as the GUI's
    // dt_dev_add_history_item calls do.
    JsonArray *changed = json_array_new();
    for (guint i = 0; i < ctx.touched->len; ++i) {
        OmTouched *t = ctx.touched->pdata[i];
        const gboolean differs =
            memcmp(t->params, t->module->params, t->module->params_size) != 0 ||
            (t->module->blend_params && memcmp(&t->blend, t->module->blend_params, sizeof(t->blend)) != 0);
        if (!error && (differs || (t->force_enable && !t->enabled))) {
            dt_dev_add_history_item_ext(&engine->dev, t->module, t->force_enable ? TRUE : t->module->enabled, FALSE);
            json_array_add_string_element(changed, t->module->op);
        } else if (error && differs) {
            memcpy(t->module->params, t->params, t->module->params_size);
            if (t->module->blend_params)
                *t->module->blend_params = t->blend;
        }
        g_free(t->params);
    }
    if (json_array_get_length(changed)) {
        engine->dev.full.pipe->changed |= DT_DEV_PIPE_SYNCH;
        if (om_engine_bind_controls(engine) && !error)
            error = 4;
    }
    JsonObject *out = json_object_new();
    json_object_set_string_member(out, "tool", tool);
    json_object_set_array_member(out, "changed", changed);
    if (ctx.picked.valid) {
        JsonObject *picked = json_object_new();
        add_stats(picked, "mean", ctx.picked.in[DT_PICK_MEAN]);
        add_stats(picked, "min", ctx.picked.in[DT_PICK_MIN]);
        add_stats(picked, "max", ctx.picked.in[DT_PICK_MAX]);
        if (ctx.picked.output_valid)
            add_stats(picked, "output", ctx.picked.out[DT_PICK_MEAN]);
        json_object_set_int_member(picked, "colorspace", ctx.picker_cst);
        json_object_set_object_member(out, "picked", picked);
    }
    json_object_set_object_member(out, "gui", ctx.gui_out);
    GList *members = json_object_get_members(ctx.extra);
    for (GList *it = members; it; it = it->next)
        json_object_set_member(out, it->data, json_node_copy(json_object_get_member(ctx.extra, it->data)));
    g_list_free(members);
    json_object_unref(ctx.extra);
    JsonNode *node = json_node_new(JSON_NODE_OBJECT);
    json_node_take_object(node, out);
    *result = json_to_string(node, FALSE);
    json_node_unref(node);
    g_ptr_array_free(ctx.touched, TRUE);
    capture_free(ctx.capture);
    g_object_unref(parser);
    return error;
}
