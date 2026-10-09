// SPDX-License-Identifier: GPL-3.0-or-later
// darktable's mask manager (src/libs/masks.c) for one module's drawn mask, and rasterfile's
// "vectorize" (src/iop/rasterfile.c:160-188), without their GTK code. Both run as module tools:
//   "masks"     gui {"action": ..., "id": formid, "value": n, "name": text}; every result lists
//               the module's group (in darktable's tree order: last member first) and the
//               shapes that can be added. Actions, as darktable's tree context menu (1124-1185):
//               list · add (add existing shape, _tree_add_exist / masks.c _menu_add_exist 1554) ·
//               same (use same shapes as another module, dt_masks_iop_use_same_as 1587) ·
//               mode (value: DT_MASKS_STATE_UNION | INTERSECTION | DIFFERENCE | SUM | EXCLUSION,
//               _tree_operation) · invert (use inverted shape) · up / down (move up / move down,
//               dt_masks_form_move) · remove (remove from group: the shape stays) · opacity ·
//               rename (the shape's name) · duplicate (duplicate this shape, a copy that no group
//               uses) · delete (delete this shape from every group and the image) · cleanup
//               (delete unused shapes, dt_masks_cleanup_unused) · property (a "properties"
//               slider: property size, hardness, feather, rotation, curvature or compression
//               moved from old to value; the result lists them as "properties")
//   "vectorize" (rasterfile) the raster mask file traced into path shapes (ras2forms, threshold
//               0.6) registered with the image (dt_masks_register_forms), ready to be added.
// darktable's own helpers write darktable.develop, the GUI's develop context; the group lists are
// edited here on the engine's develop context instead, and recorded with
// dt_dev_add_masks_history_item_ext as the GUI records them.
#include "module_tools_internal.h"
#include "canvas_internal.h"
#include "common/ras2vect.h"
#include "imageio/imageio_png.h"
#include "common/pfm.h"
#include "common/history.h"

static dt_masks_form_t *group_of(dt_develop_t *dev, const dt_iop_module_t *module) {
    if (!module->blend_params)
        return NULL;
    dt_masks_form_t *group = dt_masks_get_from_id(dev, module->blend_params->mask_id);
    return group && (group->type & DT_MASKS_GROUP) ? group : NULL;
}

static dt_masks_point_group_t *member_of(dt_masks_form_t *group, dt_mask_id_t id) {
    for (GList *it = group ? group->points : NULL; it; it = it->next)
        if (((dt_masks_point_group_t *)it->data)->formid == id)
            return it->data;
    return NULL;
}

// masks.c _check_id: a form id not used yet.
static void unique_id(dt_develop_t *dev, dt_masks_form_t *form) {
    dt_mask_id_t nid = 100;
    for (GList *it = dev->forms; it;)
        if (((dt_masks_form_t *)it->data)->formid == form->formid) {
            form->formid = nid++;
            it = dev->forms;
        } else
            it = it->next;
}

// masks.c _group_create (304) with _set_group_name_from_module (296).
static dt_masks_form_t *group_create(dt_develop_t *dev, dt_iop_module_t *module) {
    dt_masks_form_t *group = dt_masks_create(DT_MASKS_GROUP);
    gchar *label = dt_history_item_get_name(module);
    snprintf(group->name, sizeof(group->name), _("group `%s'"), label);
    g_free(label);
    unique_id(dev, group);
    dev->forms = g_list_append(dev->forms, group);
    module->blend_params->mask_id = group->formid;
    return group;
}

static gboolean used_by_any_group(dt_develop_t *dev, dt_mask_id_t id) {
    for (GList *it = dev->forms; it; it = it->next) {
        dt_masks_form_t *f = it->data;
        if ((f->type & DT_MASKS_GROUP) && member_of(f, id))
            return TRUE;
    }
    return FALSE;
}

static const char *type_name(const dt_masks_form_t *form) {
    if (form->type & DT_MASKS_GROUP)
        return "group";
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
    return "shape";
}

// The mask manager's "properties" sliders (libs/masks.c _masks_properties 110, without opacity,
// which each shape row has, and the brush and object-mask preferences): they change the shape
// selected on the photo, or every shape of the group while none is selected.
static const struct {
    dt_masks_property_t property;
    const char *key, *unit;
    float min, max;
    gboolean relative;
} om_mask_properties[] = {
    {DT_MASKS_PROPERTY_SIZE, "size", "%", 0.0001f, 1, TRUE},
    {DT_MASKS_PROPERTY_HARDNESS, "hardness", "%", 0.0001f, 1, TRUE},
    {DT_MASKS_PROPERTY_FEATHER, "feather", "%", 0.0001f, 1, TRUE},
    {DT_MASKS_PROPERTY_ROTATION, "rotation", "°", 0, 360, FALSE},
    {DT_MASKS_PROPERTY_CURVATURE, "curvature", "%", -1, 1, FALSE},
    {DT_MASKS_PROPERTY_COMPRESSION, "compression", "%", 0.0001f, 1, TRUE},
};

// _property_changed (libs/masks.c:131-250), the branch for a group: every shape (or the selected
// one) takes the slider's change from old to value through its own modify_property, and the
// slider shows their mean within the range all of them allow. old == value only reads.
// Returns the number of shapes that have the property.
static int mask_property(dt_develop_t *dev, dt_masks_form_t *group, int index, float old, float value, float *mean,
                         float *low, float *high) {
    float min = om_mask_properties[index].min, max = om_mask_properties[index].max, sum = 0;
    int count = 0;
    if (om_mask_properties[index].relative) {
        max /= min;
        min /= om_mask_properties[index].max;
    } else {
        max -= min;
        min -= om_mask_properties[index].max;
    }
    // brush.c _brush_modify_property (3210) asks the GUI whether a stroke is being painted and
    // which node is selected: none here, so the whole stroke changes.
    dt_masks_form_gui_t gui = {0};
    gui.point_selected = -1;
    dt_masks_form_gui_t *saved = darktable.develop ? darktable.develop->form_gui : NULL;
    for (GList *it = group ? group->points : NULL; it; it = it->next) {
        const dt_masks_point_group_t *pt = it->data;
        dt_masks_form_t *sel = dt_masks_get_from_id(dev, pt->formid);
        if (!sel || (dev->mask_form_selected_id && dev->mask_form_selected_id != sel->formid))
            continue;
        if (!sel->functions || !sel->functions->modify_property)
            continue;
        if ((sel->type & DT_MASKS_BRUSH) && !darktable.develop)
            continue;
        if (darktable.develop)
            darktable.develop->form_gui = &gui;
        sel->functions->modify_property(sel, om_mask_properties[index].property, old, value, &sum, &count, &min, &max);
        if (darktable.develop)
            darktable.develop->form_gui = saved;
    }
    if (!count)
        return 0;
    *mean = sum / count;
    if (om_mask_properties[index].relative) {
        max *= *mean;
        min *= *mean;
    } else {
        max += *mean;
        min += *mean;
    }
    *low = isnan(min) ? om_mask_properties[index].min : min;
    *high = isnan(max) ? om_mask_properties[index].max : max;
    return count;
}

static void report(OmToolContext *ctx) {
    dt_develop_t *dev = ctx->dev;
    dt_iop_module_t *module = ctx->module;
    dt_masks_form_t *group = group_of(dev, module);
    JsonBuilder *b = json_builder_new();
    json_builder_begin_object(b);
    json_builder_set_member_name(b, "group");
    json_builder_begin_array(b);
    // libs/masks.c shows a group's members last first
    for (GList *it = group ? g_list_last(group->points) : NULL; it; it = it->prev) {
        const dt_masks_point_group_t *pt = it->data;
        const dt_masks_form_t *form = dt_masks_get_from_id(dev, pt->formid);
        if (!form)
            continue;
        json_builder_begin_object(b);
        json_builder_set_member_name(b, "id");
        json_builder_add_int_value(b, pt->formid);
        json_builder_set_member_name(b, "name");
        json_builder_add_string_value(b, form->name);
        json_builder_set_member_name(b, "type");
        json_builder_add_string_value(b, type_name(form));
        json_builder_set_member_name(b, "state");
        json_builder_add_int_value(b, pt->state);
        json_builder_set_member_name(b, "opacity");
        json_builder_add_double_value(b, pt->opacity);
        json_builder_set_member_name(b, "first");
        json_builder_add_boolean_value(b, it == group->points);
        json_builder_end_object(b);
    }
    json_builder_end_array(b);
    // the properties of the selected shape, or of all shapes (libs/masks.c _update_all_properties)
    json_builder_set_member_name(b, "selected");
    json_builder_add_int_value(b, member_of(group, dev->mask_form_selected_id) ? dev->mask_form_selected_id : 0);
    json_builder_set_member_name(b, "properties");
    json_builder_begin_array(b);
    for (int i = 0; i < (int)G_N_ELEMENTS(om_mask_properties); ++i) {
        float mean = 0, low = 0, high = 0;
        if (!mask_property(dev, group, i, 1.0f, 1.0f, &mean, &low, &high))
            continue;
        json_builder_begin_object(b);
        json_builder_set_member_name(b, "key");
        json_builder_add_string_value(b, om_mask_properties[i].key);
        json_builder_set_member_name(b, "unit");
        json_builder_add_string_value(b, om_mask_properties[i].unit);
        json_builder_set_member_name(b, "relative");
        json_builder_add_boolean_value(b, om_mask_properties[i].relative);
        json_builder_set_member_name(b, "value");
        json_builder_add_double_value(b, mean);
        json_builder_set_member_name(b, "min");
        json_builder_add_double_value(b, low);
        json_builder_set_member_name(b, "max");
        json_builder_add_double_value(b, high);
        json_builder_end_object(b);
    }
    json_builder_end_array(b);
    // dt_masks_iop_combo_populate (masks.c:1651-1700): shapes that are not clone shapes, groups
    // or already in this group, and modules with drawn masks to share
    json_builder_set_member_name(b, "available");
    json_builder_begin_array(b);
    for (GList *it = dev->forms; it; it = it->next) {
        const dt_masks_form_t *form = it->data;
        if ((form->type & (DT_MASKS_CLONE | DT_MASKS_NON_CLONE | DT_MASKS_GROUP)) || member_of(group, form->formid))
            continue;
        json_builder_begin_object(b);
        json_builder_set_member_name(b, "id");
        json_builder_add_int_value(b, form->formid);
        json_builder_set_member_name(b, "name");
        json_builder_add_string_value(b, form->name);
        json_builder_set_member_name(b, "type");
        json_builder_add_string_value(b, type_name(form));
        json_builder_set_member_name(b, "used");
        json_builder_add_boolean_value(b, used_by_any_group(dev, form->formid));
        json_builder_end_object(b);
    }
    json_builder_end_array(b);
    json_builder_set_member_name(b, "modules");
    json_builder_begin_array(b);
    for (GList *it = dev->iop; it; it = it->next) {
        dt_iop_module_t *m = it->data;
        dt_masks_form_t *g = m != module ? group_of(dev, m) : NULL;
        if (!g || !g->points || (g->type & DT_MASKS_CLONE))
            continue;
        json_builder_begin_object(b);
        json_builder_set_member_name(b, "operation");
        json_builder_add_string_value(b, m->op);
        json_builder_set_member_name(b, "instance");
        json_builder_add_int_value(b, m->multi_priority);
        gchar *label = dt_history_item_get_name(m);
        json_builder_set_member_name(b, "label");
        json_builder_add_string_value(b, label);
        g_free(label);
        json_builder_end_object(b);
    }
    json_builder_end_array(b);
    json_builder_end_object(b);
    JsonNode *root = json_builder_get_root(b);
    json_object_set_member(ctx->extra, "masks", root);
    g_object_unref(b);
}

static const char *gui_string(OmToolContext *ctx, const char *name) {
    return ctx->gui ? json_object_get_string_member_with_default(ctx->gui, name, NULL) : NULL;
}

static int commit(OmToolContext *ctx, dt_iop_module_t *module) {
    dt_dev_add_masks_history_item_ext(ctx->dev, module, TRUE, FALSE);
    ctx->dev->full.pipe->changed |= DT_DEV_PIPE_SYNCH;
    json_object_set_boolean_member(ctx->extra, "forms_changed", TRUE);
    return 0;
}

static int masks_tool(OmToolContext *ctx) {
    dt_develop_t *dev = ctx->dev;
    dt_iop_module_t *module = ctx->module;
    if (!module->blend_params || !(module->flags() & IOP_FLAGS_SUPPORTS_BLENDING) ||
        (module->flags() & IOP_FLAGS_NO_MASKS))
        return 3;
    const char *action = gui_string(ctx, "action");
    if (!action || !strcmp(action, "list")) {
        report(ctx);
        return 0;
    }
    const dt_mask_id_t id = (dt_mask_id_t)om_tool_gui(ctx, "id", 0);
    dt_masks_form_t *group = group_of(dev, module);
    dt_masks_point_group_t *member = member_of(group, id);
    dt_masks_form_t *form = dt_masks_get_from_id(dev, id);
    int error = 0;
    if (!strcmp(action, "add")) {
        if (!form || (form->type & (DT_MASKS_CLONE | DT_MASKS_NON_CLONE)))
            return 5;
        if (!group)
            group = group_create(dev, module);
        if (!dt_masks_group_add_form(group, form))
            return 5;
        // the blend section only offers shapes while its mask mode draws them
        module->blend_params->mask_mode |= DEVELOP_MASK_ENABLED | DEVELOP_MASK_MASK;
        error = commit(ctx, module);
    } else if (!strcmp(action, "same")) {
        const char *operation = gui_string(ctx, "operation");
        dt_iop_module_t *src = operation ? dt_iop_get_module_by_op_priority(dev->iop, operation,
                                                                           (int)om_tool_gui(ctx, "instance", 0))
                                         : NULL;
        dt_masks_form_t *src_group = src && src != module ? group_of(dev, src) : NULL;
        if (!src_group || src_group->type != DT_MASKS_GROUP)
            return 5;
        if (!group)
            group = group_create(dev, module);
        for (GList *it = src_group->points; it; it = it->next) {
            const dt_masks_point_group_t *pt = it->data;
            const dt_masks_form_t *f = dt_masks_get_from_id(dev, pt->formid);
            dt_masks_point_group_t *added = f ? dt_masks_group_add_form(group, f) : NULL;
            if (added) {
                added->state = pt->state;
                added->opacity = pt->opacity;
            }
        }
        module->blend_params->mask_mode |= DEVELOP_MASK_ENABLED | DEVELOP_MASK_MASK;
        error = commit(ctx, module);
    } else if (!strcmp(action, "cleanup")) {
        dt_masks_cleanup_unused(dev);
        json_object_set_boolean_member(ctx->extra, "forms_changed", TRUE);
    } else if (!strcmp(action, "property")) {
        // a "properties" slider moved from "old" to "value" (both as darktable's slider holds them)
        const char *key = gui_string(ctx, "property");
        int index = -1;
        for (int i = 0; key && i < (int)G_N_ELEMENTS(om_mask_properties); ++i)
            if (!strcmp(key, om_mask_properties[i].key))
                index = i;
        const float old = om_tool_gui(ctx, "old", 0), value = om_tool_gui(ctx, "value", 0);
        float mean = 0, low = 0, high = 0;
        if (index < 0 || !group || !mask_property(dev, group, index, old, value, &mean, &low, &high))
            return 5;
        if (value != old)
            error = commit(ctx, module);
    } else if (!strcmp(action, "rename")) {
        const char *name = gui_string(ctx, "name");
        if (!form || !name || !name[0])
            return 5;
        g_strlcpy(form->name, name, sizeof(form->name));
        error = commit(ctx, module);
    } else if (!strcmp(action, "duplicate")) {
        if (!form || (form->type & DT_MASKS_GROUP))
            return 5;
        // masks.c dt_masks_form_duplicate (424-442) on this develop context
        dt_masks_form_t *copy = dt_masks_dup_masks_form(form);
        copy->formid = (dt_mask_id_t)time(NULL);
        unique_id(dev, copy);
        snprintf(copy->name, sizeof(copy->name), _("copy of `%s'"), form->name);
        dev->forms = g_list_append(dev->forms, copy);
        error = commit(ctx, module);
    } else if (!strcmp(action, "delete")) {
        // _tree_delete_shape outside a group: the shape leaves every group and the image
        if (!form || (form->type & DT_MASKS_GROUP))
            return 5;
        for (GList *it = dev->forms; it; it = it->next) {
            dt_masks_form_t *g = it->data;
            dt_masks_point_group_t *pt = (g->type & DT_MASKS_GROUP) ? member_of(g, id) : NULL;
            if (pt) {
                g->points = g_list_remove(g->points, pt);
                free(pt);
            }
        }
        dev->forms = g_list_remove(dev->forms, form);
        dt_masks_free_form(form);
        if (dev->mask_form_selected_id == id)
            dev->mask_form_selected_id = 0;
        error = commit(ctx, module);
    } else {
        if (!member)
            return 5;
        if (!strcmp(action, "mode")) {
            const int mode = (int)om_tool_gui(ctx, "value", DT_MASKS_STATE_UNION) & DT_MASKS_STATE_OP;
            if (!mode)
                return 5;
            member->state = (member->state & ~DT_MASKS_STATE_OP) | mode;
        } else if (!strcmp(action, "invert"))
            member->state ^= DT_MASKS_STATE_INVERSE;
        else if (!strcmp(action, "up") || !strcmp(action, "down"))
            dt_masks_form_move(group, id, !strcmp(action, "up"));
        else if (!strcmp(action, "opacity"))
            member->opacity = CLAMP(om_tool_gui(ctx, "value", member->opacity), 0.05f, 1.0f);
        else if (!strcmp(action, "remove")) {
            // remove from group: the shape stays with the image (libs/masks.c _tree_delete_shape)
            group->points = g_list_remove(group->points, member);
            free(member);
            if (dev->mask_form_selected_id == id)
                dev->mask_form_selected_id = 0;
        } else
            return 3;
        error = commit(ctx, module);
    }
    report(ctx);
    return error;
}

// rasterfile.c _read_rasterfile (190-305) for the vectorizer: the mask from the file's channels
// given by mode, 0..1.
static float *read_raster(const char *filename, int mode, int *width, int *height) {
    *width = *height = 0;
    const char *extension = filename ? strrchr(filename, '.') : NULL;
    if (!extension)
        return NULL;
    if (!g_ascii_strcasecmp(extension, ".png")) {
        dt_imageio_png_t png;
        if (!dt_imageio_png_read_header(filename, &png))
            return NULL;
        const size_t rowbytes = png_get_rowbytes(png.png_ptr, png.info_ptr);
        uint8_t *buf = dt_alloc_aligned((size_t)png.height * rowbytes);
        if (!buf) {
            fclose(png.f);
            png_destroy_read_struct(&png.png_ptr, &png.info_ptr, NULL);
            return NULL;
        }
        if (!dt_imageio_png_read_image(&png, buf)) {
            dt_free_align(buf);
            return NULL;
        }
        const int w = png.width, h = png.height;
        float *mask = dt_iop_image_alloc(w, h, 1);
        if (mask)
            for (size_t k = 0; k < (size_t)w * h; k++) {
                float val = 0.0f, r, g, b;
                if (png.bit_depth < 16) {
                    r = buf[3 * k] / 255.0f, g = buf[3 * k + 1] / 255.0f, b = buf[3 * k + 2] / 255.0f;
                } else {
                    r = (buf[6 * k] * 256.0f + buf[6 * k + 1]) / 65535.0f;
                    g = (buf[6 * k + 2] * 256.0f + buf[6 * k + 3]) / 65535.0f;
                    b = (buf[6 * k + 4] * 256.0f + buf[6 * k + 5]) / 65535.0f;
                }
                if (mode & 1)
                    val = MAX(val, r);
                if (mode & 2)
                    val = MAX(val, g);
                if (mode & 4)
                    val = MAX(val, b);
                mask[k] = CLIP(val);
            }
        dt_free_align(buf);
        *width = w;
        *height = h;
        return mask;
    }
    int w = 0, h = 0, channels = 0, error = 0;
    float *image = dt_read_pfm(filename, &error, &w, &h, &channels, 3);
    float *mask = image ? dt_iop_image_alloc(w, h, 1) : NULL;
    if (!image || !mask) {
        dt_free_align(image);
        dt_free_align(mask);
        return NULL;
    }
    for (size_t k = 0; k < (size_t)w * h; k++) {
        float val = 0.0f;
        if (mode & 1)
            val = MAX(val, image[k * 3]);
        if (mode & 2)
            val = MAX(val, image[k * 3 + 1]);
        if (mode & 4)
            val = MAX(val, image[k * 3 + 2]);
        mask[k] = CLIP(val);
    }
    dt_free_align(image);
    *width = w;
    *height = h;
    return mask;
}

static int rasterfile_vectorize(OmToolContext *ctx) {
    dt_iop_module_t *m = ctx->module;
    const char *path = m->get_f && m->get_f("path") ? m->get_p(m->params, "path") : NULL;
    const char *file = m->get_f && m->get_f("file") ? m->get_p(m->params, "file") : NULL;
    const int *mode = m->get_f && m->get_f("mode") ? m->get_p(m->params, "mode") : NULL;
    if (!path || !file || !mode || !path[0] || !file[0])
        return 5;
    gchar *filename = g_build_filename(path, file, NULL);
    int width = 0, height = 0;
    float *mask = read_raster(filename, *mode, &width, &height);
    g_free(filename);
    if (!mask)
        return 5;
    GList *forms = ras2forms(mask, width, height, &ctx->dev->image_storage, 0.6f, 0, 0.0, NULL);
    dt_free_align(mask);
    const int count = g_list_length(forms);
    json_object_set_int_member(ctx->extra, "shapes", count);
    if (!count) {
        json_object_set_string_member(ctx->extra, "message",
                                      "no mask extracted from the raster file\nmake sure the masks have proper contrast");
        return 0;
    }
    for (GList *it = forms; it; it = it->next)
        unique_id(ctx->dev, it->data);
    // dt_masks_register_forms (masks.c:322) on this develop context
    for (GList *it = forms; it; it = it->next)
        ctx->dev->forms = g_list_append(ctx->dev->forms, it->data);
    g_list_free(forms);
    dt_dev_add_masks_history_item_ext(ctx->dev, NULL, TRUE, FALSE);
    json_object_set_boolean_member(ctx->extra, "forms_changed", TRUE);
    gchar *message = g_strdup_printf(ngettext("%d mask extracted from the raster file",
                                              "%d masks extracted from the raster file", count),
                                     count);
    json_object_set_string_member(ctx->extra, "message", message);
    g_free(message);
    return 0;
}

const OmToolSpec om_tools_masks[] = {
    {"*", "masks", OM_TOOL_BUTTON, -1, OM_TOOL_KEEP_OFF, masks_tool},
    {"rasterfile", "vectorize", OM_TOOL_BUTTON, -1, OM_TOOL_KEEP_OFF, rasterfile_vectorize},
    {NULL, NULL, 0, 0, 0, NULL},
};
