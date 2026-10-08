// SPDX-License-Identifier: GPL-3.0-or-later
// darktable's multi-instance menu without GTK (develop/imageop.c, release-5.6.1:
// _gui_copy_callback / _gui_duplicate_callback → dt_iop_gui_duplicate, _gui_moveup_callback,
// _gui_movedown_callback, _gui_delete_callback, dt_iop_gui_rename_module → _rename_module_key_press,
// and _get_multi_show for what the menu offers). The darktable calls that need a GUI
// (dt_dev_add_history_item, dt_dev_module_remove's history cleanup) return early when no GUI
// is attached, so their headless equivalents are spelled out here.
#include "engine_internal.h"
#include "module_instances.h"
#include "common/history.h"
#include "common/utility.h"
#include "develop/masks.h"
#include <limits.h>

// darktable moves an instance past the next module shown in the right-hand panel. Without that
// panel, a module counts as shown when it has a GUI, and deprecated ones only while enabled, as
// in Omalux's module panes.
static gboolean shown(dt_iop_module_t *module) {
    return !dt_iop_is_hidden(module) && (!(module->flags() & IOP_FLAGS_DEPRECATED) || module->enabled);
}

// dt_iop_gui_get_next_visible_module (later = TRUE) / dt_iop_gui_get_previous_visible_module.
static dt_iop_module_t *neighbour(dt_develop_t *dev, dt_iop_module_t *module, gboolean later) {
    GList *link = g_list_find(dev->iop, module);
    for (link = link ? (later ? link->next : link->prev) : NULL; link; link = later ? link->next : link->prev)
        if (shown(link->data))
            return link->data;
    return NULL;
}

static gboolean can_move(dt_develop_t *dev, dt_iop_module_t *module, gboolean later) {
    dt_iop_module_t *other = neighbour(dev, module, later);
    if (!other || other->iop_order == INT_MAX)
        return FALSE;
    return later ? dt_ioppr_check_can_move_after_iop(dev->iop, module, other)
                 : dt_ioppr_check_can_move_before_iop(dev->iop, module, other);
}

static int count_instances(dt_develop_t *dev, dt_iop_module_t *module) {
    int count = 0;
    for (GList *it = dev->iop; it; it = it->next)
        if (((dt_iop_module_t *)it->data)->instance == module->instance)
            ++count;
    return count;
}

static gboolean can_create(dt_iop_module_t *module) {
    return !(module->flags() & IOP_FLAGS_ONE_INSTANCE) && !dt_iop_is_hidden(module);
}

void om_instance_describe(JsonObject *entry, dt_iop_module_t *module) {
    dt_develop_t *dev = module->dev;
    json_object_set_string_member(entry, "multi_name", module->multi_name);
    json_object_set_boolean_member(entry, "multi_name_hand_edited", module->multi_name_hand_edited);
    // What darktable's module header shows after the name (_iop_panel_name): nothing for an
    // empty name or "0", the hand-edited name as typed, otherwise the localised automatic one
    // (preset names such as "_builtin_scene-referred default" lose their prefix).
    gchar *label = !module->multi_name[0] || !strcmp(module->multi_name, "0") ? g_strdup("")
                   : module->multi_name_hand_edited
                       ? g_strdup(module->multi_name)
                       : dt_util_localize_segmented_name(module->multi_name, FALSE);
    json_object_set_string_member(entry, "instance_label", label ? label : "");
    g_free(label);
    // _get_multi_show: what the multi-instance menu offers for this instance.
    json_object_set_boolean_member(entry, "can_new", can_create(module));
    json_object_set_boolean_member(entry, "can_delete",
                                   !dt_iop_is_hidden(module) && count_instances(dev, module) > 1);
    json_object_set_boolean_member(entry, "can_move_up",
                                   !dt_iop_is_hidden(module) && can_move(dev, module, TRUE));
    json_object_set_boolean_member(entry, "can_move_down",
                                   !dt_iop_is_hidden(module) && can_move(dev, module, FALSE));
}

// dt_iop_gui_duplicate without its GUI part.
static int create_instance(OmEngine *engine, dt_iop_module_t *base, gboolean copy_params, int *result) {
    dt_develop_t *dev = &engine->dev;
    if (!can_create(base))
        return 4;
    // darktable copies the shapes of a drawn mask into a new group through the GUI's develop
    // context (dt_masks_iop_use_same_as uses darktable.develop). Refuse rather than drop them.
    if (copy_params && (base->flags() & IOP_FLAGS_SUPPORTS_BLENDING) &&
        dt_is_valid_maskid(base->blend_params->mask_id) &&
        dt_masks_get_from_id(dev, base->blend_params->mask_id))
        return 4;
    // "make sure the duplicated module appears in the history"
    dt_dev_add_history_item_ext(dev, base, FALSE, FALSE);
    dt_iop_module_t *module = dt_dev_module_duplicate(dev, base);
    if (!module)
        return 4;
    dt_iop_reload_defaults(module);
    if (copy_params) {
        memcpy(module->params, base->params, module->params_size);
        if (module->flags() & IOP_FLAGS_SUPPORTS_BLENDING)
            dt_iop_commit_blend_params(module, base->blend_params);
    }
    // "we save the new instance creation"
    dt_dev_add_history_item_ext(dev, module, TRUE, FALSE);
    dt_dev_pixelpipe_rebuild(dev);
    *result = module->multi_priority;
    return 0;
}

// _gui_moveup_callback (later in the pipe) and _gui_movedown_callback (earlier).
static int move_instance(OmEngine *engine, dt_iop_module_t *module, gboolean later, int *result) {
    dt_develop_t *dev = &engine->dev;
    if (dt_iop_is_hidden(module) || !can_move(dev, module, later))
        return 4;
    dt_iop_module_t *other = neighbour(dev, module, later);
    const gboolean moved =
        later ? dt_ioppr_move_iop_after(dev, module, other) : dt_ioppr_move_iop_before(dev, module, other);
    if (!moved)
        return 4;
    dt_dev_add_history_item_ext(dev, module, TRUE, FALSE);
    dt_dev_pixelpipe_rebuild(dev);
    *result = module->multi_priority;
    return 0;
}

// _gui_delete_callback with dt_dev_module_remove's history cleanup.
static int delete_instance(OmEngine *engine, dt_iop_module_t *module, int *result) {
    dt_develop_t *dev = &engine->dev;
    if (dt_iop_is_hidden(module))
        return 4;
    // Another instance of the same module: the previous one if any, otherwise the next.
    dt_iop_module_t *next = NULL;
    gboolean found = FALSE;
    for (GList *it = dev->iop; it; it = it->next) {
        dt_iop_module_t *mod = it->data;
        if (mod == module) {
            found = TRUE;
            if (next)
                break;
        } else if (mod->instance == module->instance) {
            next = mod;
            if (found)
                break;
        }
    }
    if (!next)
        return 4;
    const gboolean is_zero = module->multi_priority == 0;
    dt_pthread_mutex_lock(&dev->history_mutex);
    // dt_dev_module_remove drops the module's history items only with a GUI attached.
    int index = 0;
    for (GList *it = dev->history; it;) {
        GList *following = it->next;
        dt_dev_history_item_t *item = it->data;
        if (item->module == module) {
            dt_dev_free_history_item(item);
            dev->history = g_list_delete_link(dev->history, it);
            if (index < dev->history_end)
                --dev->history_end;
        } else
            ++index;
        it = following;
    }
    dev->iop = g_list_remove(dev->iop, module);
    // Keep the order list keyed by (operation, instance) consistent with the renumbering below.
    GList *entry = dt_ioppr_get_iop_order_link(dev->iop_order_list, module->op, module->multi_priority);
    if (entry) {
        g_free(entry->data);
        dev->iop_order_list = g_list_delete_link(dev->iop_order_list, entry);
    }
    dt_pthread_mutex_unlock(&dev->history_mutex);
    dt_iop_module_t *survivor = next;
    if (is_zero) {
        // The instance first in history becomes instance 0.
        dt_iop_module_t *first = NULL;
        for (GList *it = dev->history; it && !first; it = it->next) {
            dt_dev_history_item_t *item = it->data;
            if (item->module && item->module->instance == module->instance && item->module != module)
                first = item->module;
        }
        if (!first)
            first = next;
        GList *order = dt_ioppr_get_iop_order_link(dev->iop_order_list, first->op, first->multi_priority);
        if (order)
            ((dt_iop_order_entry_t *)order->data)->instance = 0;
        dt_iop_update_multi_priority(first, 0);
        for (GList *it = dev->history; it; it = it->next) {
            dt_dev_history_item_t *item = it->data;
            if (item->module == first)
                item->multi_priority = 0;
        }
        survivor = first;
    }
    om_style_baseline_forget(engine, module);
    // "don't delete the module, a pipe may still need it"; dt_dev_cleanup frees it.
    dev->alliop = g_list_append(dev->alliop, module);
    dt_dev_pixelpipe_rebuild(dev);
    *result = survivor->multi_priority;
    return 0;
}

// _rename_module_key_press → dt_iop_update_multi_name(…, force TRUE).
static int rename_instance(OmEngine *engine, dt_iop_module_t *module, const char *name, int *result) {
    dt_develop_t *dev = &engine->dev;
    char *text = g_strstrip(g_strdup(name ? name : ""));
    if (strlen(text) >= sizeof(module->multi_name)) {
        g_free(text);
        return 5;
    }
    if (*text) {
        if (strcmp(module->multi_name, text) || !module->multi_name_hand_edited) {
            g_strlcpy(module->multi_name, text, sizeof(module->multi_name));
            module->multi_name_hand_edited = TRUE;
            dt_dev_add_history_item_ext(dev, module, TRUE, FALSE);
        }
    } else {
        // An empty name gives the module back its automatic label.
        g_strlcpy(module->multi_name, "", sizeof(module->multi_name));
        module->multi_name_hand_edited = FALSE;
        dt_dev_add_history_item_ext(dev, module, FALSE, FALSE);
    }
    g_free(text);
    *result = module->multi_priority;
    return 0;
}

int om_engine_module_instance(OmEngine *engine, const char *operation, int instance, const char *action,
                              const char *name, int *result) {
    if (!engine->loaded)
        return 1;
    dt_iop_module_t *module = dt_iop_get_module_by_op_priority(engine->dev.iop, operation, instance);
    if (!module)
        return 2;
    int error = 3;
    *result = instance;
    if (!strcmp(action, "new") || !strcmp(action, "duplicate"))
        error = create_instance(engine, module, !strcmp(action, "duplicate"), result);
    else if (!strcmp(action, "up") || !strcmp(action, "down"))
        error = move_instance(engine, module, !strcmp(action, "up"), result);
    else if (!strcmp(action, "delete"))
        error = delete_instance(engine, module, result);
    else if (!strcmp(action, "rename"))
        error = rename_instance(engine, module, name, result);
    if (error)
        return error;
    engine->dev.full.pipe->changed |= DT_DEV_PIPE_SYNCH;
    // Curated controls bind to instance 0, which a deletion may have changed.
    return om_engine_bind_controls(engine) ? 4 : 0;
}
