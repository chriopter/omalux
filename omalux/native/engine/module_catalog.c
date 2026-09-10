// SPDX-License-Identifier: GPL-3.0-or-later
// Every processing module and every parameter darktable describes about itself.
// darktable generates this description from the module sources, and builds its own
// sliders from it; reading it here means a control does not have to be transcribed
// into our registry to be shown.
#include "engine_internal.h"
#include "common/introspection.h"
#include "controls.h"

// True when the curated panel already offers this parameter under its own name. The raw
// module list hides those, so what it shows is exactly the work still to be designed.
static gboolean is_curated(const char *operation, const char *field_name) {
    if (!operation || !field_name)
        return FALSE;
    for (size_t i = 0; i < OM_CONTROL_COUNT; ++i)
        if (!strcmp(om_controls[i].module, operation) && !strcmp(om_controls[i].parameter, field_name))
            return TRUE;
    return FALSE;
}

static const char *type_name(dt_introspection_type_t type) {
    switch (type) {
    case DT_INTROSPECTION_TYPE_FLOAT: return "float";
    case DT_INTROSPECTION_TYPE_DOUBLE: return "double";
    case DT_INTROSPECTION_TYPE_INT: return "int";
    case DT_INTROSPECTION_TYPE_UINT: return "uint";
    case DT_INTROSPECTION_TYPE_CHAR: return "char";
    case DT_INTROSPECTION_TYPE_BOOL: return "bool";
    case DT_INTROSPECTION_TYPE_ENUM: return "enum";
    case DT_INTROSPECTION_TYPE_ARRAY: return "array";
    case DT_INTROSPECTION_TYPE_STRUCT: return "struct";
    case DT_INTROSPECTION_TYPE_UNION: return "union";
    case DT_INTROSPECTION_TYPE_OPAQUE: return "opaque";
    default: return "other";
    }
}

// The value a module currently holds for one field, as a double where that is meaningful.
static gboolean read_value(const dt_introspection_field_t *field, const void *params, double *value) {
    const void *p = (const char *)params + field->header.offset;
    switch (field->header.type) {
    case DT_INTROSPECTION_TYPE_FLOAT: *value = *(const float *)p; return TRUE;
    case DT_INTROSPECTION_TYPE_DOUBLE: *value = *(const double *)p; return TRUE;
    case DT_INTROSPECTION_TYPE_INT: *value = *(const int *)p; return TRUE;
    case DT_INTROSPECTION_TYPE_UINT: *value = *(const unsigned int *)p; return TRUE;
    case DT_INTROSPECTION_TYPE_ENUM: *value = *(const int *)p; return TRUE;
    case DT_INTROSPECTION_TYPE_BOOL: *value = *(const gboolean *)p; return TRUE;
    case DT_INTROSPECTION_TYPE_CHAR: *value = *(const char *)p; return TRUE;
    default: return FALSE;
    }
}

static void describe_field(JsonArray *out, const dt_introspection_field_t *field, const void *params,
                           const char *operation);

static void describe_children(JsonArray *out, const dt_introspection_field_t *field, const void *params,
                              const char *operation) {
    if (field->header.type == DT_INTROSPECTION_TYPE_STRUCT ||
        field->header.type == DT_INTROSPECTION_TYPE_UNION)
        for (dt_introspection_field_t **child = field->Struct.fields; child && *child; ++child)
            describe_field(out, *child, params, operation);
}

static void describe_field(JsonArray *out, const dt_introspection_field_t *field, const void *params,
                           const char *operation) {
    if (field->header.type == DT_INTROSPECTION_TYPE_STRUCT ||
        field->header.type == DT_INTROSPECTION_TYPE_UNION) {
        describe_children(out, field, params, operation);
        return;
    }
    JsonObject *row = json_object_new();
    json_object_set_string_member(row, "name", field->header.name ? field->header.name : "");
    json_object_set_string_member(row, "field", field->header.field_name ? field->header.field_name : "");
    json_object_set_string_member(row, "label", field->header.description ? field->header.description : "");
    json_object_set_string_member(row, "type", type_name(field->header.type));
    json_object_set_int_member(row, "size", (int)field->header.size);
    json_object_set_boolean_member(row, "curated", is_curated(operation, field->header.field_name));
    double value = 0;
    if (read_value(field, params, &value))
        json_object_set_double_member(row, "value", value);
    switch (field->header.type) {
    case DT_INTROSPECTION_TYPE_FLOAT:
        json_object_set_double_member(row, "minimum", field->Float.Min);
        json_object_set_double_member(row, "maximum", field->Float.Max);
        json_object_set_double_member(row, "default", field->Float.Default);
        break;
    case DT_INTROSPECTION_TYPE_INT:
        json_object_set_double_member(row, "minimum", field->Int.Min);
        json_object_set_double_member(row, "maximum", field->Int.Max);
        json_object_set_double_member(row, "default", field->Int.Default);
        break;
    case DT_INTROSPECTION_TYPE_BOOL:
        json_object_set_double_member(row, "default", field->Bool.Default);
        break;
    case DT_INTROSPECTION_TYPE_ENUM: {
        json_object_set_double_member(row, "default", field->Enum.Default);
        JsonArray *values = json_array_new();
        for (size_t i = 0; i < field->Enum.entries; ++i) {
            JsonObject *entry = json_object_new();
            json_object_set_int_member(entry, "value", field->Enum.values[i].value);
            json_object_set_string_member(entry, "name", field->Enum.values[i].name);
            json_object_set_string_member(entry, "label", field->Enum.values[i].description
                                                              ? field->Enum.values[i].description
                                                              : field->Enum.values[i].name);
            json_array_add_object_element(values, entry);
        }
        json_object_set_array_member(row, "values", values);
        break;
    }
    case DT_INTROSPECTION_TYPE_ARRAY:
        json_object_set_int_member(row, "count", (int)field->Array.count);
        json_object_set_string_member(row, "element", type_name(field->Array.type));
        break;
    default:
        break;
    }
    json_array_add_object_element(out, row);
}

char *om_engine_modules(OmEngine *engine) {
    if (!engine->loaded)
        return NULL;
    JsonArray *modules = json_array_new();
    int position = 0;
    for (GList *it = engine->dev.iop; it; it = it->next) {
        dt_iop_module_t *module = it->data;
        JsonObject *entry = json_object_new();
        json_object_set_string_member(entry, "operation", module->op);
        json_object_set_string_member(entry, "label", module->name());
        json_object_set_int_member(entry, "instance", module->multi_priority);
        json_object_set_int_member(entry, "position", position++);
        json_object_set_boolean_member(entry, "enabled", module->enabled);
        json_object_set_boolean_member(entry, "default_enabled", module->default_enabled);
        json_object_set_boolean_member(entry, "hidden", dt_iop_is_hidden(module));
        json_object_set_boolean_member(entry, "blending", (module->flags() & IOP_FLAGS_SUPPORTS_BLENDING) != 0);
        const dt_introspection_t *introspection =
            module->get_introspection ? module->get_introspection() : NULL;
        json_object_set_boolean_member(entry, "described", introspection != NULL);
        JsonArray *fields = json_array_new();
        if (introspection && introspection->field) {
            json_object_set_int_member(entry, "params_version", introspection->params_version);
            describe_field(fields, introspection->field, module->params, module->op);
        }
        json_object_set_array_member(entry, "parameters", fields);
        json_array_add_object_element(modules, entry);
    }
    JsonNode *node = json_node_new(JSON_NODE_ARRAY);
    json_node_take_array(node, modules);
    char *result = json_to_string(node, FALSE);
    json_node_free(node);
    return result;
}

// Set one described parameter by module operation and field name, then record it in history.
int om_engine_set_parameter(OmEngine *engine, const char *operation, int instance, const char *field_name,
                            double value) {
    if (!engine->loaded)
        return 1;
    dt_iop_module_t *module = dt_iop_get_module_by_op_priority(engine->dev.iop, operation, instance);
    if (!module || !module->get_introspection)
        return 2;
    const dt_introspection_field_t *field = module->get_f(field_name);
    if (!field)
        return 3;
    void *p = (char *)module->params + field->header.offset;
    switch (field->header.type) {
    case DT_INTROSPECTION_TYPE_FLOAT:
        *(float *)p = (float)CLAMP(value, field->Float.Min, field->Float.Max);
        break;
    case DT_INTROSPECTION_TYPE_INT:
        *(int *)p = (int)CLAMP(lround(value), field->Int.Min, field->Int.Max);
        break;
    case DT_INTROSPECTION_TYPE_ENUM:
        *(int *)p = (int)lround(value);
        break;
    case DT_INTROSPECTION_TYPE_BOOL:
        *(gboolean *)p = value > 0.5;
        break;
    default:
        return 4;
    }
    dt_dev_add_history_item_ext(&engine->dev, module, FALSE, FALSE);
    engine->dev.full.pipe->changed |= DT_DEV_PIPE_SYNCH;
    return om_engine_bind_controls(engine);
}
