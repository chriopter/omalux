// SPDX-License-Identifier: GPL-3.0-or-later
// On-canvas tools without GTK: coordinate spaces, the dispatcher, graduated density's line
// and rotate and perspective's straightening line and drawn structure.
//
// darktable's modules receive pointer positions as fractions of the preview pipe's processed
// size and convert them with dt_dev_distort_(back)transform_plus on dev->preview_pipe. The
// adapter has no preview pipe; its full pipe is processed at full input resolution (iscale 1)
// and scaled to the viewport by the output ROI, so its processed size and node transforms
// give the same geometry in full-resolution pixels. Each port below names its source in
// darktable 5.6.1 (src/iop/graduatednd.c, src/iop/ashift.c).
#include "canvas_internal.h"
#include "common/exif.h"
#include "common/history.h"
#include <math.h>

gboolean om_space_init(OmEngine *engine, OmSpace *space) {
    if (!engine->loaded)
        return FALSE;
    dt_dev_pixelpipe_t *pipe = engine->dev.full.pipe;
    if (!pipe || pipe->processed_width <= 0 || pipe->processed_height <= 0 || pipe->iwidth <= 0 ||
        pipe->iheight <= 0 || !pipe->nodes)
        return FALSE;
    space->dev = &engine->dev;
    space->pipe = pipe;
    space->width = pipe->processed_width;
    space->height = pipe->processed_height;
    space->iwidth = pipe->iwidth;
    space->iheight = pipe->iheight;
    return TRUE;
}

void om_space_from_preview(const OmSpace *space, float *points, int count) {
    for (int i = 0; i < count; ++i) {
        points[2 * i] *= space->width;
        points[2 * i + 1] *= space->height;
    }
}

void om_space_to_preview(const OmSpace *space, float *points, int count) {
    for (int i = 0; i < count; ++i) {
        points[2 * i] /= space->width;
        points[2 * i + 1] /= space->height;
    }
}

gboolean om_space_backtransform(const OmSpace *space, float *points, int count) {
    return count <= 0 || dt_dev_distort_backtransform_plus(space->dev, space->pipe, 0.0, DT_DEV_TRANSFORM_DIR_ALL,
                                                           points, count);
}

gboolean om_space_transform(const OmSpace *space, float *points, int count) {
    return count <= 0 ||
           dt_dev_distort_transform_plus(space->dev, space->pipe, 0.0, DT_DEV_TRANSFORM_DIR_ALL, points, count);
}

gboolean om_space_transform_dir(const OmSpace *space, double iop_order, int direction, gboolean back,
                                float *points, int count) {
    if (count <= 0)
        return TRUE;
    return back ? dt_dev_distort_backtransform_plus(space->dev, space->pipe, iop_order, direction, points, count)
                : dt_dev_distort_transform_plus(space->dev, space->pipe, iop_order, direction, points, count);
}

JsonObject *om_gesture_parse(const char *json, JsonParser **parser) {
    *parser = json_parser_new();
    if (!json || !json_parser_load_from_data(*parser, json, -1, NULL) || !json_parser_get_root(*parser) ||
        !JSON_NODE_HOLDS_OBJECT(json_parser_get_root(*parser)))
        return NULL;
    return json_node_get_object(json_parser_get_root(*parser));
}

static gboolean number_of(JsonNode *node, double *value) {
    if (!node || !JSON_NODE_HOLDS_VALUE(node))
        return FALSE;
    const GType type = json_node_get_value_type(node);
    if (type != G_TYPE_DOUBLE && type != G_TYPE_INT64)
        return FALSE;
    *value = json_node_get_double(node);
    return isfinite(*value);
}

gboolean om_gesture_point(JsonObject *gesture, const char *member, float *xy) {
    JsonNode *node = json_object_get_member(gesture, member);
    if (!node || !JSON_NODE_HOLDS_ARRAY(node))
        return FALSE;
    JsonArray *array = json_node_get_array(node);
    double x, y;
    if (json_array_get_length(array) != 2 || !number_of(json_array_get_element(array, 0), &x) ||
        !number_of(json_array_get_element(array, 1), &y))
        return FALSE;
    xy[0] = x;
    xy[1] = y;
    return TRUE;
}

double om_gesture_number(JsonObject *gesture, const char *member, double fallback) {
    double value;
    return number_of(json_object_get_member(gesture, member), &value) ? value : fallback;
}

static void add_number(JsonBuilder *builder, double value) {
    json_builder_add_double_value(builder, isfinite(value) ? value : 0.0);
}

void om_json_point(JsonBuilder *builder, const char *member, float x, float y) {
    if (member)
        json_builder_set_member_name(builder, member);
    json_builder_begin_array(builder);
    add_number(builder, x);
    add_number(builder, y);
    json_builder_end_array(builder);
}

// Points outside a sensible frame around the image (darktable's guide lines may extend far)
// are dropped; INFINITY separates sub-paths and is written as a break (null).
void om_json_polyline(JsonBuilder *builder, const char *member, const float *xy, int count) {
    json_builder_set_member_name(builder, member);
    json_builder_begin_array(builder);
    for (int i = 0; i < count; ++i) {
        const float x = xy[2 * i], y = xy[2 * i + 1];
        if (!isfinite(x) || !isfinite(y) || fabsf(x) > 4.f || fabsf(y) > 4.f) {
            json_builder_add_null_value(builder);
            continue;
        }
        om_json_point(builder, NULL, x, y);
    }
    json_builder_end_array(builder);
}

char *om_json_finish(JsonBuilder *builder) {
    JsonNode *root = json_builder_get_root(builder);
    char *text = json_to_string(root, FALSE);
    json_node_free(root);
    g_object_unref(builder);
    return text;
}

int om_canvas_commit(OmEngine *engine, dt_iop_module_t *module, gboolean masks) {
    if (masks)
        dt_dev_add_masks_history_item_ext(&engine->dev, module, TRUE, FALSE);
    else
        dt_dev_add_history_item_ext(&engine->dev, module, TRUE, FALSE);
    engine->dev.full.pipe->changed |= DT_DEV_PIPE_SYNCH;
    return om_engine_bind_controls(engine);
}

static float *float_field(dt_iop_module_t *module, const char *name, float *minimum, float *maximum) {
    const dt_introspection_field_t *field = module->get_f ? module->get_f(name) : NULL;
    if (!field || field->header.type != DT_INTROSPECTION_TYPE_FLOAT)
        return NULL;
    if (minimum)
        *minimum = field->Float.Min;
    if (maximum)
        *maximum = field->Float.Max;
    return module->get_p(module->params, name);
}

// ---- graduated density (src/iop/graduatednd.c) -------------------------------------------

// _set_grad_from_points (graduatednd.c:206): the rotation is found by bisection, the offset
// follows from it. Points are fractions of the module's output buffer.
static int gradient_from_points(const float *pts, float *rotation, float *offset) {
    float v1 = -M_PI_F;
    float v2 = M_PI_F;
    float sinv, cosv, r1, r2, v, r;
    sinv = sinf(v1), cosv = cosf(v1);
    r1 = pts[1] * cosv - pts[0] * sinv + pts[2] * sinv - pts[3] * cosv;
    const float pas = M_PI_F / 16.f;
    do {
        v2 += pas;
        sinv = sinf(v2), cosv = cosf(v2);
        r2 = pts[1] * cosv - pts[0] * sinv + pts[2] * sinv - pts[3] * cosv;
        if (r1 * r2 < 0)
            break;
    } while (v2 <= M_PI_F);
    if (v2 == M_PI_F)
        return 9;
    const float eps = .0001f;
    int iter = 0;
    do {
        v = (v1 + v2) / 2.0;
        sinv = sinf(v), cosv = cosf(v);
        r = pts[1] * cosv - pts[0] * sinv + pts[2] * sinv - pts[3] * cosv;
        if (r < eps && r > -eps)
            break;
        if (r * r2 < 0)
            v1 = v;
        else {
            r2 = r;
            v2 = v;
        }
    } while (iter++ < 1000);
    if (iter >= 1000)
        return 8;
    const float diff_x = pts[2] - pts[0];
    if (diff_x > eps) {
        if (v >= M_PI_2f)
            v -= M_PI_F;
        if (v < -M_PI_2f)
            v += M_PI_F;
    } else if (diff_x < -eps) {
        if (v < M_PI_2f && v >= 0)
            v -= M_PI_F;
        if (v > -M_PI_2f && v < 0)
            v += M_PI_F;
    } else {
        const float diff_y = pts[3] - pts[1];
        if (diff_y <= 0.0f)
            v = -M_PI_2f;
        else
            v = M_PI_2f;
    }
    *rotation = rad2degf(-v);
    sinv = sinf(v);
    cosv = cosf(v);
    const float ofs = (-2.0f * sinv * pts[0]) + sinv - cosv + 1.0f + (2.0f * cosv * pts[1]);
    *offset = ofs * 50.0f;
    return 1;
}

// _set_points_from_grad (graduatednd.c:307): the visible part of the line, 10 % in from the
// borders, in module output pixels.
static void points_from_gradient(float wp, float hp, float rotation, float offset, float *pts) {
    const float v = deg2radf(-rotation);
    const float sinv = sinf(v);
    if (sinv == 0.0f) {
        if (rotation == 0.0f) {
            pts[0] = wp * 0.1f;
            pts[2] = wp * 0.9f;
            pts[1] = pts[3] = hp * offset / 100.0f;
        } else {
            pts[2] = wp * 0.1f;
            pts[0] = wp * 0.9f;
            pts[1] = pts[3] = hp * (1.0f - offset / 100.0f);
        }
    } else if (fabsf(sinv) == 1) {
        if (rotation == 90) {
            pts[0] = pts[2] = wp * offset / 100.0f;
            pts[3] = hp * 0.1f;
            pts[1] = hp * 0.9f;
        } else {
            pts[0] = pts[2] = wp * (1.0 - offset / 100.0f);
            pts[1] = hp * 0.1f;
            pts[3] = hp * 0.9f;
        }
    } else {
        const float cosv = cosf(v);
        float xx1 = (sinv - cosv + 1.0f - offset / 50.0f) * wp * 0.5f / sinv;
        float xx2 = (sinv + cosv + 1.0f - offset / 50.0f) * wp * 0.5f / sinv;
        float yy1 = 0.0f;
        float yy2 = hp;
        const float a = hp / (xx2 - xx1);
        const float b = -xx1 * a;
        if (xx2 > wp) {
            yy2 = a * wp + b;
            xx2 = wp;
        }
        if (xx2 < 0) {
            yy2 = b;
            xx2 = 0;
        }
        if (xx1 > wp) {
            yy1 = a * wp + b;
            xx1 = wp;
        }
        if (xx1 < 0) {
            yy1 = b;
            xx1 = 0;
        }
        xx2 -= (xx2 - xx1) * 0.1;
        xx1 += (xx2 - xx1) * 0.1;
        yy2 -= (yy2 - yy1) * 0.1;
        yy1 += (yy2 - yy1) * 0.1;
        const gboolean ordered = rotation < 90.0f && rotation > -90.0f ? xx1 < xx2 : xx2 < xx1;
        if (ordered) {
            pts[0] = xx1;
            pts[1] = yy1;
            pts[2] = xx2;
            pts[3] = yy2;
        } else {
            pts[2] = xx1;
            pts[3] = yy1;
            pts[0] = xx2;
            pts[1] = yy2;
        }
    }
}

static void graduatednd_overlay(const OmSpace *space, dt_iop_module_t *module, JsonBuilder *builder) {
    const dt_dev_pixelpipe_iop_t *piece = dt_dev_distort_get_iop_pipe(space->dev, space->pipe, module);
    float *rotation = float_field(module, "rotation", NULL, NULL);
    float *offset = float_field(module, "offset", NULL, NULL);
    if (!piece || !rotation || !offset)
        return;
    float pts[4];
    points_from_gradient(piece->buf_out.width, piece->buf_out.height, *rotation, *offset, pts);
    if (!om_space_transform_dir(space, module->iop_order, DT_DEV_TRANSFORM_DIR_FORW_EXCL, FALSE, pts, 2))
        return;
    om_space_to_preview(space, pts, 2);
    json_builder_set_member_name(builder, "line");
    json_builder_begin_object(builder);
    om_json_point(builder, "a", pts[0], pts[1]);
    om_json_point(builder, "b", pts[2], pts[3]);
    json_builder_end_object(builder);
}

// button_released (graduatednd.c:670): a dragged end point or a new line sets rotation and
// offset; dragging the whole line keeps the rotation.
static int graduatednd_edit(OmEngine *engine, const OmSpace *space, dt_iop_module_t *module, JsonObject *gesture) {
    if (g_strcmp0(json_object_get_string_member_with_default(gesture, "action", ""), "line"))
        return 3;
    float pts[4], minimum = 0, maximum = 100;
    if (!om_gesture_point(gesture, "a", pts) || !om_gesture_point(gesture, "b", pts + 2))
        return 5;
    if (hypotf((pts[2] - pts[0]) * space->width, (pts[3] - pts[1]) * space->height) < 1.f)
        return 5;
    float *rotation = float_field(module, "rotation", NULL, NULL);
    float *offset = float_field(module, "offset", &minimum, &maximum);
    const dt_dev_pixelpipe_iop_t *piece = dt_dev_distort_get_iop_pipe(space->dev, space->pipe, module);
    if (!rotation || !offset || !piece)
        return 4;
    om_space_from_preview(space, pts, 2);
    if (!om_space_transform_dir(space, module->iop_order, DT_DEV_TRANSFORM_DIR_FORW_EXCL, TRUE, pts, 2))
        return 4;
    pts[0] /= (float)piece->buf_out.width;
    pts[2] /= (float)piece->buf_out.width;
    pts[1] /= (float)piece->buf_out.height;
    pts[3] /= (float)piece->buf_out.height;
    float r = 0.0f, o = 0.0f;
    if (gradient_from_points(pts, &r, &o) != 1)
        return 5;
    if (!json_object_get_boolean_member_with_default(gesture, "keepRotation", FALSE))
        *rotation = CLAMP(r, -180.f, 180.f);
    *offset = CLAMP(o, minimum, maximum);
    return om_canvas_commit(engine, module, FALSE);
}

// ---- rotate and perspective (src/iop/ashift.c) --------------------------------------------

enum { OM_ASHIFT_MAX_SAVED_LINES = 50 };

static void ashift_overlay(const OmSpace *space, dt_iop_module_t *module, JsonBuilder *builder) {
    float *lines = module->get_p(module->params, "last_drawn_lines");
    int *count = module->get_p(module->params, "last_drawn_lines_count");
    float *quad = module->get_p(module->params, "last_quad_lines");
    json_builder_set_member_name(builder, "lines");
    json_builder_begin_array(builder);
    if (lines && count && *count > 0) {
        const int n = MIN(*count, OM_ASHIFT_MAX_SAVED_LINES);
        float *pts = g_memdup2(lines, sizeof(float) * 4 * n);
        if (om_space_transform(space, pts, 2 * n)) {
            om_space_to_preview(space, pts, 2 * n);
            for (int i = 0; i < n; ++i) {
                json_builder_begin_array(builder);
                for (int k = 0; k < 4; ++k)
                    add_number(builder, pts[4 * i + k]);
                json_builder_end_array(builder);
            }
        }
        g_free(pts);
    }
    json_builder_end_array(builder);
    // _draw_retrieve_lines_from_params (ashift.c:3071): a quad is stored only once all four
    // leading coordinates are positive.
    json_builder_set_member_name(builder, "quad");
    if (quad && quad[0] > 0.f && quad[1] > 0.f && quad[2] > 0.f && quad[3] > 0.f) {
        // Stored as the left line (top, bottom) and the right line (top, bottom).
        float pts[8] = {quad[0], quad[1], quad[4], quad[5], quad[6], quad[7], quad[2], quad[3]};
        if (om_space_transform(space, pts, 4)) {
            om_space_to_preview(space, pts, 4);
            json_builder_begin_array(builder);
            for (int i = 0; i < 4; ++i)
                om_json_point(builder, NULL, pts[2 * i], pts[2 * i + 1]);
            json_builder_end_array(builder);
        } else
            json_builder_add_null_value(builder);
    } else
        json_builder_add_null_value(builder);
}

// _calculate_straightening (ashift.c:4070), without the 25 px minimum length, which the
// caller applies in screen pixels.
static float straightening_angle(const OmSpace *space, dt_iop_module_t *module, const float *a, const float *b) {
    float dx = (b[0] - a[0]) * space->width, dy = (b[1] - a[1]) * space->height;
    float pts[4] = {0, 0, dx, dy};
    om_space_transform_dir(space, module->iop_order, DT_DEV_TRANSFORM_DIR_FORW_EXCL, TRUE, pts, 2);
    dx = pts[0] - pts[2];
    dy = pts[1] - pts[3];
    if (dx < 0) {
        dx = -dx;
        dy = -dy;
    }
    const float angle = atan2f(dy, dx);
    if (!(angle >= -M_PI_2f && angle <= M_PI_2f))
        return 0.0f;
    float close = angle;
    if (close > M_PI_4f)
        close = M_PI_2f - close;
    else if (close < -M_PI_4f)
        close = -M_PI_2f - close;
    else
        close = -close;
    float result = rad2degf(close);
    if (result < -180.f)
        result += 360.f;
    if (result > 180.f)
        result -= 360.f;
    return result;
}

static int ashift_edit(OmEngine *engine, const OmSpace *space, dt_iop_module_t *module, JsonObject *gesture) {
    const char *action = json_object_get_string_member_with_default(gesture, "action", "");
    float minimum = -180, maximum = 180;
    if (!strcmp(action, "straighten")) {
        // button_released (ashift.c:5060): rotation minus the angle of the drawn line.
        float a[2], b[2];
        float *rotation = float_field(module, "rotation", &minimum, &maximum);
        if (!rotation || !om_gesture_point(gesture, "a", a) || !om_gesture_point(gesture, "b", b))
            return 5;
        const float angle = straightening_angle(space, module, a, b);
        if (angle == 0.0f)
            return 0;
        *rotation = CLAMP(*rotation - angle, minimum, maximum);
        return om_canvas_commit(engine, module, FALSE);
    }
    float *lines = module->get_p(module->params, "last_drawn_lines");
    int *count = module->get_p(module->params, "last_drawn_lines_count");
    float *quad = module->get_p(module->params, "last_quad_lines");
    if (!lines || !count || !quad)
        return 4;
    if (!strcmp(action, "lines")) {
        // _draw_save_lines_to_params (ashift.c:3008): line ends in original image pixels.
        JsonNode *node = json_object_get_member(gesture, "lines");
        if (!node || !JSON_NODE_HOLDS_ARRAY(node))
            return 5;
        JsonArray *array = json_node_get_array(node);
        const int n = MIN((int)json_array_get_length(array), OM_ASHIFT_MAX_SAVED_LINES);
        float pts[OM_ASHIFT_MAX_SAVED_LINES * 4];
        for (int i = 0; i < n; ++i) {
            JsonNode *line = json_array_get_element(array, i);
            if (!JSON_NODE_HOLDS_ARRAY(line) || json_array_get_length(json_node_get_array(line)) != 4)
                return 5;
            for (int k = 0; k < 4; ++k) {
                JsonNode *value = json_array_get_element(json_node_get_array(line), k);
                if (!JSON_NODE_HOLDS_VALUE(value))
                    return 5;
                pts[4 * i + k] = json_node_get_double(value);
            }
        }
        om_space_from_preview(space, pts, 2 * n);
        if (!om_space_backtransform(space, pts, 2 * n))
            return 4;
        memset(lines, 0, sizeof(float) * 4 * OM_ASHIFT_MAX_SAVED_LINES);
        memcpy(lines, pts, sizeof(float) * 4 * n);
        *count = n;
        return om_canvas_commit(engine, module, FALSE);
    }
    if (!strcmp(action, "quad")) {
        // Corners top-left, top-right, bottom-right, bottom-left; darktable keeps the two
        // vertical lines (ashift.c:3018) and derives the horizontal ones from their ends.
        float c[8];
        if (!om_gesture_point(gesture, "topLeft", c) || !om_gesture_point(gesture, "topRight", c + 2) ||
            !om_gesture_point(gesture, "bottomRight", c + 4) || !om_gesture_point(gesture, "bottomLeft", c + 6))
            return 5;
        float pts[8] = {c[0], c[1], c[6], c[7], c[2], c[3], c[4], c[5]};
        om_space_from_preview(space, pts, 4);
        if (!om_space_backtransform(space, pts, 4))
            return 4;
        memcpy(quad, pts, sizeof(pts));
        return om_canvas_commit(engine, module, FALSE);
    }
    if (!strcmp(action, "clear-structure")) {
        memset(lines, 0, sizeof(float) * 4 * OM_ASHIFT_MAX_SAVED_LINES);
        *count = 0;
        memset(quad, 0, sizeof(float) * 8);
        return om_canvas_commit(engine, module, FALSE);
    }
    return 3;
}

// ---- dispatch -----------------------------------------------------------------------------

static dt_iop_module_t *canvas_module(OmEngine *engine, const char *operation, int instance) {
    return engine->loaded && operation ? dt_iop_get_module_by_op_priority(engine->dev.iop, operation, instance)
                                       : NULL;
}

char *om_engine_canvas(OmEngine *engine, const char *operation, int instance) {
    dt_iop_module_t *module = canvas_module(engine, operation, instance);
    OmSpace space;
    if (!module || !om_space_init(engine, &space))
        return NULL;
    const gboolean line = !strcmp(module->op, "graduatednd"), ashift = !strcmp(module->op, "ashift"),
                   liquify = !strcmp(module->op, "liquify");
    if (!line && !ashift && !liquify && !om_shapes_module(module))
        return NULL;
    JsonBuilder *builder = json_builder_new();
    json_builder_begin_object(builder);
    json_builder_set_member_name(builder, "operation");
    json_builder_add_string_value(builder, module->op);
    json_builder_set_member_name(builder, "instance");
    json_builder_add_int_value(builder, module->multi_priority);
    json_builder_set_member_name(builder, "enabled");
    json_builder_add_boolean_value(builder, module->enabled);
    json_builder_set_member_name(builder, "width");
    json_builder_add_double_value(builder, space.width);
    json_builder_set_member_name(builder, "height");
    json_builder_add_double_value(builder, space.height);
    json_builder_set_member_name(builder, "kind");
    json_builder_add_string_value(builder, line ? "line" : ashift ? "ashift" : liquify ? "liquify" : "shapes");
    if (line)
        graduatednd_overlay(&space, module, builder);
    else if (ashift)
        ashift_overlay(&space, module, builder);
    else if (liquify)
        om_liquify_overlay(engine, &space, module, builder);
    else
        om_shapes_overlay(engine, &space, module, builder);
    json_builder_end_object(builder);
    return om_json_finish(builder);
}

int om_engine_canvas_edit(OmEngine *engine, const char *operation, int instance, const char *json) {
    if (!engine->loaded)
        return 1;
    dt_iop_module_t *module = canvas_module(engine, operation, instance);
    if (!module)
        return 2;
    OmSpace space;
    if (!om_space_init(engine, &space))
        return 4;
    JsonParser *parser = NULL;
    JsonObject *gesture = om_gesture_parse(json, &parser);
    int result = 5;
    if (gesture) {
        if (!strcmp(module->op, "graduatednd"))
            result = graduatednd_edit(engine, &space, module, gesture);
        else if (!strcmp(module->op, "ashift"))
            result = ashift_edit(engine, &space, module, gesture);
        else if (!strcmp(module->op, "liquify"))
            result = om_liquify_edit(engine, &space, module, gesture);
        else if (om_shapes_module(module))
            result = om_shapes_edit(engine, &space, module, gesture);
        else
            result = 4;
    }
    g_object_unref(parser);
    return result;
}

// darktable's piece hash takes drawn forms from the process-global darktable.develop
// (develop/imageop.c:2235, dt_iop_commit_params), not from the develop context being
// processed, and only while a mask mode is set; retouch and spot removal use their forms
// without one. A change of the worker's forms therefore leaves the hash, and the cached
// output of the module, unchanged. Invalidate from the first module using a mask group.
void om_engine_canvas_before_render(OmEngine *engine) {
    if (!engine->loaded)
        return;
    const guint64 hash = om_shapes_hash(engine);
    if (hash == engine->canvas_forms_hash)
        return;
    engine->canvas_forms_hash = hash;
    double first = -1.0;
    for (GList *it = engine->dev.iop; it; it = it->next) {
        dt_iop_module_t *module = it->data;
        if (module->blend_params && dt_is_valid_maskid(module->blend_params->mask_id) &&
            (first < 0 || module->iop_order < first))
            first = module->iop_order;
    }
    dt_dev_pixelpipe_cache_invalidate_later(engine->dev.full.pipe, first < 0 ? 0 : (int32_t)first,
                                            "omalux drawn shapes: ");
}

void om_engine_canvas_parameters_changed(OmEngine *engine, void *module) {
    om_shapes_selection_changed(engine, module);
}

int om_engine_canvas_uses_forms(OmEngine *engine, const char *operation) {
    if (!engine->loaded || !engine->dev.forms || !operation)
        return 0;
    if (!strcmp(operation, "*"))
        return 1;
    for (GList *it = engine->dev.iop; it; it = it->next) {
        const dt_iop_module_t *module = it->data;
        if (!strcmp(module->op, operation) && module->blend_params &&
            dt_masks_get_from_id(&engine->dev, module->blend_params->mask_id))
            return 1;
    }
    return 0;
}

// The comparison process applies this with image:apply_sidecar (lua/image.c:96), which
// replaces its history with the one written here, drawn forms included.
int om_engine_canvas_sidecar(OmEngine *engine, const char *path) {
    if (!engine->loaded || !path)
        return 1;
    dt_dev_write_history_ext(&engine->dev, engine->dev.image_storage.id);
    return dt_exif_xmp_write(engine->dev.image_storage.id, path, TRUE) ? 2 : 0;
}
