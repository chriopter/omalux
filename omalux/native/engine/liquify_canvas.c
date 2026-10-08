// SPDX-License-Identifier: GPL-3.0-or-later
// liquify's warps without GTK (src/iop/liquify.c in darktable 5.6.1).
//
// liquify keeps its paths in its parameters: up to 100 nodes, each a warp (centre, strength
// point, radius point, hardness) and, for curves, two Bézier control points, all as complex
// numbers in input pixels of the full pipe. The node layout below mirrors the module's
// private structs; it is checked against the introspection size before any access.
#include "canvas_internal.h"
#include <complex.h>
#include <math.h>

enum { OM_LIQUIFY_NODES = 100 };
enum { OM_PATH_INVALIDATED = 0, OM_PATH_MOVE_TO = 1, OM_PATH_LINE_TO = 2, OM_PATH_CURVE_TO = 3 };
enum { OM_WARP_LINEAR = 0, OM_WARP_GROW = 1, OM_WARP_SHRINK = 2 };
enum { OM_NODE_AUTOSMOOTH = 3 };

typedef struct {
    int type, node_type, selected, hovered;
    int8_t prev, idx, next;
} OmLiquifyHeader;

typedef struct {
    float complex point, strength, radius;
    float control1, control2;
    int type, status;
} OmLiquifyWarp;

typedef struct {
    OmLiquifyHeader header;
    OmLiquifyWarp warp;
    struct {
        float complex ctrl1, ctrl2;
    } node;
} OmLiquifyNode;

static OmLiquifyNode *liquify_nodes(dt_iop_module_t *module) {
    const dt_introspection_field_t *field = module->get_f ? module->get_f("nodes") : NULL;
    if (!field || field->header.type != DT_INTROSPECTION_TYPE_ARRAY || field->Array.count != OM_LIQUIFY_NODES ||
        field->Array.field->header.size != sizeof(OmLiquifyNode) ||
        module->params_size != sizeof(OmLiquifyNode) * OM_LIQUIFY_NODES)
        return NULL;
    return module->get_p(module->params, "nodes");
}

static OmLiquifyNode *node_get(OmLiquifyNode *nodes, int index) {
    return index > -1 && index < OM_LIQUIFY_NODES ? &nodes[index] : NULL;
}

static OmLiquifyNode *node_prev(OmLiquifyNode *nodes, const OmLiquifyNode *n) {
    return n->header.prev == -1 ? NULL : &nodes[n->header.prev];
}

static OmLiquifyNode *node_next(OmLiquifyNode *nodes, const OmLiquifyNode *n) {
    return n->header.next == -1 ? NULL : &nodes[n->header.next];
}

// node_alloc, init_warp (liquify.c:340, 2619).
static OmLiquifyNode *node_alloc(OmLiquifyNode *nodes, int type, float complex point) {
    for (int k = 0; k < OM_LIQUIFY_NODES; ++k)
        if (nodes[k].header.type == OM_PATH_INVALIDATED) {
            OmLiquifyNode *n = &nodes[k];
            memset(n, 0, sizeof(*n));
            n->header.idx = k;
            n->header.next = n->header.prev = -1;
            n->header.type = type;
            n->header.node_type = OM_NODE_AUTOSMOOTH;
            n->warp.type = OM_WARP_LINEAR;
            n->warp.point = n->warp.radius = n->warp.strength = point;
            n->warp.control1 = 0.5;
            n->warp.control2 = 0.75;
            return n;
        }
    return NULL;
}

// node_gc (liquify.c:393): compact the array, keeping the links.
static void node_gc(OmLiquifyNode *p) {
    int last = 0;
    for (last = OM_LIQUIFY_NODES - 1; last > 0; last--)
        if (p[last].header.type != OM_PATH_INVALIDATED)
            break;
    int k = 0;
    while (k <= last) {
        if (p[k].header.type == OM_PATH_INVALIDATED) {
            for (int e = 0; e < last; e++) {
                if (e >= k)
                    p[e] = p[e + 1];
                if (e >= k)
                    p[e].header.idx--;
                if (p[e].header.prev >= k)
                    p[e].header.prev--;
                if (p[e].header.next >= k)
                    p[e].header.next--;
            }
            last--;
        } else
            k++;
    }
    for (int l = last + 1; l < OM_LIQUIFY_NODES; l++)
        p[l].header.type = OM_PATH_INVALIDATED;
}

// node_delete and path_delete (liquify.c:425, 449).
static void node_delete(OmLiquifyNode *p, OmLiquifyNode *self) {
    OmLiquifyNode *prev = node_prev(p, self), *next = node_next(p, self);
    if (!prev && next) {
        next->header.prev = -1;
        next->header.type = OM_PATH_MOVE_TO;
    } else if (prev) {
        prev->header.next = self->header.next;
        if (next)
            next->header.prev = prev->header.idx;
    }
    self->header.prev = self->header.next = -1;
    self->header.type = OM_PATH_INVALIDATED;
    node_gc(p);
}

static void path_delete(OmLiquifyNode *p, OmLiquifyNode *self) {
    for (OmLiquifyNode *n = self; n; n = node_next(p, n))
        n->header.type = OM_PATH_INVALIDATED;
    for (OmLiquifyNode *n = self; n; n = node_prev(p, n))
        n->header.type = OM_PATH_INVALIDATED;
    node_gc(p);
}

// smooth_path_linsys and smooth_paths_linsys (liquify.c:2420, 2509): control points of
// autosmooth nodes from a tridiagonal system, solved with the Thomas algorithm.
static void smooth_path_linsys(size_t n, const float complex *k, float complex *c1, float complex *c2,
                               const int *equation) {
    --n;
    float *a = malloc(sizeof(float) * n), *b = malloc(sizeof(float) * n), *c = malloc(sizeof(float) * n);
    float complex *d = malloc(sizeof(float complex) * n);
    for (int i = 0; i < (int)n; i++) {
        switch (equation[i]) {
#define ABCD(A, B, C, D)                                                                                         \
    {                                                                                                          \
        a[i] = A;                                                                                              \
        b[i] = B;                                                                                              \
        c[i] = C;                                                                                              \
        d[i] = D;                                                                                              \
        continue;                                                                                              \
    }
        case 1:
            ABCD(0, 2, 1, k[i] + 2 * k[i + 1]);
        case 2:
            ABCD(1, 4, 1, 4 * k[i] + 2 * k[i + 1]);
        case 3:
            ABCD(2, 7, 0, 8 * k[i] + k[i + 1]);
        case 4:
            ABCD(0, 1, 0, c1[i]);
        case 5:
            ABCD(0, 1, 0, c1[i]);
        case 6:
            ABCD(1, 4, 0, 4 * k[i] + c2[i]);
        case 7:
            ABCD(0, 1, 0, c1[i]);
        case 8:
            ABCD(0, 3, 0, 2 * k[i] + k[i + 1]);
        case 9:
            ABCD(0, 2, 0, k[i] + c2[i]);
#undef ABCD
        }
    }
    for (int i = 1; i < (int)n; i++) {
        const float m = a[i] / b[i - 1];
        b[i] = b[i] - m * c[i - 1];
        d[i] = d[i] - m * d[i - 1];
    }
    c1[n - 1] = d[n - 1] / b[n - 1];
    for (int i = n - 2; i >= 0; i--)
        c1[i] = (d[i] - c[i] * c1[i + 1]) / b[i];
    for (int i = 0; i < (int)n; i++) {
        switch (equation[i]) {
        case 5:
        case 6:
        case 9:
            break;
        case 3:
        case 7:
        case 8:
            c2[i] = (c1[i] + k[i + 1]) / 2;
            break;
        default:
            c2[i] = 2 * k[i + 1] - c1[i + 1];
        }
    }
    free(a);
    free(b);
    free(c);
    free(d);
}

static void smooth_paths_linsys(OmLiquifyNode *params) {
    for (int k = 0; k < OM_LIQUIFY_NODES; k++) {
        if (params[k].header.type == OM_PATH_INVALIDATED)
            break;
        if (params[k].header.prev != -1)
            continue;
        OmLiquifyNode *node = &params[k];
        size_t n = 1;
        for (const OmLiquifyNode *m = node; m->header.next != -1; m = &params[m->header.next])
            ++n;
        if (n < 2)
            continue;
        float complex *pt = calloc(n, sizeof(float complex)), *c1 = calloc(n, sizeof(float complex)),
                      *c2 = calloc(n, sizeof(float complex));
        int *eqn = calloc(n, sizeof(int));
        size_t idx = 0;
        while (node) {
            const OmLiquifyNode *d = node, *p = node_prev(params, node), *nx = node_next(params, node);
            const OmLiquifyNode *nn = nx ? node_next(params, nx) : NULL;
            pt[idx] = node->warp.point;
            if (d->header.type == OM_PATH_CURVE_TO) {
                c1[idx - 1] = d->node.ctrl1;
                c2[idx - 1] = d->node.ctrl2;
            }
            const int autosmooth = d->header.node_type == OM_NODE_AUTOSMOOTH;
            const int next_autosmooth = nx && nx->header.node_type == OM_NODE_AUTOSMOOTH;
            const int firstseg = !p || d->header.type != OM_PATH_CURVE_TO;
            const int lastseg = !nn || nn->header.type != OM_PATH_CURVE_TO;
            const int lineseg = nx && nx->header.type == OM_PATH_LINE_TO;
            if (lineseg)
                eqn[idx] = 5;
            else if (!autosmooth && !next_autosmooth)
                eqn[idx] = 5;
            else if (firstseg && lastseg && !autosmooth && next_autosmooth)
                eqn[idx] = 7;
            else if (firstseg && lastseg && autosmooth && next_autosmooth)
                eqn[idx] = 8;
            else if (firstseg && lastseg && autosmooth && !next_autosmooth)
                eqn[idx] = 9;
            else if (firstseg && autosmooth && !next_autosmooth)
                eqn[idx] = 5;
            else if (firstseg && autosmooth)
                eqn[idx] = 1;
            else if (lastseg && autosmooth && next_autosmooth)
                eqn[idx] = 3;
            else if (lastseg && !autosmooth && next_autosmooth)
                eqn[idx] = 7;
            else if (autosmooth && !next_autosmooth)
                eqn[idx] = 6;
            else if (!autosmooth && next_autosmooth)
                eqn[idx] = 4;
            else
                eqn[idx] = 2;
            ++idx;
            node = node_next(params, node);
        }
        smooth_path_linsys(n, pt, c1, c2, eqn);
        node = node_next(params, &params[k]);
        idx = 0;
        while (node) {
            if (node->header.type == OM_PATH_CURVE_TO) {
                node->node.ctrl1 = c1[idx];
                node->node.ctrl2 = c2[idx];
            }
            ++idx;
            node = node_next(params, node);
        }
        free(pt);
        free(c1);
        free(c2);
        free(eqn);
    }
}

// The overlay skips liquify itself (_distort_paths_locked with DIR_ALL, liquify.c:542):
// input pixels through the modules before it, then the modules after it.
static gboolean liquify_to_preview(const OmSpace *space, const dt_iop_module_t *module, float *pts, int count) {
    if (!om_space_transform_dir(space, module->iop_order, DT_DEV_TRANSFORM_DIR_BACK_EXCL, FALSE, pts, count) ||
        !om_space_transform_dir(space, module->iop_order, DT_DEV_TRANSFORM_DIR_FORW_EXCL, FALSE, pts, count))
        return FALSE;
    om_space_to_preview(space, pts, count);
    return TRUE;
}

// get_point_scale (liquify.c:2771): the reverse, preview to input pixels.
static gboolean liquify_from_preview(const OmSpace *space, const dt_iop_module_t *module, const float *preview,
                                     float complex *point) {
    float pts[2] = {preview[0] * space->width, preview[1] * space->height};
    if (!om_space_transform_dir(space, module->iop_order, DT_DEV_TRANSFORM_DIR_FORW_EXCL, TRUE, pts, 1) ||
        !om_space_transform_dir(space, module->iop_order, DT_DEV_TRANSFORM_DIR_BACK_EXCL, TRUE, pts, 1))
        return FALSE;
    *point = pts[0] + pts[1] * I;
    return TRUE;
}

static const char *warp_name(int type) {
    return type == OM_WARP_GROW ? "grow" : type == OM_WARP_SHRINK ? "shrink" : "linear";
}

void om_liquify_overlay(OmEngine *engine, const OmSpace *space, dt_iop_module_t *module, JsonBuilder *builder) {
    (void)engine;
    OmLiquifyNode *nodes = liquify_nodes(module);
    json_builder_set_member_name(builder, "nodes");
    json_builder_begin_array(builder);
    for (int k = 0; nodes && k < OM_LIQUIFY_NODES && nodes[k].header.type != OM_PATH_INVALIDATED; ++k) {
        const OmLiquifyNode *n = &nodes[k];
        float pts[10] = {crealf(n->warp.point),    cimagf(n->warp.point),  crealf(n->warp.strength),
                         cimagf(n->warp.strength), crealf(n->warp.radius), cimagf(n->warp.radius),
                         crealf(n->node.ctrl1),    cimagf(n->node.ctrl1),  crealf(n->node.ctrl2),
                         cimagf(n->node.ctrl2)};
        const int count = n->header.type == OM_PATH_CURVE_TO ? 5 : 3;
        if (!liquify_to_preview(space, module, pts, count))
            continue;
        json_builder_begin_object(builder);
        json_builder_set_member_name(builder, "index");
        json_builder_add_int_value(builder, k);
        json_builder_set_member_name(builder, "type");
        json_builder_add_string_value(builder, n->header.type == OM_PATH_MOVE_TO   ? "move"
                                               : n->header.type == OM_PATH_LINE_TO ? "line"
                                                                                   : "curve");
        json_builder_set_member_name(builder, "prev");
        json_builder_add_int_value(builder, n->header.prev);
        json_builder_set_member_name(builder, "next");
        json_builder_add_int_value(builder, n->header.next);
        json_builder_set_member_name(builder, "warp");
        json_builder_add_string_value(builder, warp_name(n->warp.type));
        om_json_point(builder, "center", pts[0], pts[1]);
        om_json_point(builder, "strength", pts[2], pts[3]);
        om_json_point(builder, "radius", pts[4], pts[5]);
        if (count == 5) {
            om_json_point(builder, "ctrl1", pts[6], pts[7]);
            om_json_point(builder, "ctrl2", pts[8], pts[9]);
        }
        json_builder_end_object(builder);
    }
    json_builder_end_array(builder);
}

// dt_conf_get_sanitize_float (liquify.c:2987): an out-of-range value moves a quarter of the
// way to the default, and is stored back.
static float conf_sanitize(const char *name, float min, float max, float default_value) {
    const float value = dt_conf_get_float(name);
    float new_value = CLAMP(value, min, max);
    if (default_value != 0.0f && new_value != value)
        new_value = 0.25f * default_value + 0.75f * value;
    dt_conf_set_float(name, new_value);
    return new_value;
}

// The stamp of a new warp (get_stamp_params, liquify.c:3003): radius about 9 % of the image's
// shorter side at fit, strength 1.5 × radius, angle 0, or the last values used. `scale`
// is the viewport's shorter side over the displayed image's (1 at fit).
static void stamp(const OmSpace *space, double scale, float *radius, float *strength, float *phi) {
    const float im_scale = 0.09f * MIN(space->iwidth, space->iheight) * (scale > 0 ? scale : 1.0);
    *radius = conf_sanitize("plugins/darkroom/liquify/radius", 0.1f * im_scale, 3.0f * im_scale, im_scale);
    *strength = conf_sanitize("plugins/darkroom/liquify/strength", 0.5f * *radius, 2.0f * *radius,
                                           1.5f * *radius);
    *phi = conf_sanitize("plugins/darkroom/liquify/angle", -M_PI, M_PI, 0.0f);
}

static int parse_warp(const char *name) {
    if (!g_strcmp0(name, "grow"))
        return OM_WARP_GROW;
    if (!g_strcmp0(name, "shrink"))
        return OM_WARP_SHRINK;
    if (!g_strcmp0(name, "linear"))
        return OM_WARP_LINEAR;
    return -1;
}

int om_liquify_edit(OmEngine *engine, const OmSpace *space, dt_iop_module_t *module, JsonObject *gesture) {
    OmLiquifyNode *nodes = liquify_nodes(module);
    if (!nodes)
        return 4;
    const char *action = json_object_get_string_member_with_default(gesture, "action", "");
    float a[2], b[2];
    if (!strcmp(action, "add-point") || !strcmp(action, "add-line") || !strcmp(action, "add-curve")) {
        // The point, line and curve tools (liquify.c:3177 _start_new_shape, button_released):
        // a move-to node, and for a line or curve a second node linked after it.
        float complex start, end = 0;
        const gboolean path = strcmp(action, "add-point") != 0;
        if (!om_gesture_point(gesture, path ? "from" : "at", a) || !liquify_from_preview(space, module, a, &start))
            return 5;
        if (path && (!om_gesture_point(gesture, "to", b) || !liquify_from_preview(space, module, b, &end)))
            return 5;
        float radius, strength, phi;
        stamp(space, om_gesture_number(gesture, "scale", 1.0), &radius, &strength, &phi);
        OmLiquifyNode *first = node_alloc(nodes, OM_PATH_MOVE_TO, start);
        if (!first)
            return 4;
        first->warp.radius = start + radius;
        first->warp.strength = start + strength * cexpf(phi * I);
        if (path) {
            OmLiquifyNode *second = node_alloc(nodes, strcmp(action, "add-line") ? OM_PATH_CURVE_TO : OM_PATH_LINE_TO, end);
            if (!second) {
                first->header.type = OM_PATH_INVALIDATED;
                node_gc(nodes);
                return 4;
            }
            second->warp.radius = end + radius;
            second->warp.strength = end + strength * cexpf(phi * I);
            second->node.ctrl1 = start + (end - start) / 3.0f;
            second->node.ctrl2 = start + 2.0f * (end - start) / 3.0f;
            first->header.next = second->header.idx;
            second->header.prev = first->header.idx;
        }
    } else {
        OmLiquifyNode *n = node_get(nodes, (int)om_gesture_number(gesture, "index", -1));
        if (!n || n->header.type == OM_PATH_INVALIDATED)
            return 5;
        if (!strcmp(action, "move")) {
            // mouse_moved while dragging a layer (liquify.c:2876).
            const char *part = json_object_get_string_member_with_default(gesture, "part", "center");
            float complex pt;
            if (!om_gesture_point(gesture, "to", a) || !liquify_from_preview(space, module, a, &pt))
                return 5;
            if (!strcmp(part, "center")) {
                OmLiquifyNode *p = node_prev(nodes, n), *nx = node_next(nodes, n);
                const float complex delta = pt - n->warp.point;
                if (n->header.type == OM_PATH_CURVE_TO)
                    n->node.ctrl2 += delta;
                if (nx && nx->header.type == OM_PATH_CURVE_TO)
                    nx->node.ctrl1 += delta;
                if (p && p->header.type == OM_PATH_CURVE_TO)
                    p->node.ctrl2 += delta;
                n->warp.radius += delta;
                n->warp.strength += delta;
                n->warp.point = pt;
            } else if (!strcmp(part, "radius")) {
                n->warp.radius = pt;
                dt_conf_set_float("plugins/darkroom/liquify/radius", cabsf(n->warp.radius - n->warp.point));
            } else if (!strcmp(part, "strength")) {
                n->warp.strength = pt;
                dt_conf_set_float("plugins/darkroom/liquify/strength", cabsf(n->warp.strength - n->warp.point));
                dt_conf_set_float("plugins/darkroom/liquify/angle", cargf(n->warp.strength - n->warp.point));
            } else if (!strcmp(part, "ctrl1") && n->header.type == OM_PATH_CURVE_TO) {
                n->node.ctrl1 = pt;
                n->header.node_type = 0; // cusp: the user placed it
            } else if (!strcmp(part, "ctrl2") && n->header.type == OM_PATH_CURVE_TO) {
                n->node.ctrl2 = pt;
                n->header.node_type = 0;
            } else
                return 5;
        } else if (!strcmp(action, "warp")) {
            const int type = parse_warp(json_object_get_string_member_with_default(gesture, "value", ""));
            if (type < 0)
                return 5;
            n->warp.type = type;
        } else if (!strcmp(action, "remove"))
            node_delete(nodes, n);
        else if (!strcmp(action, "remove-path"))
            path_delete(nodes, n);
        else
            return 3;
    }
    smooth_paths_linsys(nodes);
    return om_canvas_commit(engine, module, FALSE);
}
