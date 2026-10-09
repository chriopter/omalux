// SPDX-License-Identifier: GPL-3.0-or-later
// darktable's drawn shapes (develop/masks: circle, ellipse, path, brush, gradient) for
// retouch, spot removal and the drawn masks of a module's blend section, without GTK.
//
// darktable creates and edits forms in its mouse handlers against the process-global
// darktable.develop and its preview pipe (develop/masks/*.c). Those handlers cannot run
// headless, so the parts that decide the stored values are ported here and applied to the
// worker's develop context: creation defaults from darktable's configuration, the numbering
// and group bookkeeping of dt_masks_gui_form_save_creation (develop/masks/masks.c:336), the
// scroll steps of each shape and the parameter resynchronisation of retouch and spots.
// Stored coordinates are fractions of the pipe's input size, as darktable stores them.
#include "canvas_internal.h"
#include "masks_api.h"
#include <math.h>

#define OM_HARDNESS_MIN 0.0005f // develop/masks/brush.c:30
#define OM_HARDNESS_MAX 1.0f
#define OM_BORDER_MIN 0.00005f
#define OM_BORDER_MAX 0.5f
#define OM_MIN_CIRCLE_BORDER 0.0005f // develop/masks/circle.c:30

static gboolean is_op(const dt_iop_module_t *module, const char *operation) {
    return module && !strcmp(module->op, operation);
}

gboolean om_shapes_module(const dt_iop_module_t *module) {
    return is_op(module, "retouch") || is_op(module, "spots") ||
           (module->flags() & IOP_FLAGS_SUPPORTS_BLENDING);
}

static dt_masks_form_t *module_group(OmEngine *engine, const dt_iop_module_t *module) {
    if (!module->blend_params)
        return NULL;
    dt_masks_form_t *group = dt_masks_get_from_id(&engine->dev, module->blend_params->mask_id);
    return group && (group->type & DT_MASKS_GROUP) ? group : NULL;
}

static dt_masks_point_group_t *group_point(dt_masks_form_t *group, dt_mask_id_t id) {
    for (GList *it = group ? group->points : NULL; it; it = it->next)
        if (((dt_masks_point_group_t *)it->data)->formid == id)
            return it->data;
    return NULL;
}

guint64 om_shapes_hash(OmEngine *engine) {
    dt_hash_t hash = DT_INITHASH;
    for (GList *it = engine->dev.forms; it; it = it->next) {
        const dt_masks_form_t *form = it->data;
        hash = dt_hash(hash, &form->type, sizeof(form->type));
        hash = dt_hash(hash, &form->formid, sizeof(form->formid));
        hash = dt_hash(hash, form->source, sizeof(form->source));
        const size_t size = (form->type & DT_MASKS_GROUP) ? sizeof(dt_masks_point_group_t)
                            : form->functions                ? (size_t)form->functions->point_struct_size
                                                             : 0;
        for (GList *p = form->points; p && size; p = p->next)
            hash = dt_hash(hash, p->data, size);
    }
    for (GList *it = engine->dev.iop; it; it = it->next) {
        const dt_iop_module_t *module = it->data;
        if (module->blend_params)
            hash = dt_hash(hash, &module->blend_params->mask_id, sizeof(dt_mask_id_t));
    }
    return hash;
}

// ---- retouch and spots parameters ------------------------------------------------------------

// rt_forms through introspection, so a changed struct layout is detected rather than misread.
typedef struct {
    char *base;
    const dt_introspection_field_t *item;
    int count;
} OmFormTable;

static gboolean retouch_table(dt_iop_module_t *module, OmFormTable *table) {
    const dt_introspection_field_t *array = module->get_f ? module->get_f("rt_forms") : NULL;
    if (!array || array->header.type != DT_INTROSPECTION_TYPE_ARRAY ||
        array->Array.field->header.type != DT_INTROSPECTION_TYPE_STRUCT)
        return FALSE;
    table->base = module->get_p(module->params, "rt_forms");
    table->item = array->Array.field;
    table->count = array->Array.count;
    return table->base != NULL;
}

static void *table_member(const OmFormTable *table, int index, const char *name) {
    for (dt_introspection_field_t **child = table->item->Struct.fields; child && *child; ++child)
        if (!strcmp((*child)->header.field_name, name))
            return table->base + (size_t)index * table->item->header.size +
                   ((*child)->header.offset - table->item->header.offset);
    return NULL;
}

static int *table_int(const OmFormTable *table, int index, const char *name) {
    return table_member(table, index, name);
}

static float *table_float(const OmFormTable *table, int index, const char *name) {
    return table_member(table, index, name);
}

static int retouch_index(const OmFormTable *table, dt_mask_id_t id) {
    if (!dt_is_valid_maskid(id))
        return -1;
    for (int i = 0; i < table->count; ++i)
        if (*table_int(table, i, "formid") == id)
            return i;
    return -1;
}

static int *param_int(dt_iop_module_t *module, const char *name) {
    return module->get_p(module->params, name);
}

static float *param_float(dt_iop_module_t *module, const char *name) {
    return module->get_p(module->params, name);
}

enum { OM_RT_CLONE = 1, OM_RT_HEAL = 2, OM_RT_BLUR = 3, OM_RT_FILL = 4 };

// rt_resynch_params (iop/retouch.c:739): one entry per shape of the group, in group order;
// new shapes take the module's current algorithm and its settings.
static void retouch_resynch(OmEngine *engine, dt_iop_module_t *module) {
    OmFormTable table;
    if (!retouch_table(module, &table))
        return;
    const size_t bytes = (size_t)table.count * table.item->header.size;
    char *previous = g_memdup2(table.base, bytes);
    OmFormTable old = table;
    old.base = previous;
    memset(table.base, 0, bytes);
    dt_masks_form_t *group = module_group(engine, module);
    int next = 0;
    for (GList *it = group ? group->points : NULL; it && next < table.count; it = it->next) {
        const dt_mask_id_t id = ((dt_masks_point_group_t *)it->data)->formid;
        const int index = retouch_index(&old, id);
        if (index >= 0) {
            memcpy(table.base + (size_t)next * table.item->header.size,
                   previous + (size_t)index * table.item->header.size, table.item->header.size);
            ++next;
        } else if (dt_masks_get_from_id(&engine->dev, id)) {
            *table_int(&table, next, "formid") = id;
            *table_int(&table, next, "scale") = *param_int(module, "curr_scale");
            const int algorithm = *param_int(module, "algorithm");
            *table_int(&table, next, "algorithm") = algorithm;
            *table_int(&table, next, "distort_mode") = 2;
            if (algorithm == OM_RT_BLUR) {
                *table_int(&table, next, "blur_type") = *param_int(module, "blur_type");
                *table_float(&table, next, "blur_radius") = *param_float(module, "blur_radius");
            } else if (algorithm == OM_RT_FILL) {
                *table_int(&table, next, "fill_mode") = *param_int(module, "fill_mode");
                memcpy(table_float(&table, next, "fill_color"), param_float(module, "fill_color"),
                       3 * sizeof(float));
                *table_float(&table, next, "fill_brightness") = *param_float(module, "fill_brightness");
            }
            ++next;
        }
    }
    g_free(previous);
}

// _resynch_params (iop/spots.c:183), including its initialiser that gives only the first
// new entry algorithm 2.
static void spots_resynch(OmEngine *engine, dt_iop_module_t *module) {
    int *ids = param_int(module, "clone_id"), *algorithms = param_int(module, "clone_algo");
    if (!ids || !algorithms)
        return;
    int nid[64] = {0};
    int nalgo[64] = {2};
    dt_masks_form_t *group = module_group(engine, module);
    int i = 0;
    for (GList *it = group ? group->points : NULL; i < 64 && it; it = it->next) {
        nid[i] = ((dt_masks_point_group_t *)it->data)->formid;
        for (int j = 0; j < 64; ++j)
            if (ids[j] == nid[i]) {
                nalgo[i] = algorithms[j];
                break;
            }
        ++i;
    }
    memcpy(algorithms, nalgo, sizeof(nalgo));
    memcpy(ids, nid, sizeof(nid));
}

static void resynch(OmEngine *engine, dt_iop_module_t *module) {
    if (is_op(module, "retouch"))
        retouch_resynch(engine, module);
    else if (is_op(module, "spots"))
        spots_resynch(engine, module);
}

// rt_shape_selection_changed (iop/retouch.c:507): the selected shape's settings become the
// module's own fields. Returns TRUE when a field changed.
static gboolean retouch_show_selection(OmEngine *engine, dt_iop_module_t *module) {
    OmFormTable table;
    const int index = retouch_table(module, &table) ? retouch_index(&table, engine->dev.mask_form_selected_id) : -1;
    if (index < 0)
        return FALSE;
    const void *before = g_memdup2(module->params, module->params_size);
    const int algorithm = *table_int(&table, index, "algorithm");
    if (algorithm == OM_RT_BLUR) {
        *param_int(module, "blur_type") = *table_int(&table, index, "blur_type");
        *param_float(module, "blur_radius") = *table_float(&table, index, "blur_radius");
    } else if (algorithm == OM_RT_FILL) {
        *param_int(module, "fill_mode") = *table_int(&table, index, "fill_mode");
        *param_float(module, "fill_brightness") = *table_float(&table, index, "fill_brightness");
        memcpy(param_float(module, "fill_color"), table_float(&table, index, "fill_color"), 3 * sizeof(float));
    }
    *param_int(module, "algorithm") = algorithm;
    const gboolean changed = memcmp(before, module->params, module->params_size) != 0;
    g_free((void *)before);
    return changed;
}

// gui_changed (iop/retouch.c:2087) and the colour button (retouch.c:1098): editing the
// module's blur or fill fields edits the selected shape of that algorithm.
void om_shapes_selection_changed(OmEngine *engine, dt_iop_module_t *module) {
    OmFormTable table;
    if (!engine->loaded || !is_op(module, "retouch") || !retouch_table(module, &table))
        return;
    const int index = retouch_index(&table, engine->dev.mask_form_selected_id);
    if (index < 0)
        return;
    const int algorithm = *table_int(&table, index, "algorithm");
    if (algorithm == OM_RT_BLUR) {
        *table_int(&table, index, "blur_type") = *param_int(module, "blur_type");
        *table_float(&table, index, "blur_radius") = *param_float(module, "blur_radius");
    } else if (algorithm == OM_RT_FILL) {
        *table_int(&table, index, "fill_mode") = *param_int(module, "fill_mode");
        *table_float(&table, index, "fill_brightness") = *param_float(module, "fill_brightness");
        memcpy(table_float(&table, index, "fill_color"), param_float(module, "fill_color"), 3 * sizeof(float));
    }
}

// ---- outlines ----------------------------------------------------------------------------------

typedef struct {
    float *xy;
    int count, capacity;
} OmPoly;

static void poly_add(OmPoly *poly, float x, float y) {
    if (poly->count == poly->capacity) {
        poly->capacity = poly->capacity ? 2 * poly->capacity : 64;
        poly->xy = g_renew(float, poly->xy, 2 * poly->capacity);
    }
    poly->xy[2 * poly->count] = x;
    poly->xy[2 * poly->count + 1] = y;
    ++poly->count;
}

static void poly_break(OmPoly *poly) {
    poly_add(poly, INFINITY, INFINITY);
}

static float min_side(const OmSpace *space) {
    return MIN(space->iwidth, space->iheight);
}

// _points_to_transform (develop/masks/circle.c:671), in input pixels.
static void circle_outline(const OmSpace *space, float cx, float cy, float radius, OmPoly *poly) {
    const float r = radius * min_side(space);
    const int n = 72;
    for (int i = 0; i <= n; ++i) {
        const float alpha = i * DT_2PI_F / n;
        poly_add(poly, cx * space->iwidth + r * cosf(alpha), cy * space->iheight + r * sinf(alpha));
    }
}

// _points_to_transform (develop/masks/ellipse.c:219), without the guide points.
static void ellipse_outline(const OmSpace *space, float cx, float cy, float ra, float rb, float rotation,
                            OmPoly *poly) {
    const float m = min_side(space);
    float a, b, v;
    if (ra >= rb) {
        a = ra * m;
        b = rb * m;
        v = deg2radf(rotation);
    } else {
        a = rb * m;
        b = ra * m;
        v = deg2radf(rotation - 90.0f);
    }
    const float sinv = sinf(v), cosv = cosf(v), x = cx * space->iwidth, y = cy * space->iheight;
    const int n = 90;
    for (int i = 0; i <= n; ++i) {
        const float alpha = i * DT_2PI_F / n;
        poly_add(poly, x + a * cosf(alpha) * cosv - b * sinf(alpha) * sinv,
                 y + a * cosf(alpha) * sinv + b * sinf(alpha) * cosv);
    }
}

static void bezier(const float *p0, const float *p1, const float *p2, const float *p3, float t, float *out) {
    const float u = 1.f - t;
    for (int k = 0; k < 2; ++k)
        out[k] = u * u * u * p0[k] + 3 * u * u * t * p1[k] + 3 * u * t * t * p2[k] + t * t * t * p3[k];
}

// Path and brush points share corner, ctrl1, ctrl2 and border at the same offsets.
typedef struct {
    float corner[2], ctrl1[2], ctrl2[2], border[2];
} OmBezierPoint;

static void bezier_point(const dt_masks_form_t *form, const void *data, OmBezierPoint *out) {
    if (form->type & DT_MASKS_PATH) {
        const dt_masks_point_path_t *p = data;
        memcpy(out->corner, p->corner, sizeof(out->corner));
        memcpy(out->ctrl1, p->ctrl1, sizeof(out->ctrl1));
        memcpy(out->ctrl2, p->ctrl2, sizeof(out->ctrl2));
        memcpy(out->border, p->border, sizeof(out->border));
    } else {
        const dt_masks_point_brush_t *p = data;
        memcpy(out->corner, p->corner, sizeof(out->corner));
        memcpy(out->ctrl1, p->ctrl1, sizeof(out->ctrl1));
        memcpy(out->ctrl2, p->ctrl2, sizeof(out->ctrl2));
        memcpy(out->border, p->border, sizeof(out->border));
    }
}

// The Bézier spine of a path (closed) or brush stroke (open) and, beside it, the feather
// (path) or stroke width (brush) as offset lines: the fallback when darktable's own border
// (form_dt_border, shape_nodes.inc) cannot be computed.
static void spline_outline(const OmSpace *space, const dt_masks_form_t *form, OmPoly *line, OmPoly *border) {
    const int n = g_list_length(form->points);
    if (n < 2)
        return;
    const gboolean closed = (form->type & DT_MASKS_PATH) != 0;
    OmBezierPoint *points = g_new(OmBezierPoint, n);
    int i = 0;
    for (GList *it = form->points; it; it = it->next)
        bezier_point(form, it->data, &points[i++]);
    const float sx = space->iwidth, sy = space->iheight, m = min_side(space);
    const int steps = 16, segments = closed ? n : n - 1;
    OmPoly samples = {0};
    float *widths = g_new(float, segments * steps + 1);
    for (int s = 0; s < segments; ++s) {
        const OmBezierPoint *a = &points[s], *b = &points[(s + 1) % n];
        const float p0[2] = {a->corner[0] * sx, a->corner[1] * sy}, p1[2] = {a->ctrl2[0] * sx, a->ctrl2[1] * sy},
                    p2[2] = {b->ctrl1[0] * sx, b->ctrl1[1] * sy}, p3[2] = {b->corner[0] * sx, b->corner[1] * sy};
        for (int k = 0; k < steps + (s == segments - 1 ? 1 : 0); ++k) {
            const float t = (float)k / steps;
            float q[2];
            bezier(p0, p1, p2, p3, t, q);
            widths[samples.count] = ((1 - t) * 0.5f * (a->border[0] + a->border[1]) +
                                     t * 0.5f * (b->border[0] + b->border[1])) * m;
            poly_add(&samples, q[0], q[1]);
        }
    }
    for (int k = 0; k < samples.count; ++k)
        poly_add(line, samples.xy[2 * k], samples.xy[2 * k + 1]);
    // Outward normal: the sign of the polygon area tells the orientation of a closed path.
    float area = 0;
    for (int k = 0; k < samples.count; ++k) {
        const int j = (k + 1) % samples.count;
        area += samples.xy[2 * k] * samples.xy[2 * j + 1] - samples.xy[2 * j] * samples.xy[2 * k + 1];
    }
    const float side = area > 0 ? 1.f : -1.f;
    for (int pass = 0; pass < (closed ? 1 : 2); ++pass) {
        const float sign = closed ? side : (pass ? -1.f : 1.f);
        for (int k = 0; k < samples.count; ++k) {
            const int kk = closed || !pass ? k : samples.count - 1 - k;
            const int a = MAX(0, kk - 1), b = MIN(samples.count - 1, kk + 1);
            float tx = samples.xy[2 * b] - samples.xy[2 * a], ty = samples.xy[2 * b + 1] - samples.xy[2 * a + 1];
            const float length = hypotf(tx, ty);
            if (length <= 0)
                continue;
            tx /= length;
            ty /= length;
            poly_add(border, samples.xy[2 * kk] + sign * ty * widths[kk], samples.xy[2 * kk + 1] - sign * tx * widths[kk]);
        }
    }
    if (border->count)
        poly_add(border, border->xy[0], border->xy[1]);
    g_free(widths);
    g_free(samples.xy);
    g_free(points);
}

// _gradient_get_points (develop/masks/gradient.c:684): the line through the anchor with its
// curvature, and the pivots 10 % of the shorter side away; `offset` moves the line along
// its normal, as _gradient_get_pts_border does for the border lines.
static void gradient_line(const OmSpace *space, const dt_masks_point_gradient_t *g, float offset, OmPoly *poly) {
    const float wd = space->iwidth, ht = space->iheight, scale = hypotf(wd, ht);
    const float v1 = deg2radf(-(g->rotation - 90.0f));
    const float x0 = g->anchor[0] * wd + offset * scale * cosf(v1), y0 = g->anchor[1] * ht + offset * scale * sinf(v1);
    const float v = deg2radf(-g->rotation), cosv = cosf(v), sinv = sinf(v);
    const int count = 256;
    const float xstart = fabsf(g->curvature) > 1.0f ? -sqrtf(1.0f / fabsf(g->curvature)) : -1.0f;
    const float xdelta = -2.0f * xstart / (count - 1);
    for (int i = 0; i < count; ++i) {
        const float xi = xstart + i * xdelta, yi = g->curvature * xi * xi;
        const float x = (cosv * xi + sinv * yi) * scale + x0, y = (sinv * xi - cosv * yi) * scale + y0;
        if (!(x < -wd || x > 2 * wd || y < -ht || y > 2 * ht))
            poly_add(poly, x, y);
    }
}

static void gradient_pivots(const OmSpace *space, const dt_masks_point_gradient_t *g, float *pts) {
    const float distance = 0.1f * min_side(space);
    const float v1 = deg2radf(-(g->rotation - 90.0f)), v2 = deg2radf(-(g->rotation + 90.0f));
    pts[0] = g->anchor[0] * space->iwidth + distance * cosf(v1);
    pts[1] = g->anchor[1] * space->iheight + distance * sinf(v1);
    pts[2] = g->anchor[0] * space->iwidth + distance * cosf(v2);
    pts[3] = g->anchor[1] * space->iheight + distance * sinf(v2);
}

// The reference point a clone source is measured from: the centre, or the first corner of
// a path or brush (develop/masks/path.c:1355, brush.c:750).
static gboolean form_reference(const dt_masks_form_t *form, float *xy) {
    if (!form->points)
        return FALSE;
    const void *first = form->points->data;
    if (form->type & DT_MASKS_CIRCLE)
        memcpy(xy, ((const dt_masks_point_circle_t *)first)->center, 2 * sizeof(float));
    else if (form->type & DT_MASKS_ELLIPSE)
        memcpy(xy, ((const dt_masks_point_ellipse_t *)first)->center, 2 * sizeof(float));
    else if (form->type & DT_MASKS_PATH)
        memcpy(xy, ((const dt_masks_point_path_t *)first)->corner, 2 * sizeof(float));
    else if (form->type & DT_MASKS_BRUSH)
        memcpy(xy, ((const dt_masks_point_brush_t *)first)->corner, 2 * sizeof(float));
    else if (form->type & DT_MASKS_GRADIENT)
        memcpy(xy, ((const dt_masks_point_gradient_t *)first)->anchor, 2 * sizeof(float));
    else
        return FALSE;
    return TRUE;
}

// Input pixels → preview, through every distortion.
static gboolean to_preview(const OmSpace *space, OmPoly *poly) {
    // INFINITY breaks would be transformed into garbage: transform the runs between them.
    int start = 0;
    for (int k = 0; k <= poly->count; ++k)
        if (k == poly->count || !isfinite(poly->xy[2 * k])) {
            if (k > start && !om_space_transform(space, poly->xy + 2 * start, k - start))
                return FALSE;
            start = k + 1;
        }
    for (int k = 0; k < poly->count; ++k)
        if (isfinite(poly->xy[2 * k])) {
            poly->xy[2 * k] /= space->width;
            poly->xy[2 * k + 1] /= space->height;
        }
    return TRUE;
}

// The clone source drawn where darktable draws it (_circle_get_points_source,
// develop/masks/circle.c:702): the target outline in the module's input space, shifted by
// the source offset there, then through the module and everything after it.
static gboolean source_outline(const OmSpace *space, const dt_iop_module_t *module, const dt_masks_form_t *form,
                               OmPoly *poly) {
    float ref[2];
    if (!form_reference(form, ref) || !poly->count)
        return FALSE;
    float pts[4] = {ref[0] * space->iwidth, ref[1] * space->iheight, form->source[0] * space->iwidth,
                    form->source[1] * space->iheight};
    if (!om_space_transform_dir(space, module->iop_order, DT_DEV_TRANSFORM_DIR_BACK_EXCL, FALSE, pts, 2) ||
        !om_space_transform_dir(space, module->iop_order, DT_DEV_TRANSFORM_DIR_BACK_EXCL, FALSE, poly->xy,
                                poly->count))
        return FALSE;
    const float dx = pts[2] - pts[0], dy = pts[3] - pts[1];
    for (int k = 0; k < poly->count; ++k) {
        poly->xy[2 * k] += dx;
        poly->xy[2 * k + 1] += dy;
    }
    if (!om_space_transform_dir(space, module->iop_order, DT_DEV_TRANSFORM_DIR_FORW_INCL, FALSE, poly->xy,
                                poly->count))
        return FALSE;
    om_space_to_preview(space, poly->xy, poly->count);
    return TRUE;
}

static const char *type_name(const dt_masks_form_t *form) {
    if (form->type & DT_MASKS_CIRCLE)
        return "circle";
    if (form->type & DT_MASKS_ELLIPSE)
        return "ellipse";
    if (form->type & DT_MASKS_PATH)
        return "path";
    if (form->type & DT_MASKS_BRUSH)
        return "brush";
    if (form->type & DT_MASKS_GRADIENT)
        return "gradient";
    return "other";
}

static const char *algorithm_name(int algorithm) {
    switch (algorithm) {
    case OM_RT_CLONE:
        return "clone";
    case OM_RT_HEAL:
        return "heal";
    case OM_RT_BLUR:
        return "blur";
    case OM_RT_FILL:
        return "fill";
    default:
        return "";
    }
}

static void write_poly(JsonBuilder *builder, const char *member, OmPoly *poly) {
    om_json_polyline(builder, member, poly->xy, poly->count);
    g_free(poly->xy);
    *poly = (OmPoly){0};
}

// area E: single nodes of paths and brush strokes (shape_nodes.inc)
#include "shape_nodes.inc"

static void shape_overlay(OmEngine *engine, const OmSpace *space, dt_iop_module_t *module,
                          const dt_masks_point_group_t *member, JsonBuilder *builder) {
    dt_masks_form_t *form = dt_masks_get_from_id(&engine->dev, member->formid);
    if (!form || !form->points || (form->type & DT_MASKS_GROUP))
        return;
    json_builder_begin_object(builder);
    json_builder_set_member_name(builder, "id");
    json_builder_add_int_value(builder, form->formid);
    json_builder_set_member_name(builder, "type");
    json_builder_add_string_value(builder, type_name(form));
    json_builder_set_member_name(builder, "name");
    json_builder_add_string_value(builder, form->name);
    json_builder_set_member_name(builder, "opacity");
    json_builder_add_double_value(builder, member->opacity);
    json_builder_set_member_name(builder, "inverted");
    json_builder_add_boolean_value(builder, (member->state & DT_MASKS_STATE_INVERSE) != 0);
    json_builder_set_member_name(builder, "clone");
    json_builder_add_boolean_value(builder, (form->type & DT_MASKS_CLONE) != 0);
    OmFormTable table;
    if (is_op(module, "retouch") && retouch_table(module, &table)) {
        const int index = retouch_index(&table, form->formid);
        json_builder_set_member_name(builder, "algorithm");
        json_builder_add_string_value(builder, index >= 0 ? algorithm_name(*table_int(&table, index, "algorithm")) : "");
    }
    float ref[2] = {0, 0};
    form_reference(form, ref);
    float centre[2] = {ref[0] * space->iwidth, ref[1] * space->iheight};
    OmPoly line = {0}, border = {0}, target = {0}, dt_border = {0};
    const void *first = form->points->data;
    if (form->type & DT_MASKS_CIRCLE) {
        const dt_masks_point_circle_t *c = first;
        circle_outline(space, c->center[0], c->center[1], c->radius, &line);
        circle_outline(space, c->center[0], c->center[1], c->radius + c->border, &border);
    } else if (form->type & DT_MASKS_ELLIPSE) {
        const dt_masks_point_ellipse_t *e = first;
        const gboolean prop = e->flags & DT_MASKS_ELLIPSE_PROPORTIONAL;
        ellipse_outline(space, e->center[0], e->center[1], e->radius[0], e->radius[1], e->rotation, &line);
        ellipse_outline(space, e->center[0], e->center[1], prop ? e->radius[0] * (1.0f + e->border) : e->radius[0] + e->border,
                        prop ? e->radius[1] * (1.0f + e->border) : e->radius[1] + e->border, e->rotation, &border);
        // The end of the first axis, a handle to turn the ellipse.
        const float m = min_side(space), v = deg2radf(e->rotation);
        float pivot[2] = {centre[0] + e->radius[0] * m * cosf(v), centre[1] + e->radius[0] * m * sinf(v)};
        if (om_space_transform(space, pivot, 1)) {
            om_space_to_preview(space, pivot, 1);
            om_json_point(builder, "pivot", pivot[0], pivot[1]);
        }
    } else if (form->type & (DT_MASKS_PATH | DT_MASKS_BRUSH)) {
        spline_outline(space, form, &line, &border);
        // the border exactly as darktable draws it (shape_nodes.inc), already in output pixels
        if (form_dt_border(space, form, &dt_border, NULL)) {
            g_free(border.xy);
            border = (OmPoly){0};
        }
        shape_nodes_overlay(space, form, builder);
        // The geometric centre of the corners, where a drag of the whole shape is anchored.
        float sx = 0, sy = 0;
        int n = 0;
        for (GList *it = form->points; it; it = it->next, ++n) {
            OmBezierPoint p;
            bezier_point(form, it->data, &p);
            sx += p.corner[0];
            sy += p.corner[1];
        }
        centre[0] = sx / n * space->iwidth;
        centre[1] = sy / n * space->iheight;
    } else if (form->type & DT_MASKS_GRADIENT) {
        const dt_masks_point_gradient_t *g = first;
        json_builder_set_member_name(builder, "curvature");
        json_builder_add_double_value(builder, g->curvature);
        json_builder_set_member_name(builder, "transition");
        json_builder_add_string_value(builder, g->state == DT_MASKS_GRADIENT_STATE_LINEAR ? "linear" : "sigmoid");
        gradient_line(space, g, 0.f, &line);
        gradient_line(space, g, g->compression, &border);
        poly_break(&border);
        gradient_line(space, g, -g->compression, &border);
        float pivots[4];
        gradient_pivots(space, g, pivots);
        if (om_space_transform(space, pivots, 2)) {
            om_space_to_preview(space, pivots, 2);
            om_json_point(builder, "pivot", pivots[0], pivots[1]);
            om_json_point(builder, "pivot2", pivots[2], pivots[3]);
        }
    }
    if (form->type & DT_MASKS_CLONE) {
        target.xy = g_memdup2(line.xy, sizeof(float) * 2 * line.count);
        target.count = target.capacity = line.count;
    }
    if (om_space_transform(space, centre, 1)) {
        om_space_to_preview(space, centre, 1);
        om_json_point(builder, "center", centre[0], centre[1]);
    }
    json_builder_set_member_name(builder, "closed");
    json_builder_add_boolean_value(builder, !(form->type & (DT_MASKS_BRUSH | DT_MASKS_GRADIENT)));
    if (to_preview(space, &line))
        write_poly(builder, "outline", &line);
    if (dt_border.count) {
        om_space_to_preview(space, dt_border.xy, dt_border.count);
        write_poly(builder, "border", &dt_border);
    } else if (to_preview(space, &border))
        write_poly(builder, "border", &border);
    if (form->type & DT_MASKS_CLONE) {
        float source[2] = {form->source[0] * space->iwidth, form->source[1] * space->iheight};
        if (source_outline(space, module, form, &target))
            write_poly(builder, "source", &target);
        if (om_space_transform(space, source, 1)) {
            om_space_to_preview(space, source, 1);
            om_json_point(builder, "sourceCenter", source[0], source[1]);
        }
    }
    g_free(line.xy);
    g_free(border.xy);
    g_free(dt_border.xy);
    g_free(target.xy);
    json_builder_end_object(builder);
}

void om_shapes_overlay(OmEngine *engine, const OmSpace *space, dt_iop_module_t *module, JsonBuilder *builder) {
    const gboolean retouch = is_op(module, "retouch"), spots = is_op(module, "spots");
    json_builder_set_member_name(builder, "tool");
    json_builder_add_string_value(builder, retouch ? "retouch" : spots ? "spots" : "mask");
    if (retouch) {
        json_builder_set_member_name(builder, "algorithm");
        json_builder_add_string_value(builder, algorithm_name(*param_int(module, "algorithm")));
    }
    json_builder_set_member_name(builder, "maskMode");
    json_builder_add_int_value(builder, module->blend_params ? module->blend_params->mask_mode : 0);
    dt_masks_form_t *group = module_group(engine, module);
    json_builder_set_member_name(builder, "selected");
    json_builder_add_int_value(builder, group_point(group, engine->dev.mask_form_selected_id)
                                            ? engine->dev.mask_form_selected_id
                                            : 0);
    json_builder_set_member_name(builder, "shapes");
    json_builder_begin_array(builder);
    for (GList *it = group ? group->points : NULL; it; it = it->next)
        shape_overlay(engine, space, module, it->data, builder);
    json_builder_end_array(builder);
}

// ---- creation -------------------------------------------------------------------------------

// _check_id (develop/masks/masks.c:280) on the worker's forms.
static void check_id(OmEngine *engine, dt_masks_form_t *form) {
    dt_mask_id_t nid = 100;
    for (GList *it = engine->dev.forms; it;) {
        if (((dt_masks_form_t *)it->data)->formid == form->formid) {
            form->formid = nid++;
            it = engine->dev.forms;
        } else
            it = it->next;
    }
}

// The first half of dt_masks_gui_form_save_creation (masks.c:336): a unique name, numbered
// per shape type, and registration with the develop context.
static void register_form(OmEngine *engine, dt_masks_form_t *form) {
    check_id(engine, form);
    guint nb = 0;
    for (GList *it = engine->dev.forms; it; it = it->next)
        if (((dt_masks_form_t *)it->data)->type == form->type)
            nb++;
    gboolean exist;
    do {
        exist = FALSE;
        nb++;
        if (form->functions && form->functions->set_form_name)
            form->functions->set_form_name(form, nb);
        for (GList *it = engine->dev.forms; it; it = it->next)
            if (!strcmp(((dt_masks_form_t *)it->data)->name, form->name)) {
                exist = TRUE;
                break;
            }
    } while (exist);
    engine->dev.forms = g_list_append(engine->dev.forms, form);
}

// The second half: the module's group (_group_create, masks.c:304) and the form's entry.
static void add_to_group(OmEngine *engine, dt_iop_module_t *module, dt_masks_form_t *form) {
    dt_masks_form_t *group = module_group(engine, module);
    if (!group) {
        group = dt_masks_create((form->type & (DT_MASKS_CLONE | DT_MASKS_NON_CLONE)) ? DT_MASKS_GROUP | DT_MASKS_CLONE
                                                                                    : DT_MASKS_GROUP);
        gchar *label = dt_history_item_get_name(module);
        snprintf(group->name, sizeof(group->name), _("group `%s'"), label);
        g_free(label);
        check_id(engine, group);
        engine->dev.forms = g_list_append(engine->dev.forms, group);
        module->blend_params->mask_id = group->formid;
    }
    dt_masks_point_group_t *member = malloc(sizeof(dt_masks_point_group_t));
    member->formid = form->formid;
    member->parentid = group->formid;
    member->state = DT_MASKS_STATE_SHOW | DT_MASKS_STATE_USE;
    if (group->points)
        member->state |= form->type == DT_MASKS_BRUSH ? DT_MASKS_STATE_SUM : DT_MASKS_STATE_UNION;
    member->opacity = dt_conf_get_float("plugins/darkroom/masks/opacity");
    group->points = g_list_append(group->points, member);
}

static gboolean back_point(const OmSpace *space, const float *preview, float *normalised) {
    float pts[2] = {preview[0] * space->width, preview[1] * space->height};
    if (!om_space_backtransform(space, pts, 1))
        return FALSE;
    normalised[0] = pts[0] / space->iwidth;
    normalised[1] = pts[1] / space->iheight;
    return TRUE;
}

static int read_points(JsonObject *gesture, float **points) {
    JsonNode *node = json_object_get_member(gesture, "points");
    if (!node || !JSON_NODE_HOLDS_ARRAY(node))
        return 0;
    JsonArray *array = json_node_get_array(node);
    const int n = json_array_get_length(array);
    *points = g_new(float, 2 * MAX(n, 1));
    for (int i = 0; i < n; ++i) {
        JsonNode *p = json_array_get_element(array, i);
        if (!JSON_NODE_HOLDS_ARRAY(p) || json_array_get_length(json_node_get_array(p)) != 2) {
            g_free(*points);
            *points = NULL;
            return 0;
        }
        (*points)[2 * i] = json_array_get_double_element(json_node_get_array(p), 0);
        (*points)[2 * i + 1] = json_array_get_double_element(json_node_get_array(p), 1);
    }
    return n;
}

// The default source of a clone: initial_source_pos of each shape (circle.c:1493,
// ellipse.c:1994, path.c:4061) added to the first point in output pixels, as
// dt_masks_set_source_pos_initial_value does (masks.c:2494).
static gboolean default_source(const OmSpace *space, dt_masks_form_t *form, const float *at) {
    float dx = 0, dy = 0;
    if (form->type & DT_MASKS_CIRCLE) {
        const float radius = MIN(0.5f, dt_conf_get_float("plugins/darkroom/spots/circle_size"));
        dx = radius * space->iwidth;
        dy = -(radius * space->iheight);
    } else if (form->type & DT_MASKS_ELLIPSE) {
        dx = dt_conf_get_float("plugins/darkroom/spots/ellipse_radius_a") * space->iwidth;
        dy = -(dt_conf_get_float("plugins/darkroom/spots/ellipse_radius_b") * space->iheight);
    } else {
        dx = 0.02f * space->iwidth;
        dy = 0.02f * space->iheight;
    }
    float pts[2] = {at[0] * space->width + dx, at[1] * space->height + dy};
    if (!om_space_backtransform(space, pts, 1))
        return FALSE;
    form->source[0] = pts[0] / space->iwidth;
    form->source[1] = pts[1] / space->iheight;
    return TRUE;
}

// _path_init_ctrl_points (develop/masks/path.c:470) with _path_catmull_to_bezier.
static void catmull(const float *p1, const float *p2, const float *p3, const float *p4, float *b1, float *b2) {
    b1[0] = (-p1[0] + 6 * p2[0] + p3[0]) / 6;
    b1[1] = (-p1[1] + 6 * p2[1] + p3[1]) / 6;
    b2[0] = (p2[0] + 6 * p3[0] - p4[0]) / 6;
    b2[1] = (p2[1] + 6 * p3[1] - p4[1]) / 6;
}

static void path_init_ctrl_points(dt_masks_form_t *form) {
    const int nb = g_list_length(form->points);
    if (nb < 2)
        return;
    dt_masks_point_path_t **p = g_new(dt_masks_point_path_t *, nb);
    int i = 0;
    for (GList *it = form->points; it; it = it->next)
        p[i++] = it->data;
    for (int k = 0; k < nb; ++k) {
        dt_masks_point_path_t *point3 = p[k];
        if (!(point3->state & DT_MASKS_POINT_STATE_NORMAL))
            continue;
        dt_masks_point_path_t *point2 = p[(k - 1 + nb) % nb], *point1 = p[(k - 2 + 2 * nb) % nb],
                              *point4 = p[(k + 1) % nb], *point5 = p[(k + 2) % nb];
        float b1[2], b2[2];
        catmull(point1->corner, point2->corner, point3->corner, point4->corner, b1, b2);
        if (point2->ctrl2[0] == -1.0)
            point2->ctrl2[0] = b1[0];
        if (point2->ctrl2[1] == -1.0)
            point2->ctrl2[1] = b1[1];
        point3->ctrl1[0] = b2[0];
        point3->ctrl1[1] = b2[1];
        catmull(point2->corner, point3->corner, point4->corner, point5->corner, b1, b2);
        if (point4->ctrl1[0] == -1.0)
            point4->ctrl1[0] = b2[0];
        if (point4->ctrl1[1] == -1.0)
            point4->ctrl1[1] = b2[1];
        point3->ctrl2[0] = b1[0];
        point3->ctrl2[1] = b1[1];
    }
    g_free(p);
}

// _brush_init_ctrl_points (develop/masks/brush.c:328): open ends mirror their neighbours.
static void brush_init_ctrl_points(dt_masks_form_t *form) {
    const int nb = g_list_length(form->points);
    if (nb < 2)
        return;
    dt_masks_point_brush_t **p = g_new(dt_masks_point_brush_t *, nb);
    int i = 0;
    for (GList *it = form->points; it; it = it->next)
        p[i++] = it->data;
    for (int k = 0; k < nb; ++k) {
        dt_masks_point_brush_t *point3 = p[k];
        if (!(point3->state & DT_MASKS_POINT_STATE_NORMAL))
            continue;
        dt_masks_point_brush_t start[2], end[2];
        dt_masks_point_brush_t *point1 = k >= 2 ? p[k - 2] : NULL, *point2 = k >= 1 ? p[k - 1] : NULL,
                               *point4 = k + 1 < nb ? p[k + 1] : NULL, *point5 = k + 2 < nb ? p[k + 2] : NULL;
        if (!point1 && !point2) {
            start[0].corner[0] = start[1].corner[0] = 2 * point3->corner[0] - point4->corner[0];
            start[0].corner[1] = start[1].corner[1] = 2 * point3->corner[1] - point4->corner[1];
            point1 = &start[0];
            point2 = &start[1];
        } else if (!point1) {
            start[0].corner[0] = 2 * point2->corner[0] - point3->corner[0];
            start[0].corner[1] = 2 * point2->corner[1] - point3->corner[1];
            point1 = &start[0];
        }
        if (!point4 && !point5) {
            end[0].corner[0] = end[1].corner[0] = 2 * point3->corner[0] - point2->corner[0];
            end[0].corner[1] = end[1].corner[1] = 2 * point3->corner[1] - point2->corner[1];
            point4 = &end[0];
            point5 = &end[1];
        } else if (!point5) {
            end[0].corner[0] = 2 * point4->corner[0] - point3->corner[0];
            end[0].corner[1] = 2 * point4->corner[1] - point3->corner[1];
            point5 = &end[0];
        }
        float b1[2], b2[2];
        catmull(point1->corner, point2->corner, point3->corner, point4->corner, b1, b2);
        if (point2->ctrl2[0] == -1.0)
            point2->ctrl2[0] = b1[0];
        if (point2->ctrl2[1] == -1.0)
            point2->ctrl2[1] = b1[1];
        point3->ctrl1[0] = b2[0];
        point3->ctrl1[1] = b2[1];
        catmull(point2->corner, point3->corner, point4->corner, point5->corner, b1, b2);
        if (point4->ctrl1[0] == -1.0)
            point4->ctrl1[0] = b2[0];
        if (point4->ctrl1[1] == -1.0)
            point4->ctrl1[1] = b2[1];
        point3->ctrl2[0] = b1[0];
        point3->ctrl2[1] = b1[1];
    }
    g_free(p);
}

// _brush_point_line_distance2 and _brush_ramer_douglas_peucker (brush.c:52, 131) with a
// constant payload (border, hardness, density), as a mouse without pressure produces.
static float brush_distance2(int index, int count, const float *points) {
    const float x = points[2 * index], y = points[2 * index + 1];
    const float xs = points[0], ys = points[1], xe = points[2 * (count - 1)], ye = points[2 * (count - 1) + 1];
    const float r1 = x - xs, r2 = y - ys, r3 = xe - xs, r4 = ye - ys;
    const float l = r3 * r3 + r4 * r4;
    if (l == 0.0f)
        return r1 * r1 + r2 * r2;
    const float t = (r1 * r3 + r2 * r4) / l;
    float dx, dy;
    if (t < 0.0f) {
        dx = r1;
        dy = r2;
    } else if (t > 1.0f) {
        dx = x - xe;
        dy = y - ye;
    } else {
        dx = x - (xs + t * r3);
        dy = y - (ys + t * r4);
    }
    return dx * dx + dy * dy;
}

static GList *brush_simplify(const float *points, int count, float epsilon2, const float *payload) {
    float dmax2 = 0.0f;
    int index = 0;
    for (int i = 1; i < count - 1; ++i) {
        const float d2 = brush_distance2(i, count, points);
        if (d2 > dmax2) {
            index = i;
            dmax2 = d2;
        }
    }
    if (dmax2 >= epsilon2) {
        GList *first = brush_simplify(points, index + 1, epsilon2, payload);
        GList *second = brush_simplify(points + index * 2, count - index, epsilon2, payload);
        GList *end = g_list_last(first);
        free(end->data);
        first = g_list_delete_link(first, end);
        return g_list_concat(first, second);
    }
    GList *result = NULL;
    for (int k = 0; k < 2; ++k) {
        const int i = k ? count - 1 : 0;
        dt_masks_point_brush_t *point = malloc(sizeof(dt_masks_point_brush_t));
        point->corner[0] = points[2 * i];
        point->corner[1] = points[2 * i + 1];
        point->ctrl1[0] = point->ctrl1[1] = point->ctrl2[0] = point->ctrl2[1] = -1.0f;
        point->border[0] = point->border[1] = payload[0];
        point->hardness = payload[1];
        point->density = payload[2];
        point->state = DT_MASKS_POINT_STATE_NORMAL;
        result = g_list_append(result, point);
    }
    return result;
}

static dt_masks_type_t parse_type(const char *name) {
    if (!g_strcmp0(name, "circle"))
        return DT_MASKS_CIRCLE;
    if (!g_strcmp0(name, "ellipse"))
        return DT_MASKS_ELLIPSE;
    if (!g_strcmp0(name, "path"))
        return DT_MASKS_PATH;
    if (!g_strcmp0(name, "brush"))
        return DT_MASKS_BRUSH;
    if (!g_strcmp0(name, "gradient"))
        return DT_MASKS_GRADIENT;
    return DT_MASKS_NONE;
}

// The shape buttons of retouch (retouch.c:1025, rt_add_shape) and spots create clone forms
// for clone and heal, other forms for blur and fill; spots has circle, ellipse and path.
static int creation_flags(OmEngine *engine, dt_iop_module_t *module, dt_masks_type_t shape) {
    (void)engine;
    if (is_op(module, "retouch")) {
        if (shape == DT_MASKS_GRADIENT)
            return -1;
        const int algorithm = *param_int(module, "algorithm");
        return algorithm == OM_RT_CLONE || algorithm == OM_RT_HEAL ? DT_MASKS_CLONE : DT_MASKS_NON_CLONE;
    }
    if (is_op(module, "spots"))
        return shape == DT_MASKS_CIRCLE || shape == DT_MASKS_ELLIPSE || shape == DT_MASKS_PATH ? DT_MASKS_CLONE : -1;
    return 0;
}

// Creation as in each shape's button_pressed/released (circle.c:276, ellipse.c:641,
// path.c:2078 and 1973, brush.c:1825, gradient.c:466), with darktable's configured defaults.
static int create_shape(OmEngine *engine, const OmSpace *space, dt_iop_module_t *module, JsonObject *gesture,
                        dt_mask_id_t *created) {
    const dt_masks_type_t shape = parse_type(json_object_get_string_member_with_default(gesture, "type", ""));
    if (shape == DT_MASKS_NONE)
        return 3;
    const int flags = creation_flags(engine, module, shape);
    if (flags < 0)
        return 4;
    float at[2] = {0.5f, 0.5f}, *points = NULL;
    int count = 0;
    if (shape == DT_MASKS_PATH || shape == DT_MASKS_BRUSH) {
        count = read_points(gesture, &points);
        if (count < (shape == DT_MASKS_PATH ? 3 : 1)) {
            g_free(points);
            return 5;
        }
        at[0] = points[0];
        at[1] = points[1];
    } else if (!om_gesture_point(gesture, "at", at))
        return 5;
    dt_masks_form_t *form = dt_masks_create(shape | flags);
    const dt_masks_type_t type = form->type;
    float centre[2];
    gboolean ok = TRUE;
    if (shape == DT_MASKS_CIRCLE) {
        dt_masks_point_circle_t *circle = malloc(sizeof(*circle));
        ok = back_point(space, at, circle->center);
        circle->radius = dt_conf_get_float(DT_MASKS_CONF(type, circle, size));
        circle->border = dt_conf_get_float(DT_MASKS_CONF(type, circle, border));
        form->points = g_list_append(form->points, circle);
    } else if (shape == DT_MASKS_ELLIPSE) {
        dt_masks_point_ellipse_t *ellipse = malloc(sizeof(*ellipse));
        ok = back_point(space, at, ellipse->center);
        ellipse->radius[0] = dt_conf_get_float(DT_MASKS_CONF(type, ellipse, radius_a));
        ellipse->radius[1] = dt_conf_get_float(DT_MASKS_CONF(type, ellipse, radius_b));
        ellipse->border = dt_conf_get_float(DT_MASKS_CONF(type, ellipse, border));
        ellipse->rotation = dt_conf_get_float(DT_MASKS_CONF(type, ellipse, rotation));
        ellipse->flags = dt_conf_get_int(DT_MASKS_CONF(type, ellipse, flags));
        form->points = g_list_append(form->points, ellipse);
    } else if (shape == DT_MASKS_PATH) {
        const float border = MAX(0.0005f, MIN(dt_conf_get_float(DT_MASKS_CONF(type, path, border)), 0.5f));
        for (int i = 0; i < count && ok; ++i) {
            dt_masks_point_path_t *point = malloc(sizeof(*point));
            ok = back_point(space, points + 2 * i, point->corner);
            point->ctrl1[0] = point->ctrl1[1] = point->ctrl2[0] = point->ctrl2[1] = -1.0;
            point->state = DT_MASKS_POINT_STATE_NORMAL;
            point->border[0] = point->border[1] = border;
            form->points = g_list_append(form->points, point);
        }
        path_init_ctrl_points(form);
    } else if (shape == DT_MASKS_BRUSH) {
        const float border = MIN(dt_conf_get_float(DT_MASKS_CONF(type, brush, border)), OM_BORDER_MAX);
        const float hardness = MIN(dt_conf_get_float(DT_MASKS_CONF(type, brush, hardness)), OM_HARDNESS_MAX);
        const float density = dt_conf_get_float(DT_MASKS_CONF(type, brush, density));
        // A single click gets a second point close by (brush.c:1828).
        if (count == 1) {
            points = g_renew(float, points, 4);
            points[2] = points[0] + 0.01f / space->width;
            points[3] = points[1] - 0.01f / space->height;
            count = 2;
        }
        for (int i = 0; i < count; ++i) {
            points[2 * i] *= space->width;
            points[2 * i + 1] *= space->height;
        }
        ok = om_space_backtransform(space, points, count);
        for (int i = 0; i < count; ++i) {
            points[2 * i] /= space->iwidth;
            points[2 * i + 1] /= space->iheight;
        }
        float factor = 0.01f;
        const char *smoothing = dt_conf_get_string_const("brush_smoothing");
        if (!g_strcmp0(smoothing, "low"))
            factor = 0.0025f;
        else if (!g_strcmp0(smoothing, "high"))
            factor = 0.04f;
        const float epsilon2 = factor * MAX(OM_BORDER_MIN, border) * MAX(OM_BORDER_MIN, border);
        const float payload[3] = {border, hardness, density};
        form->points = brush_simplify(points, count, epsilon2, payload);
        brush_init_ctrl_points(form);
    } else {
        // _gradient_init_values (gradient.c:239): a click keeps the configured rotation, a
        // drag ("to") points the gradient along it.
        dt_masks_point_gradient_t *gradient = malloc(sizeof(*gradient));
        float to[2];
        const gboolean dragged = om_gesture_point(gesture, "to", to) &&
                                 hypotf((to[0] - at[0]) * space->width, (to[1] - at[1]) * space->height) > 3.f;
        float pts[8] = {at[0] * space->width, at[1] * space->height, 0, 0, 0, 0, 0, 0};
        pts[2] = dragged ? to[0] * space->width : pts[0] + 100.0f;
        pts[3] = dragged ? to[1] * space->height : pts[1];
        pts[4] = pts[0] + 10.0f;
        pts[5] = pts[1];
        pts[6] = pts[0];
        pts[7] = pts[1] + 10.0f;
        ok = om_space_backtransform(space, pts, 4);
        gradient->anchor[0] = pts[0] / space->iwidth;
        gradient->anchor[1] = pts[1] / space->iheight;
        float rot = atan2f(pts[3] - pts[1], pts[2] - pts[0]);
        float check = atan2f(pts[7] - pts[1], pts[6] - pts[0]) - atan2f(pts[5] - pts[1], pts[4] - pts[0]);
        check = atan2f(sinf(check), cosf(check));
        if (check < 0.0f)
            rot -= M_PI_F;
        // Without a drag darktable keeps the configured rotation.
        gradient->rotation = dragged ? rad2degf(-rot) : dt_conf_get_float(DT_MASKS_CONF(0, gradient, rotation));
        gradient->compression = MAX(0.0f, MIN(1.0f, dt_conf_get_float(DT_MASKS_CONF(0, gradient, compression))));
        gradient->curvature = MAX(-2.0f, MIN(2.0f, dt_conf_get_float(DT_MASKS_CONF(0, gradient, curvature))));
        gradient->steepness = 0.0f;
        gradient->state = DT_MASKS_GRADIENT_STATE_SIGMOIDAL;
        form->points = g_list_append(form->points, gradient);
    }
    if (ok && (type & DT_MASKS_CLONE)) {
        if (om_gesture_point(gesture, "source", centre))
            ok = back_point(space, centre, form->source);
        else
            ok = default_source(space, form, at);
    }
    g_free(points);
    if (!ok || !form->points) {
        dt_masks_free_form(form);
        return 4;
    }
    register_form(engine, form);
    add_to_group(engine, module, form);
    resynch(engine, module);
    if (!is_op(module, "retouch") && !is_op(module, "spots"))
        module->blend_params->mask_mode |= DEVELOP_MASK_ENABLED | DEVELOP_MASK_MASK;
    engine->dev.mask_form_selected_id = form->formid;
    *created = form->formid;
    return om_canvas_commit(engine, module, TRUE);
}

// ---- editing ----------------------------------------------------------------------------------

// The stored positions of a form (centres, anchors, corners and control points).
static int form_positions(dt_masks_form_t *form, float **out, int capacity) {
    int n = 0;
    for (GList *it = form->points; it; it = it->next) {
        if (form->type & DT_MASKS_CIRCLE) {
            if (n < capacity)
                out[n] = ((dt_masks_point_circle_t *)it->data)->center;
            n++;
        } else if (form->type & DT_MASKS_ELLIPSE) {
            if (n < capacity)
                out[n] = ((dt_masks_point_ellipse_t *)it->data)->center;
            n++;
        } else if (form->type & DT_MASKS_GRADIENT) {
            if (n < capacity)
                out[n] = ((dt_masks_point_gradient_t *)it->data)->anchor;
            n++;
        } else if (form->type & DT_MASKS_PATH) {
            dt_masks_point_path_t *p = it->data;
            float *list[3] = {p->corner, p->ctrl1, p->ctrl2};
            for (int k = 0; k < 3; ++k, ++n)
                if (n < capacity)
                    out[n] = list[k];
        } else if (form->type & DT_MASKS_BRUSH) {
            dt_masks_point_brush_t *p = it->data;
            float *list[3] = {p->corner, p->ctrl1, p->ctrl2};
            for (int k = 0; k < 3; ++k, ++n)
                if (n < capacity)
                    out[n] = list[k];
        }
    }
    return n;
}

// A drag of the whole shape: the reference point follows the pointer and every stored
// position moves by the same amount in input coordinates, as the shapes' mouse_moved
// handlers do while form_dragging.
static gboolean move_form(const OmSpace *space, dt_masks_form_t *form, const float *from, const float *to) {
    float ref[2];
    if (!form_reference(form, ref))
        return FALSE;
    float pts[2] = {ref[0] * space->iwidth, ref[1] * space->iheight};
    if (!om_space_transform(space, pts, 1))
        return FALSE;
    pts[0] += (to[0] - from[0]) * space->width;
    pts[1] += (to[1] - from[1]) * space->height;
    if (!om_space_backtransform(space, pts, 1))
        return FALSE;
    const float dx = pts[0] / space->iwidth - ref[0], dy = pts[1] / space->iheight - ref[1];
    const int n = form_positions(form, NULL, 0);
    float **positions = g_new(float *, MAX(n, 1));
    form_positions(form, positions, n);
    for (int i = 0; i < n; ++i) {
        positions[i][0] += dx;
        positions[i][1] += dy;
    }
    g_free(positions);
    return TRUE;
}

static float scaled(float factor, float value, float minimum, float maximum) {
    return CLAMP(value * factor, minimum, maximum);
}

// Size and feather. `factor` is a continuous scale of a drag; the wheel uses darktable's own
// step (dt_masks_change_size: ×1/0.97 or ×0.97) through the same clamps of each shape's
// mouse_scrolled handler (circle.c:155, ellipse.c:515, path.c:1820, brush.c:1600,
// gradient.c:160). darktable remembers the new size as the default of the next shape.
static gboolean resize_form(dt_masks_form_t *form, float factor, gboolean feather) {
    const gboolean clone = form->type & (DT_MASKS_CLONE | DT_MASKS_NON_CLONE);
    const float limit = clone ? 0.5f : 1.0f;
    void *first = form->points ? form->points->data : NULL;
    if (!first || !(factor > 0) || !isfinite(factor))
        return FALSE;
    const dt_masks_type_t type = form->type;
    if (type & DT_MASKS_CIRCLE) {
        dt_masks_point_circle_t *c = first;
        if (feather) {
            c->border = scaled(factor, c->border, OM_MIN_CIRCLE_BORDER, limit);
            dt_conf_set_float(DT_MASKS_CONF(type, circle, border), c->border);
        } else {
            c->radius = scaled(factor, c->radius, OM_MIN_CIRCLE_BORDER, limit);
            dt_conf_set_float(DT_MASKS_CONF(type, circle, size), c->radius);
        }
    } else if (type & DT_MASKS_ELLIPSE) {
        dt_masks_point_ellipse_t *e = first;
        if (feather) {
            const float reference = e->flags & DT_MASKS_ELLIPSE_PROPORTIONAL ? 1.0f / fminf(e->radius[0], e->radius[1]) : 1.0f;
            e->border = scaled(factor, e->border, 0.001f * reference, limit * reference);
            dt_conf_set_float(DT_MASKS_CONF(type, ellipse, border), e->border);
        } else {
            const float old = e->radius[0];
            e->radius[0] = scaled(factor, e->radius[0], 0.001f, limit);
            e->radius[1] *= e->radius[0] / old;
            dt_conf_set_float(DT_MASKS_CONF(type, ellipse, radius_a), e->radius[0]);
            dt_conf_set_float(DT_MASKS_CONF(type, ellipse, radius_b), e->radius[1]);
        }
    } else if (type & DT_MASKS_PATH) {
        if (feather) {
            for (GList *it = form->points; it; it = it->next) {
                dt_masks_point_path_t *p = it->data;
                if (factor > 1 && (p->border[0] > 1.0f || p->border[1] > 1.0f))
                    return FALSE;
            }
            for (GList *it = form->points; it; it = it->next) {
                dt_masks_point_path_t *p = it->data;
                p->border[0] = scaled(factor, p->border[0], 0.0005f, 0.5f);
                p->border[1] = scaled(factor, p->border[1], 0.0005f, 0.5f);
            }
        } else {
            // Around the centre of gravity of the corner polygon (path.c:1858).
            float bx = 0, by = 0, surf = 0;
            for (GList *it = form->points; it; it = it->next) {
                const GList *next = it->next ? it->next : form->points;
                const dt_masks_point_path_t *p1 = it->data, *p2 = next->data;
                const float cross = p1->corner[0] * p2->corner[1] - p2->corner[0] * p1->corner[1];
                surf += cross;
                bx += (p1->corner[0] + p2->corner[0]) * cross;
                by += (p1->corner[1] + p2->corner[1]) * cross;
            }
            if (surf == 0)
                return FALSE;
            bx /= 3.0f * surf;
            by /= 3.0f * surf;
            surf = sqrtf(fabsf(surf));
            if ((factor < 1 && surf < 0.001f) || (factor > 1 && surf > 2.0f))
                return FALSE;
            for (GList *it = form->points; it; it = it->next) {
                dt_masks_point_path_t *p = it->data;
                const float c1x = (p->ctrl1[0] - p->corner[0]) * factor, c1y = (p->ctrl1[1] - p->corner[1]) * factor;
                const float c2x = (p->ctrl2[0] - p->corner[0]) * factor, c2y = (p->ctrl2[1] - p->corner[1]) * factor;
                p->corner[0] = bx + (p->corner[0] - bx) * factor;
                p->corner[1] = by + (p->corner[1] - by) * factor;
                p->ctrl1[0] = p->corner[0] + c1x;
                p->ctrl1[1] = p->corner[1] + c1y;
                p->ctrl2[0] = p->corner[0] + c2x;
                p->ctrl2[1] = p->corner[1] + c2y;
            }
        }
    } else if (type & DT_MASKS_BRUSH) {
        for (GList *it = form->points; it; it = it->next) {
            dt_masks_point_brush_t *p = it->data;
            if (feather)
                p->hardness = scaled(factor, p->hardness, OM_HARDNESS_MIN, OM_HARDNESS_MAX);
            else {
                if (factor > 1 && (p->border[0] > 1.0f || p->border[1] > 1.0f))
                    return FALSE;
                p->border[0] = scaled(factor, p->border[0], OM_BORDER_MIN, OM_BORDER_MAX);
                p->border[1] = scaled(factor, p->border[1], OM_BORDER_MIN, OM_BORDER_MAX);
            }
        }
    } else if (type & DT_MASKS_GRADIENT) {
        dt_masks_point_gradient_t *g = first;
        if (!feather)
            return FALSE;
        g->compression = fminf(fmaxf(g->compression, 0.001f) * factor, 1.0f);
        dt_conf_set_float(DT_MASKS_CONF(type, gradient, compression), g->compression);
    }
    return TRUE;
}

// The plain wheel on a gradient bends it (gradient.c:178); shift+ctrl turns an ellipse
// (ellipse.c:501, dt_masks_change_rotation: 40 steps a turn).
static gboolean scroll_form(dt_masks_form_t *form, gboolean up, const char *modifier) {
    void *first = form->points ? form->points->data : NULL;
    if (!first)
        return FALSE;
    const float step = up ? 1.0f / 0.97f : 0.97f;
    if (!strcmp(modifier, "shift+ctrl")) {
        if (!(form->type & DT_MASKS_ELLIPSE))
            return FALSE;
        dt_masks_point_ellipse_t *e = first;
        e->rotation = fmodf((up ? e->rotation + 9.0f : e->rotation - 9.0f) + 360.0f, 360.0f);
        dt_conf_set_float(DT_MASKS_CONF(form->type, ellipse, rotation), e->rotation);
        return TRUE;
    }
    if (!strcmp(modifier, "shift")) {
        if (form->type & DT_MASKS_GRADIENT)
            return resize_form(form, up ? 1.0f / 0.8f : 0.8f, TRUE);
        return resize_form(form, step, TRUE);
    }
    if (form->type & DT_MASKS_GRADIENT) {
        dt_masks_point_gradient_t *g = first;
        g->curvature = up ? fminf(g->curvature + 0.01f, 2.0f) : fmaxf(g->curvature - 0.01f, -2.0f);
        return TRUE;
    }
    return resize_form(form, step, FALSE);
}

// Turning by a handle: the ellipse's first axis, or a gradient's pivot, points at the pointer
// (gradient.c:404 checks the handedness of the transformed axes, as here).
static gboolean rotate_form(const OmSpace *space, dt_masks_form_t *form, const float *at) {
    float ref[2];
    if (!form_reference(form, ref) || !(form->type & (DT_MASKS_ELLIPSE | DT_MASKS_GRADIENT)))
        return FALSE;
    float c[2] = {ref[0] * space->iwidth, ref[1] * space->iheight};
    if (!om_space_transform(space, c, 1))
        return FALSE;
    float pts[8] = {c[0], c[1], at[0] * space->width, at[1] * space->height, c[0] + 10.0f, c[1], c[0], c[1] + 10.0f};
    if (!om_space_backtransform(space, pts, 4))
        return FALSE;
    float angle = atan2f(pts[3] - pts[1], pts[2] - pts[0]);
    float check = atan2f(pts[7] - pts[1], pts[6] - pts[0]) - atan2f(pts[5] - pts[1], pts[4] - pts[0]);
    check = atan2f(sinf(check), cosf(check));
    if (form->type & DT_MASKS_ELLIPSE) {
        dt_masks_point_ellipse_t *e = form->points->data;
        e->rotation = fmodf(rad2degf(angle) + 360.0f, 360.0f);
    } else {
        // The first pivot lies at -(rotation - 90°) from the anchor.
        dt_masks_point_gradient_t *g = form->points->data;
        if (check < 0.0f)
            angle += M_PI_F;
        float rotation = 90.0f - rad2degf(angle);
        rotation = fmodf(rotation + 540.0f, 360.0f) - 180.0f;
        g->rotation = rotation;
    }
    return TRUE;
}

static void free_form_point(dt_masks_form_t *group, dt_mask_id_t id) {
    for (GList *it = group->points; it; it = it->next)
        if (((dt_masks_point_group_t *)it->data)->formid == id) {
            free(it->data);
            group->points = g_list_delete_link(group->points, it);
            return;
        }
}

static gboolean used_elsewhere(OmEngine *engine, dt_mask_id_t id) {
    for (GList *it = engine->dev.forms; it; it = it->next) {
        dt_masks_form_t *form = it->data;
        if ((form->type & DT_MASKS_GROUP) && group_point(form, id))
            return TRUE;
    }
    return FALSE;
}

// The shape's entry leaves the group; the form itself goes once no other group uses it
// (dt_masks_form_remove, develop/masks/masks.c, without its GUI bookkeeping).
static void remove_form(OmEngine *engine, dt_iop_module_t *module, dt_mask_id_t id) {
    dt_masks_form_t *group = module_group(engine, module);
    if (group)
        free_form_point(group, id);
    if (!used_elsewhere(engine, id)) {
        dt_masks_form_t *form = dt_masks_get_from_id(&engine->dev, id);
        if (form) {
            engine->dev.forms = g_list_remove(engine->dev.forms, form);
            dt_masks_free_form(form);
        }
    }
    if (engine->dev.mask_form_selected_id == id)
        engine->dev.mask_form_selected_id = 0;
    resynch(engine, module);
}

static int parse_algorithm(const char *name) {
    if (!g_strcmp0(name, "clone"))
        return OM_RT_CLONE;
    if (!g_strcmp0(name, "heal"))
        return OM_RT_HEAL;
    if (!g_strcmp0(name, "blur"))
        return OM_RT_BLUR;
    if (!g_strcmp0(name, "fill"))
        return OM_RT_FILL;
    return 0;
}

// rt_select_algorithm_callback (iop/retouch.c:1921): with a shape given, that shape changes
// within its family (clone ↔ heal, blur ↔ fill), otherwise the algorithm of new shapes.
static int retouch_algorithm(OmEngine *engine, dt_iop_module_t *module, JsonObject *gesture) {
    const int algorithm = parse_algorithm(json_object_get_string_member_with_default(gesture, "value", ""));
    if (!algorithm)
        return 5;
    const dt_mask_id_t id = om_gesture_number(gesture, "id", 0);
    OmFormTable table;
    if (dt_is_valid_maskid(id) && id > 0) {
        const int index = retouch_table(module, &table) ? retouch_index(&table, id) : -1;
        if (index < 0)
            return 5;
        const int current = *table_int(&table, index, "algorithm");
        const gboolean cloning = current == OM_RT_CLONE || current == OM_RT_HEAL;
        if (current != algorithm && cloning != (algorithm == OM_RT_CLONE || algorithm == OM_RT_HEAL))
            return 4;
        *param_int(module, "algorithm") = algorithm;
        *table_int(&table, index, "algorithm") = algorithm;
    } else
        *param_int(module, "algorithm") = algorithm;
    return om_canvas_commit(engine, module, FALSE);
}

int om_shapes_edit(OmEngine *engine, const OmSpace *space, dt_iop_module_t *module, JsonObject *gesture) {
    const char *action = json_object_get_string_member_with_default(gesture, "action", "");
    if (!strcmp(action, "add")) {
        dt_mask_id_t created = 0;
        return create_shape(engine, space, module, gesture, &created);
    }
    if (!strcmp(action, "algorithm"))
        return is_op(module, "retouch") ? retouch_algorithm(engine, module, gesture) : 4;
    const dt_mask_id_t id = om_gesture_number(gesture, "id", 0);
    if (!strcmp(action, "select")) {
        // dt_dev_masks_selection_change: the selection lives in the develop context.
        engine->dev.mask_form_selected_id = group_point(module_group(engine, module), id) ? id : 0;
        if (is_op(module, "retouch") && retouch_show_selection(engine, module))
            return om_canvas_commit(engine, module, FALSE);
        return 0;
    }
    dt_masks_form_t *group = module_group(engine, module);
    dt_masks_point_group_t *member = group_point(group, id);
    dt_masks_form_t *form = member ? dt_masks_get_from_id(&engine->dev, id) : NULL;
    if (!form)
        return 5;
    float from[2], to[2];
    gboolean changed = FALSE;
    const int node = shape_nodes_edit(space, form, action, gesture);   // area E
    if (node == 2) {
        remove_form(engine, module, id);
        changed = TRUE;
    } else if (node >= 0) {
        if (node > 2)
            return node;
        changed = node == 1;
    } else if (!strcmp(action, "remove")) {
        remove_form(engine, module, id);
        changed = TRUE;
    } else if (!strcmp(action, "move")) {
        if (!om_gesture_point(gesture, "from", from) || !om_gesture_point(gesture, "to", to))
            return 5;
        changed = move_form(space, form, from, to);
    } else if (!strcmp(action, "move-source")) {
        if (!(form->type & DT_MASKS_CLONE) || !om_gesture_point(gesture, "from", from) ||
            !om_gesture_point(gesture, "to", to))
            return 5;
        float pts[2] = {form->source[0] * space->iwidth, form->source[1] * space->iheight};
        changed = om_space_transform(space, pts, 1);
        pts[0] += (to[0] - from[0]) * space->width;
        pts[1] += (to[1] - from[1]) * space->height;
        changed = changed && om_space_backtransform(space, pts, 1);
        if (changed) {
            form->source[0] = pts[0] / space->iwidth;
            form->source[1] = pts[1] / space->iheight;
        }
    } else if (!strcmp(action, "scale") || !strcmp(action, "feather"))
        changed = resize_form(form, om_gesture_number(gesture, "factor", 1.0), !strcmp(action, "feather"));
    else if (!strcmp(action, "scroll"))
        changed = scroll_form(form, json_object_get_boolean_member_with_default(gesture, "up", TRUE),
                              json_object_get_string_member_with_default(gesture, "modifier", ""));
    else if (!strcmp(action, "rotate")) {
        if (!om_gesture_point(gesture, "at", to))
            return 5;
        changed = rotate_form(space, form, to);
    } else if (!strcmp(action, "opacity")) {
        // dt_masks_form_change_opacity (masks.c): 5 % to 100 %.
        const double value = om_gesture_number(gesture, "value", NAN);
        if (!isfinite(value))
            return 5;
        member->opacity = CLAMP(value, 0.05f, 1.0f);
        changed = TRUE;
    } else if (!strcmp(action, "curvature-reset")) {
        // gradient.c:208 a double-click straightens the gradient
        if (!(form->type & DT_MASKS_GRADIENT))
            return 5;
        dt_masks_point_gradient_t *g = form->points->data;
        changed = g->curvature != 0.0f;
        g->curvature = 0.0f;
    } else if (!strcmp(action, "transition")) {
        // gradient.c:442 Shift+click switches the transition between linear and sigmoidal
        if (!(form->type & DT_MASKS_GRADIENT))
            return 5;
        dt_masks_point_gradient_t *g = form->points->data;
        g->state = g->state == DT_MASKS_GRADIENT_STATE_LINEAR ? DT_MASKS_GRADIENT_STATE_SIGMOIDAL
                                                              : DT_MASKS_GRADIENT_STATE_LINEAR;
        changed = TRUE;
    } else if (!strcmp(action, "invert")) {
        member->state ^= DT_MASKS_STATE_INVERSE;
        changed = TRUE;
    } else
        return 3;
    if (!changed)
        return 0;
    return om_canvas_commit(engine, module, TRUE);
}

// ---- interface for the blend section ---------------------------------------------------------

static dt_iop_module_t *blend_module(OmEngine *engine, const char *operation, int instance, int *error) {
    *error = engine->loaded ? 0 : 1;
    dt_iop_module_t *module =
        engine->loaded ? dt_iop_get_module_by_op_priority(engine->dev.iop, operation, instance) : NULL;
    if (!*error && !module)
        *error = 2;
    if (module && (!(module->flags() & IOP_FLAGS_SUPPORTS_BLENDING) || is_op(module, "retouch") ||
                   is_op(module, "spots")))
        *error = 4;
    return *error ? NULL : module;
}

// masks_api.h: `type` is darktable's DT_MASKS_* shape; `json` is optional and holds the add
// gesture's members in preview coordinates ("at", and "points" for a path or brush, see
// canvas.h). Without it the shape is placed at the centre of the photo, a path or brush as a
// small triangle or stroke around it. Returns 0 or a positive error as om_engine_canvas_edit.
int om_engine_masks_add_shape(OmEngine *engine, const char *operation, int instance, int type, const char *json) {
    int error;
    dt_iop_module_t *module = blend_module(engine, operation, instance, &error);
    OmSpace space;
    if (!module)
        return error;
    if (!om_space_init(engine, &space))
        return 4;
    const char *name = type & DT_MASKS_CIRCLE     ? "circle"
                       : type & DT_MASKS_ELLIPSE  ? "ellipse"
                       : type & DT_MASKS_PATH     ? "path"
                       : type & DT_MASKS_BRUSH    ? "brush"
                       : type & DT_MASKS_GRADIENT ? "gradient"
                                                  : NULL;
    if (!name)
        return 3;
    JsonParser *parser = NULL;
    JsonObject *given = json && *json ? om_gesture_parse(json, &parser) : NULL;
    JsonObject *gesture = given ? json_object_ref(given) : json_object_new();
    json_object_set_string_member(gesture, "type", name);
    float at[2] = {0.5f, 0.5f};
    if (!om_gesture_point(gesture, "at", at)) {
        JsonArray *point = json_array_new();
        json_array_add_double_element(point, at[0]);
        json_array_add_double_element(point, at[1]);
        json_object_set_array_member(gesture, "at", point);
    }
    if ((type & (DT_MASKS_PATH | DT_MASKS_BRUSH)) && !json_object_has_member(gesture, "points")) {
        JsonArray *points = json_array_new();
        const float d = 0.05f;
        const float corners[3][2] = {{at[0], at[1] - d}, {at[0] + d, at[1] + d}, {at[0] - d, at[1] + d}};
        for (int i = 0; i < ((type & DT_MASKS_PATH) ? 3 : 2); ++i) {
            JsonArray *p = json_array_new();
            json_array_add_double_element(p, corners[i][0]);
            json_array_add_double_element(p, corners[i][1]);
            json_array_add_array_element(points, p);
        }
        json_object_set_array_member(gesture, "points", points);
    }
    dt_mask_id_t created = 0;
    const int result = create_shape(engine, &space, module, gesture, &created);
    json_object_unref(gesture);
    if (parser)
        g_object_unref(parser);
    return result;
}

int om_engine_masks_count(OmEngine *engine, const char *operation, int instance) {
    int error;
    dt_iop_module_t *module = blend_module(engine, operation, instance, &error);
    if (!module)
        return -error;
    dt_masks_form_t *group = module_group(engine, module);
    return group ? (int)g_list_length(group->points) : 0;
}

int om_engine_masks_clear(OmEngine *engine, const char *operation, int instance) {
    int error;
    dt_iop_module_t *module = blend_module(engine, operation, instance, &error);
    if (!module)
        return error;
    dt_masks_form_t *group = module_group(engine, module);
    while (group && group->points)
        remove_form(engine, module, ((dt_masks_point_group_t *)group->points->data)->formid);
    module->blend_params->mask_mode &= ~DEVELOP_MASK_MASK;
    return om_canvas_commit(engine, module, TRUE);
}
