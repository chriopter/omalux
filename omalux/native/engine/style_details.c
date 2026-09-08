// SPDX-License-Identifier: GPL-3.0-or-later
#include "engine_internal.h"
#include "style_details.h"
#include <json-glib/json-glib.h>
#include "common/introspection.h"

static JsonNode *style_text(const char *value) {
    JsonNode *node = json_node_new(JSON_NODE_VALUE);
    json_node_set_string(node, value);
    return node;
}
static JsonNode *style_field_value(const dt_introspection_field_t *field, const unsigned char *bytes,
                                   size_t size, size_t extra, int depth) {
    const size_t offset = field->header.offset + extra, count = field->header.size;
    if (depth > 12 || offset > size || count > size - offset)
        return style_text("Invalid parameter layout");
    const unsigned char *p = bytes + offset;
    JsonNode *node = json_node_new(JSON_NODE_VALUE);
#define STYLE_NUMBER(kind, ctype)                                                                            \
    case DT_INTROSPECTION_TYPE_##kind: {                                                                     \
        ctype v;                                                                                             \
        if (count != sizeof(v))                                                                              \
            break;                                                                                           \
        memcpy(&v, p, sizeof(v));                                                                            \
        json_node_set_double(node, (double)v);                                                               \
        return node;                                                                                         \
    }
    switch (field->header.type) {
        STYLE_NUMBER(FLOAT, float)
        STYLE_NUMBER(DOUBLE, double)
        STYLE_NUMBER(CHAR, char)
        STYLE_NUMBER(INT8, int8_t)
        STYLE_NUMBER(UINT8, uint8_t)
        STYLE_NUMBER(SHORT, short)
        STYLE_NUMBER(USHORT, unsigned short)
        STYLE_NUMBER(INT, int)
        STYLE_NUMBER(UINT, unsigned int)
        STYLE_NUMBER(LONG, long)
        STYLE_NUMBER(ULONG, unsigned long)
    case DT_INTROSPECTION_TYPE_BOOL: {
        gboolean v = FALSE;
        if (count == sizeof(v))
            memcpy(&v, p, count);
        else if (count == 1)
            v = *p;
        else
            break;
        json_node_set_boolean(node, v);
        return node;
    }
    case DT_INTROSPECTION_TYPE_ENUM: {
        int v;
        if (count != sizeof(v))
            break;
        memcpy(&v, p, count);
        for (dt_introspection_type_enum_tuple_t *it = field->Enum.values; it->name; ++it)
            if (it->value == v) {
                json_node_set_string(node, it->description && *it->description ? it->description : it->name);
                return node;
            }
        json_node_set_int(node, v);
        return node;
    }
    case DT_INTROSPECTION_TYPE_ARRAY: {
        if (field->Array.field->header.type == DT_INTROSPECTION_TYPE_CHAR) {
            gchar *s = g_strndup((const char *)p, count);
            if (g_utf8_validate(s, -1, NULL)) {
                json_node_set_string(node, s);
                g_free(s);
                return node;
            }
            g_free(s);
            break;
        }
        JsonArray *array = json_array_new();
        for (size_t i = 0; i < field->Array.count; ++i)
            json_array_add_element(array,
                                   style_field_value(field->Array.field, bytes, size,
                                                     extra + i * field->Array.field->header.size, depth + 1));
        json_node_init_array(node, array);
        json_array_unref(array);
        return node;
    }
    case DT_INTROSPECTION_TYPE_STRUCT: {
        JsonObject *object = json_object_new();
        for (dt_introspection_field_t **it = field->Struct.fields; *it; ++it)
            json_object_set_member(object, (*it)->header.field_name,
                                   style_field_value(*it, bytes, size, extra, depth + 1));
        json_node_init_object(node, object);
        json_object_unref(object);
        return node;
    }
    default:
        break;
    }
#undef STYLE_NUMBER
    // Preserve untyped/union data in the inspector without guessing its meaning.
    GString *hex = g_string_new("hex: ");
    for (size_t i = 0; i < count; ++i)
        g_string_append_printf(hex, "%02x", p[i]);
    json_node_set_string(node, hex->str);
    g_string_free(hex, TRUE);
    return node;
}

gchar *describe_style(dt_develop_t *context, const char *path, const char *name) {
    JsonObject *root = json_object_new();
    JsonArray *modules = json_array_new();
    json_object_set_array_member(root, "modules", modules);
    json_object_set_string_member(root, "error", "");
    if (dt_styles_exists(name))
        dt_styles_delete_by_name_adv(name, FALSE, FALSE);
    dt_styles_import_from_file(path);
    GList *items = dt_styles_get_item_list(name, FALSE, -1, TRUE);
    if (!items)
        json_object_set_string_member(root, "error", "Style contains no usable modules");
    for (GList *it = items; it; it = it->next) {
        dt_style_item_t *item = it->data;
        dt_iop_module_t *module = dt_iop_get_module_by_op_priority(context->iop, item->operation, -1);
        JsonObject *entry = json_object_new();
        JsonArray *settings = json_array_new();
        json_array_add_object_element(modules, entry);
        json_object_set_string_member(entry, "name", module ? module->name() : item->operation);
        json_object_set_string_member(entry, "operation", item->operation);
        json_object_set_string_member(entry, "instance", item->multi_name ? item->multi_name : "");
        json_object_set_boolean_member(entry, "enabled", item->enabled);
        json_object_set_array_member(entry, "settings", settings);
        if (!module || module->version() != item->module_version ||
            module->params_size != item->params_size) {
            gchar *error = g_strdup_printf("Unavailable or incompatible module: %s", item->operation);
            json_object_set_string_member(root, "error", error);
            g_free(error);
            continue;
        }
        const dt_introspection_t *info = module->get_introspection();
        if (info && info->field && info->field->header.type == DT_INTROSPECTION_TYPE_STRUCT) {
            for (dt_introspection_field_t **f = info->field->Struct.fields; *f; ++f) {
                JsonObject *setting = json_object_new();
                json_array_add_object_element(settings, setting);
                json_object_set_string_member(setting, "parameter", (*f)->header.field_name);
                gchar *label =
                    g_strdup((*f)->header.description && *(*f)->header.description ? (*f)->header.description
                                                                                   : (*f)->header.field_name);
                for (char *c = label; *c; ++c)
                    if (*c == '_')
                        *c = ' ';
                json_object_set_string_member(setting, "label", label);
                g_free(label);
                json_object_set_member(setting, "value",
                                       style_field_value(*f, item->params, item->params_size, 0, 0));
            }
        } else {
            JsonObject *setting = json_object_new();
            json_array_add_object_element(settings, setting);
            json_object_set_string_member(setting, "label", "stored settings");
            json_object_set_string_member(setting, "parameter", "");
            GString *hex = g_string_new("hex: ");
            for (int i = 0; i < item->params_size; ++i)
                g_string_append_printf(hex, "%02x", ((unsigned char *)item->params)[i]);
            json_object_set_string_member(setting, "value", hex->str);
            g_string_free(hex, TRUE);
        }
        json_object_set_string_member(entry, "blending",
                                      item->blendop_params_size ? "Includes stored blending settings"
                                                                : "Default blending; no masks");
    }
    g_list_free_full(items, dt_style_item_free);
    JsonNode *node = json_node_new(JSON_NODE_OBJECT);
    json_node_take_object(node, root);
    gchar *json = json_to_string(node, FALSE);
    json_node_free(node);
    return json;
}
