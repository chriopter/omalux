// SPDX-License-Identifier: GPL-3.0-or-later
#include "engine_internal.h"
#include "common/exif.h"
// A style snapshot is generated without mutating the current module parameters.
// Complex masks need their own portable serialization and are rejected here.
char *om_snapshot(OmEngine *engine, const char *name, const char *prefix, const char *only_module) {
    if (!engine->loaded || engine->dev.forms)
        return NULL;
    JsonObject *object = json_object_new();
    JsonArray *assets = json_array_new();
    char *safe_name = g_markup_escape_text(name, -1);
    GString *xml = g_string_new(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?><darktable_style version=\"1.0\"><info><name>");
    g_string_append_printf(xml, "%s</name><description>Saved in Omalux</description></info><style>",
                           safe_name);
    g_free(safe_name);
    int number = 0;
    for (GList *it = engine->dev.iop; it; it = it->next) {
        dt_iop_module_t *module = it->data;
        gboolean include = FALSE;
        for (unsigned int i = 0; i < OM_CONTROL_COUNT; ++i)
            if (engine->modules[i] == module)
                include = TRUE;
        for (GList *h = engine->dev.history; h; h = h->next)
            if (((dt_dev_history_item_t *)h->data)->module == module && !module->hide_enable_button)
                include = TRUE;
        if (only_module && strcmp(only_module, "*"))
            include = !strcmp(module->op, only_module);
        if (!include)
            continue;
        if (module->multi_priority != 0 ||
            (module->blend_params->mask_mode & (DEVELOP_MASK_MASK | DEVELOP_MASK_RASTER)))
            goto unsupported;
        // Do not silently export a dependency which this snapshot writer cannot package.
        if (module->enabled && (!strcmp(module->op, "watermark") || !strcmp(module->op, "overlay") ||
                                !strcmp(module->op, "rasterfile")))
            goto unsupported;
        void *copy = g_memdup2(module->params, module->params_size);
        if (!only_module && !strcmp(module->op, "lut3d")) {
            char *path = module->get_p(copy, "filepath");
            if (path && *path) {
                const dt_introspection_field_t *field = module->get_f("filepath");
                const char *extension = strrchr(path, '.');
                if (extension && (strchr(extension, '/') || strchr(extension, '\\'))) {
                    g_free(copy);
                    goto unsupported;
                }
                char *target = g_strdup_printf("assets/lut-%d%s", number, extension ? extension : ".cube");
                char *new_path = g_strdup_printf("%s/%s", prefix, target);
                if (!field || strlen(new_path) >= field->header.size) {
                    g_free(target);
                    g_free(new_path);
                    g_free(copy);
                    goto unsupported;
                }
                JsonObject *asset = json_object_new();
                json_object_set_string_member(asset, "source", path);
                json_object_set_string_member(asset, "path", target);
                json_object_set_string_member(asset, "role", "lut");
                json_array_add_object_element(assets, asset);
                g_strlcpy(path, new_path, field->header.size);
                g_free(target);
                g_free(new_path);
            }
        }
        char *params = dt_exif_xmp_encode_internal(copy, module->params_size, NULL, FALSE);
        char *blend = dt_exif_xmp_encode_internal((unsigned char *)module->blend_params,
                                                  sizeof(*module->blend_params), NULL, FALSE);
        char *multi = g_markup_escape_text(module->multi_name, -1);
        g_string_append_printf(
            xml,
            "<plugin><num>%d</num><module>%d</module><operation>%s</operation><op_params>%s</"
            "op_params><enabled>%d</enabled><blendop_params>%s</blendop_params><blendop_version>%d</"
            "blendop_version><multi_priority>0</multi_priority><multi_name>%s</"
            "multi_name><multi_name_hand_edited>0</multi_name_hand_edited></plugin>",
            number++, module->version(), module->op, params, module->enabled, blend,
            dt_develop_blend_version(), multi);
        g_free(copy);
        g_free(params);
        g_free(blend);
        g_free(multi);
    }
    g_string_append(xml, "</style></darktable_style>");
    json_object_set_string_member(object, "xml", xml->str);
    g_string_free(xml, TRUE);
    json_object_set_array_member(object, "assets", assets);
    JsonNode *node = json_node_new(JSON_NODE_OBJECT);
    json_node_take_object(node, object);
    char *result = json_to_string(node, FALSE);
    json_node_free(node);
    return result;
unsupported:
    g_string_free(xml, TRUE);
    json_array_unref(assets);
    json_object_unref(object);
    return NULL;
}

char *om_engine_snapshot(OmEngine *engine, const char *name, const char *prefix) {
    return om_snapshot(engine, name, prefix, NULL);
}
char *om_engine_module_snapshot(OmEngine *engine, const char *name, const char *module) {
    return om_snapshot(engine, name, "", module);
}
