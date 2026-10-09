// SPDX-License-Identifier: GPL-3.0-or-later
// Film profiles: a lookup table for darktable's LUT 3D module, put on the photograph as a base
// rendering before any look. A look carries its own lookup table in LUT 3D too, so the film
// gets an instance of its own, placed before the base instance (multi_priority 0), which stays
// the look's and the one the LUT 3D controls address.
//
// Checked against release-5.6.1:
// - dt_dev_module_duplicate_ext (develop/develop.c 3520) with reorder FALSE leaves the new
//   instance where dt_ioppr_insert_module_instance (common/iop_order.c) puts its order entry:
//   before the operation's last entry. With further instances present the film is moved before
//   the base one (dt_ioppr_move_iop_before).
// - Applying a style item (common/styles.c dt_styles_apply_style_item →
//   common/history.c dt_history_merge_module_into_history, 309) picks the instance to replace
//   among those not in `modules_used`: same name, else a disabled one, else one at its
//   defaults, else the same priority. The film instance is handed in as already used, so a
//   look's lut3d item never lands on it, whatever state it is in.
// - Before that, dt_ioppr_update_for_style_items → _ioppr_update_for_entries
//   (common/iop_order.c) renumbers the item to the priority of the n-th instance in pipe
//   order (_get_multi_priority), which is the film's, as it comes first. The item is given the
//   base instance's priority and order back (om_film_style_items), so the merge finds the base
//   instance by priority as well when its name differs.
#include "engine_internal.h"
#include "control/conf.h"

dt_iop_module_t *om_film_module(OmEngine *engine) {
    dt_iop_module_t *film = engine->film_module;
    // Deleting the base instance promotes another one to priority 0 (module_instances.c): a
    // film instance that became the base is the look's instance from then on.
    if (film && (film->multi_priority == 0 || !g_list_find(engine->dev.iop, film)))
        film = engine->film_module = NULL;
    return film;
}

dt_iop_module_t *om_film_create(dt_develop_t *dev) {
    dt_iop_module_t *base = dt_iop_get_module_by_op_priority(dev->iop, "lut3d", 0);
    if (!base)
        return NULL;
    dt_iop_module_t *film = dt_dev_module_duplicate_ext(dev, base, FALSE);
    if (!film)
        return NULL;
    dt_ioppr_resync_modules_order(dev);
    if (film->iop_order > base->iop_order)
        dt_ioppr_move_iop_before(dev, film, base);
    dt_iop_reload_defaults(film);
    return film;
}

const char *om_film_unavailable(const dt_iop_module_t *base, const void *params, int params_size) {
    if (!base || !base->get_p)
        return "this darktable has no LUT 3D module";
    if (!params || params_size != base->params_size)
        return "wrong module version";
    // lut3d.c commit_params → _calculate_clut: a compressed table sits in the parameters, any
    // other is read from the file below the LUT root folder. A missing file leaves the module
    // without effect, which would show a film as applied that does nothing.
    const int *keypoints = base->get_p(params, "nb_keypoints");
    const char *file = base->get_p(params, "filepath");
    if (!file || !*file)
        return "the film names no lookup table";
    if (keypoints && *keypoints > 0)
        return NULL;
    gchar *root = dt_conf_get_string("plugins/darkroom/lut3d/def_path");
    gchar *path = g_build_filename(root, file, NULL);
    const gboolean found = root[0] && g_file_test(path, G_FILE_TEST_IS_REGULAR);
    g_free(path);
    g_free(root);
    return found ? NULL : "its lookup table file is missing";
}

void om_film_write(dt_iop_module_t *film, const void *params, const dt_develop_blend_params_t *blend,
                   const char *label) {
    memcpy(film->params, params, film->params_size);
    dt_iop_commit_blend_params(film, blend ? blend : film->default_blendop_params);
    film->enabled = TRUE;
    g_strlcpy(film->multi_name, label, sizeof(film->multi_name));
    film->multi_name_hand_edited = FALSE;
}

GList *om_film_style_items(dt_develop_t *dev, dt_iop_module_t *film, GList *items) {
    if (!film)
        return NULL;
    dt_iop_module_t *base = dt_iop_get_module_by_op_priority(dev->iop, "lut3d", 0);
    for (GList *it = items; it; it = it->next) {
        dt_style_item_t *item = it->data;
        if (base && !strcmp(item->operation, "lut3d") && item->multi_priority == film->multi_priority) {
            item->multi_priority = base->multi_priority;
            item->iop_order = base->iop_order;
        }
    }
    return g_list_append(NULL, film);
}

dt_iop_module_t *om_film_mirror(OmEngine *engine, dt_develop_t *scratch) {
    dt_iop_module_t *film = om_film_module(engine);
    if (!film)
        return NULL;
    // The scratch context reads the image's stored history, which holds the film once an export
    // has written it.
    dt_iop_module_t *copy = dt_iop_get_module_by_op_priority(scratch->iop, "lut3d", film->multi_priority);
    if (!copy && film->enabled)
        copy = om_film_create(scratch);
    if (!copy || copy->params_size != film->params_size)
        return NULL;
    if (copy->enabled == film->enabled && !memcmp(copy->params, film->params, film->params_size) &&
        !memcmp(copy->blend_params, film->blend_params, sizeof(*film->blend_params)))
        return copy;
    om_film_write(copy, film->params, film->blend_params, film->multi_name);
    copy->enabled = film->enabled;
    dt_dev_add_history_item_ext(scratch, copy, FALSE, FALSE);
    return copy;
}
