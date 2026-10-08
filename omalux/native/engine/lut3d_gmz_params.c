// SPDX-License-Identifier: GPL-3.0-or-later
// LUT 3D with G'MIC compressed LUTs (.gmz): what darktable's _filepath_callback and
// _lutname_callback do through _get_compressed_clut (src/iop/lut3d.c:1355-1390, 1459-1505),
// without the GTK list. A .gmz file (relative to plugins/darkroom/lut3d/def_path, like any LUT)
// holds several LUTs: the one named lutname is read, or the first one when that name is not in
// the file (whose name is then stored); its keypoints go to c_clut / nb_keypoints, which the
// lut3d plugin decompresses when the pipe commits the parameters.
#include "engine_internal.h"
#include "lut3d_gmz.h"
#include <json-glib/json-glib.h>

static gboolean is_gmz(const char *path) {
    return path && (g_str_has_suffix(path, ".gmz") || g_str_has_suffix(path, ".GMZ"));
}

static char *full_path(dt_iop_module_t *module) {
    const char *filepath = module->get_p(module->params, "filepath");
    gchar *root = dt_conf_get_string("plugins/darkroom/lut3d/def_path");
    char *full = filepath && filepath[0] && root && root[0] ? g_build_filename(root, filepath, NULL) : NULL;
    g_free(root);
    return full;
}

void om_lut3d_gmz_changed(struct dt_iop_module_t *module, const void *before) {
    if (!dt_iop_module_is(module, "lut3d") || !module->get_f || !module->get_f("c_clut") || !module->get_f("lutname"))
        return;
    const dt_introspection_field_t *fp = module->get_f("filepath"), *fl = module->get_f("lutname");
    char *filepath = module->get_p(module->params, "filepath");
    char *lutname = module->get_p(module->params, "lutname");
    const gboolean file_changed = strcmp((const char *)before + fp->header.offset, filepath) != 0;
    const gboolean name_changed = strcmp((const char *)before + fl->header.offset, lutname) != 0;
    if (!is_gmz(filepath) || (!file_changed && !name_changed))
        return;
    char *full = full_path(module);
    if (!full)
        return;
    // lut3d.c gui_init (1194-1199) creates the cache folder the decompressed LUTs are kept in.
    gchar *cache = g_build_filename(g_get_user_cache_dir(), "gmic", NULL);
    g_mkdir_with_parents(cache, 0700);
    g_free(cache);
    char *found = NULL;
    int *nb = module->get_p(module->params, "nb_keypoints");
    if (!om_lut3d_gmz_read(full, lutname, nb, module->get_p(module->params, "c_clut"), &found, NULL, NULL) && found) {
        // _get_compressed_clut: a name the file does not hold becomes the first LUT's name
        memset(lutname, 0, fl->header.size);
        g_strlcpy(lutname, found, fl->header.size);
    }
    g_free(found);
    g_free(full);
}

char *om_lut3d_gmz_names(struct dt_iop_module_t *module) {
    if (!dt_iop_module_is(module, "lut3d") || !module->get_f || !module->get_f("filepath"))
        return NULL;
    if (!is_gmz(module->get_p(module->params, "filepath")))
        return NULL;
    char *full = full_path(module);
    char **names = NULL;
    int count = 0;
    if (!full || om_lut3d_gmz_read(full, "", NULL, NULL, NULL, &names, &count)) {
        g_free(full);
        return NULL;
    }
    g_free(full);
    JsonBuilder *b = json_builder_new();
    json_builder_begin_array(b);
    for (int i = 0; i < count; i++)
        json_builder_add_string_value(b, names[i]);
    json_builder_end_array(b);
    JsonNode *root = json_builder_get_root(b);
    char *text = json_to_string(root, FALSE);
    json_node_unref(root);
    g_object_unref(b);
    g_strfreev(names);
    return text;
}
