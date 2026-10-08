// SPDX-License-Identifier: GPL-3.0-or-later
// area E: editing single nodes of a drawn path or brush stroke, as darktable does on the
// canvas (develop/masks/path.c button_pressed 2155-2350, mouse_moved 2554-2665; brush.c
// 1579-1760, 2173-2250). Included by shapes.c (it uses its point helpers and form removal).
//
// The overlay of a path or brush carries "nodes": [{corner, ctrl1, ctrl2, smooth}] (path) or
// [{corner, feather, smooth}] (brush) in preview fractions. Gestures (with "id" and "node"):
//   node-move     {from, to}         drag a node: its handles move with it (point_dragging)
//   node-ctrl     {ctrl: 1|2, to, modifier: ""|"shift"|"ctrl"|"ctrl+shift"}
//                                    drag a path handle; without modifier the handles keep
//                                    their angle and length ratio, shift moves one handle,
//                                    ctrl mirrors it, ctrl+shift keeps the angle only
//                                    (DT_MASKS_BEZIER_*); the node becomes a corner (USER)
//   node-feather  {to}               drag a brush node's feather handle (_brush_feather_to_ctrl)
//   node-toggle                      ctrl+click a node: smooth ↔ sharp corner
//   node-reset                       right-click a handle: smooth again
//   node-remove                      right-click a node; a path of three nodes or a brush of
//                                    two goes as a whole (path.c:2278, brush.c:1696)
//   node-add      {segment, at}      ctrl+click a segment: a new smooth node there, its border
//                                    (and a brush's hardness and density) interpolated
//   segment-move  {segment, from, to} drag a segment: both of its nodes move
#pragma GCC diagnostic push
#pragma GCC diagnostic ignored "-Wunused-function"
#include "shape_nodes_port.inc"
#pragma GCC diagnostic pop
// exported by libdarktable (develop/masks/path.c:387), without a header
void _update_bezier_ctrl_points(dt_masks_point_path_t *point, const float new_x, const float new_y,
                                const dt_masks_path_ctrl_t ctrl_select, const dt_masks_path_edit_mode_t ctrl_mode,
                                const float ctrl_angle, const float ctrl_scale, const float aspect_ratio);
static void path_init_ctrl_points(dt_masks_form_t *form);
static void brush_init_ctrl_points(dt_masks_form_t *form);

typedef struct {
    float *corner, *ctrl1, *ctrl2, *border;
    int *state;
} OmNode;

static gboolean node_at(dt_masks_form_t *form, int index, OmNode *node) {
    void *data = g_list_nth_data(form->points, index);
    if (!data || index < 0)
        return FALSE;
    if (form->type & DT_MASKS_PATH) {
        dt_masks_point_path_t *p = data;
        *node = (OmNode){p->corner, p->ctrl1, p->ctrl2, p->border, (int *)&p->state};
    } else if (form->type & DT_MASKS_BRUSH) {
        dt_masks_point_brush_t *p = data;
        *node = (OmNode){p->corner, p->ctrl1, p->ctrl2, p->border, (int *)&p->state};
    } else
        return FALSE;
    return TRUE;
}

static void init_ctrl_points(dt_masks_form_t *form) {
    if (form->type & DT_MASKS_PATH)
        path_init_ctrl_points(form);
    else
        brush_init_ctrl_points(form);
}

// The pointer (preview fractions) in the shape's input coordinates (fractions of the input).
static gboolean pointer_input(const OmSpace *space, const float *at, float *out) {
    float pts[2] = {at[0] * space->width, at[1] * space->height};
    if (!om_space_backtransform(space, pts, 1))
        return FALSE;
    out[0] = pts[0] / space->iwidth;
    out[1] = pts[1] / space->iheight;
    return TRUE;
}

static void shape_nodes_overlay(const OmSpace *space, dt_masks_form_t *form, JsonBuilder *builder) {
    if (!(form->type & (DT_MASKS_PATH | DT_MASKS_BRUSH)))
        return;
    const gboolean brush = (form->type & DT_MASKS_BRUSH) != 0;
    json_builder_set_member_name(builder, "nodes");
    json_builder_begin_array(builder);
    const int n = g_list_length(form->points);
    for (int k = 0; k < n; ++k) {
        OmNode node;
        node_at(form, k, &node);
        float pts[6] = {node.corner[0] * space->iwidth, node.corner[1] * space->iheight,
                        node.ctrl1[0] * space->iwidth, node.ctrl1[1] * space->iheight,
                        node.ctrl2[0] * space->iwidth, node.ctrl2[1] * space->iheight};
        if (!om_space_transform(space, pts, 3))
            continue;
        json_builder_begin_object(builder);
        if (brush) {
            // the feather handle beside the stroke (brush.c:2338, in output pixels)
            float f[2];
            _brush_ctrl2_to_feather(pts[0], pts[1], pts[4], pts[5], &f[0], &f[1], TRUE);
            om_space_to_preview(space, f, 1);
            om_json_point(builder, "feather", f[0], f[1]);
        }
        om_space_to_preview(space, pts, 3);
        om_json_point(builder, "corner", pts[0], pts[1]);
        if (!brush) {
            om_json_point(builder, "ctrl1", pts[2], pts[3]);
            om_json_point(builder, "ctrl2", pts[4], pts[5]);
        }
        json_builder_set_member_name(builder, "smooth");
        json_builder_add_boolean_value(builder, *node.state == DT_MASKS_POINT_STATE_NORMAL);
        json_builder_end_object(builder);
    }
    json_builder_end_array(builder);
}

static void shift_node(OmNode *node, float dx, float dy) {
    node->corner[0] += dx;
    node->corner[1] += dy;
    node->ctrl1[0] += dx;
    node->ctrl1[1] += dy;
    node->ctrl2[0] += dx;
    node->ctrl2[1] += dy;
}

// -1: not a node gesture; 0 nothing changed; 1 changed; 2 the whole shape goes; else an error.
static int shape_nodes_edit(const OmSpace *space, dt_masks_form_t *form, const char *action, JsonObject *gesture) {
    if (strncmp(action, "node-", 5) && strcmp(action, "segment-move"))
        return -1;
    if (!(form->type & (DT_MASKS_PATH | DT_MASKS_BRUSH)))
        return 5;
    const gboolean path = (form->type & DT_MASKS_PATH) != 0;
    const int n = g_list_length(form->points);
    const int index = (int)om_gesture_number(gesture, "node", -1);
    const float iwidth = space->iwidth, iheight = space->iheight;
    float from[2], to[2], p[2];
    OmNode node;
    if (!strcmp(action, "node-add")) {
        const int segment = (int)om_gesture_number(gesture, "segment", -1);
        // a brush stroke is open: no segment after its last node (brush.c:1629)
        if (segment < 0 || segment >= (path ? n : n - 1) || !om_gesture_point(gesture, "at", to) ||
            !pointer_input(space, to, p))
            return 5;
        OmNode left, right;
        node_at(form, segment, &left);
        node_at(form, (segment + 1) % n, &right);
        void *created;
        if (path) {
            dt_masks_point_path_t *bzpt = malloc(sizeof(dt_masks_point_path_t));
            bzpt->corner[0] = p[0];
            bzpt->corner[1] = p[1];
            bzpt->ctrl1[0] = bzpt->ctrl1[1] = bzpt->ctrl2[0] = bzpt->ctrl2[1] = -1.0;
            bzpt->state = DT_MASKS_POINT_STATE_NORMAL;
            bzpt->border[0] = MAX(0.0005f, (left.border[0] + right.border[0]) * 0.5);
            bzpt->border[1] = MAX(0.0005f, (left.border[1] + right.border[1]) * 0.5);
            created = bzpt;
        } else {
            dt_masks_point_brush_t *bzpt = malloc(sizeof(dt_masks_point_brush_t));
            bzpt->corner[0] = p[0];
            bzpt->corner[1] = p[1];
            bzpt->ctrl1[0] = bzpt->ctrl1[1] = bzpt->ctrl2[0] = bzpt->ctrl2[1] = -1.0;
            bzpt->state = DT_MASKS_POINT_STATE_NORMAL;
            const float t = _brush_get_position_in_segment(bzpt->corner[0], bzpt->corner[1], form, segment);
            const dt_masks_point_brush_t *point0 = g_list_nth_data(form->points, segment),
                                         *point1 = g_list_nth_data(form->points, segment + 1);
            bzpt->border[0] = point0->border[0] * (1.0f - t) + point1->border[0] * t;
            bzpt->border[1] = point0->border[1] * (1.0f - t) + point1->border[1] * t;
            bzpt->hardness = point0->hardness * (1.0f - t) + point1->hardness * t;
            bzpt->density = point0->density * (1.0f - t) + point1->density * t;
            created = bzpt;
        }
        form->points = g_list_insert(form->points, created, segment + 1);
        init_ctrl_points(form);
        return 1;
    }
    if (!strcmp(action, "segment-move")) {
        const int segment = (int)om_gesture_number(gesture, "segment", -1);
        if (segment < 0 || segment >= (path ? n : n - 1) || !om_gesture_point(gesture, "from", from) ||
            !om_gesture_point(gesture, "to", to))
            return 5;
        float a[2], b[2];
        if (!pointer_input(space, from, a) || !pointer_input(space, to, b))
            return 5;
        OmNode first, second;
        node_at(form, segment, &first);
        node_at(form, (segment + 1) % n, &second);
        const float dx = b[0] - a[0], dy = b[1] - a[1];
        // the clone source follows the first and the last segment (path.c:2603)
        if ((form->type & DT_MASKS_CLONE) && (segment == 0 || segment == n - 1)) {
            form->source[0] += dx;
            form->source[1] += dy;
        }
        shift_node(&first, dx, dy);
        shift_node(&second, dx, dy);
        init_ctrl_points(form);
        return 1;
    }
    if (!node_at(form, index, &node))
        return 5;
    if (!strcmp(action, "node-move")) {
        if (!om_gesture_point(gesture, "to", to) || !pointer_input(space, to, p))
            return 5;
        if (path && (form->type & DT_MASKS_CLONE) && index == 0) {
            form->source[0] += p[0] - node.corner[0];
            form->source[1] += p[1] - node.corner[1];
        }
        shift_node(&node, p[0] - node.corner[0], p[1] - node.corner[1]);
        init_ctrl_points(form);
        return 1;
    }
    if (!strcmp(action, "node-ctrl")) {
        const int ctrl = (int)om_gesture_number(gesture, "ctrl", 1);
        if (!path || !om_gesture_point(gesture, "to", to) || !pointer_input(space, to, p))
            return 5;
        const char *modifier = json_object_get_string_member_with_default(gesture, "modifier", "");
        dt_masks_path_edit_mode_t mode = DT_MASKS_BEZIER_NONE;
        if (!strcmp(modifier, "shift"))
            mode = DT_MASKS_BEZIER_SINGLE;
        else if (!strcmp(modifier, "ctrl"))
            mode = DT_MASKS_BEZIER_SYMMETRIC;
        else if (!strcmp(modifier, "ctrl+shift"))
            mode = DT_MASKS_BEZIER_SING_SYMM;
        dt_masks_point_path_t *point = g_list_nth_data(form->points, index);
        // the angle and the length ratio when the handle was grabbed (path.c:2210)
        const float angle = _get_ctrl_angle(point->corner[0], point->corner[1], point->ctrl1[0], point->ctrl1[1],
                                            point->ctrl2[0], point->ctrl2[1], iwidth / iheight);
        const float scale = _get_ctrl_scale(point->corner[0], point->corner[1], point->ctrl1[0], point->ctrl1[1],
                                            point->ctrl2[0], point->ctrl2[1], iwidth / iheight);
        _update_bezier_ctrl_points(point, p[0], p[1], ctrl == 2 ? DT_MASKS_PATH_CTRL2 : DT_MASKS_PATH_CTRL1, mode,
                                   angle, scale, iwidth / iheight);
        point->state = DT_MASKS_POINT_STATE_USER;
        init_ctrl_points(form);
        return 1;
    }
    if (!strcmp(action, "node-feather")) {
        if (path || !om_gesture_point(gesture, "to", to))
            return 5;
        float pts[2] = {to[0] * space->width, to[1] * space->height};
        if (!om_space_backtransform(space, pts, 1))
            return 5;
        float p1x, p1y, p2x, p2y;
        _brush_feather_to_ctrl(node.corner[0] * iwidth, node.corner[1] * iheight, pts[0], pts[1], &p1x, &p1y, &p2x,
                               &p2y, TRUE);
        node.ctrl1[0] = p1x / iwidth;
        node.ctrl1[1] = p1y / iheight;
        node.ctrl2[0] = p2x / iwidth;
        node.ctrl2[1] = p2y / iheight;
        *node.state = DT_MASKS_POINT_STATE_USER;
        init_ctrl_points(form);
        return 1;
    }
    if (!strcmp(action, "node-toggle")) {
        if (*node.state != DT_MASKS_POINT_STATE_NORMAL) {
            *node.state = DT_MASKS_POINT_STATE_NORMAL;
            init_ctrl_points(form);
        } else {
            node.ctrl1[0] = node.ctrl2[0] = node.corner[0];
            node.ctrl1[1] = node.ctrl2[1] = node.corner[1];
            *node.state = DT_MASKS_POINT_STATE_USER;
        }
        return 1;
    }
    if (!strcmp(action, "node-reset")) {
        if (*node.state == DT_MASKS_POINT_STATE_NORMAL)
            return 0;
        *node.state = DT_MASKS_POINT_STATE_NORMAL;
        init_ctrl_points(form);
        return 1;
    }
    if (!strcmp(action, "node-remove")) {
        if (n < (path ? 4 : 3))
            return 2;
        void *data = g_list_nth_data(form->points, index);
        form->points = g_list_remove(form->points, data);
        free(data);
        init_ctrl_points(form);
        return 1;
    }
    return 3;
}
