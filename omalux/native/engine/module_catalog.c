// SPDX-License-Identifier: GPL-3.0-or-later
// Every processing module and every parameter darktable describes about itself.
// darktable generates this description from the module sources, and builds its own
// sliders from it; reading it here means a control does not have to be transcribed
// into our registry to be shown.
//
// Introspection offsets are absolute within the params struct and computed with every
// enclosing array index at 0. Relative offsets (child minus parent) and the element size
// of an array therefore locate any element, which is how darktable's own helpers in
// common/introspection.h walk the tree.
#include "engine_internal.h"
#include "common/introspection.h"
#include "controls.h"
#include "blending.h"
#include "canvas.h"
#include <math.h>

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
    case DT_INTROSPECTION_TYPE_FLOAT:
        return "float";
    case DT_INTROSPECTION_TYPE_DOUBLE:
        return "double";
    case DT_INTROSPECTION_TYPE_FLOATCOMPLEX:
        return "complex";
    case DT_INTROSPECTION_TYPE_INT8:
        return "int8";
    case DT_INTROSPECTION_TYPE_UINT8:
        return "uint8";
    case DT_INTROSPECTION_TYPE_SHORT:
        return "short";
    case DT_INTROSPECTION_TYPE_USHORT:
        return "ushort";
    case DT_INTROSPECTION_TYPE_INT:
        return "int";
    case DT_INTROSPECTION_TYPE_UINT:
        return "uint";
    case DT_INTROSPECTION_TYPE_LONG:
        return "long";
    case DT_INTROSPECTION_TYPE_ULONG:
        return "ulong";
    case DT_INTROSPECTION_TYPE_CHAR:
        return "char";
    case DT_INTROSPECTION_TYPE_BOOL:
        return "bool";
    case DT_INTROSPECTION_TYPE_ENUM:
        return "enum";
    case DT_INTROSPECTION_TYPE_ARRAY:
        return "array";
    case DT_INTROSPECTION_TYPE_STRUCT:
        return "struct";
    case DT_INTROSPECTION_TYPE_UNION:
        return "union";
    case DT_INTROSPECTION_TYPE_OPAQUE:
        return "opaque";
    default:
        return "other";
    }
}

static gboolean is_compound(const dt_introspection_field_t *field) {
    return field->header.type == DT_INTROSPECTION_TYPE_STRUCT ||
           field->header.type == DT_INTROSPECTION_TYPE_UNION;
}

static dt_introspection_field_t **children_of(const dt_introspection_field_t *field) {
    return field->header.type == DT_INTROSPECTION_TYPE_STRUCT ? field->Struct.fields : field->Union.fields;
}

// The value stored at p for one scalar field, as a double where that is meaningful.
static gboolean read_scalar(const dt_introspection_field_t *field, const void *p, double *value) {
    switch (field->header.type) {
    case DT_INTROSPECTION_TYPE_FLOAT:
        *value = *(const float *)p;
        return TRUE;
    case DT_INTROSPECTION_TYPE_DOUBLE:
        *value = *(const double *)p;
        return TRUE;
    case DT_INTROSPECTION_TYPE_INT:
        *value = *(const int *)p;
        return TRUE;
    case DT_INTROSPECTION_TYPE_UINT:
        *value = *(const unsigned int *)p;
        return TRUE;
    case DT_INTROSPECTION_TYPE_ENUM:
        *value = *(const int *)p;
        return TRUE;
    case DT_INTROSPECTION_TYPE_BOOL:
        *value = *(const gboolean *)p;
        return TRUE;
    case DT_INTROSPECTION_TYPE_CHAR:
        *value = *(const char *)p;
        return TRUE;
    case DT_INTROSPECTION_TYPE_INT8:
        *value = *(const int8_t *)p;
        return TRUE;
    case DT_INTROSPECTION_TYPE_UINT8:
        *value = *(const uint8_t *)p;
        return TRUE;
    case DT_INTROSPECTION_TYPE_SHORT:
        *value = *(const short *)p;
        return TRUE;
    case DT_INTROSPECTION_TYPE_USHORT:
        *value = *(const unsigned short *)p;
        return TRUE;
    case DT_INTROSPECTION_TYPE_LONG:
        *value = *(const long *)p;
        return TRUE;
    case DT_INTROSPECTION_TYPE_ULONG:
        *value = *(const unsigned long *)p;
        return TRUE;
    default:
        return FALSE;
    }
}

static JsonNode *number_node(double value) {
    JsonNode *node = json_node_new(JSON_NODE_VALUE);
    if (isfinite(value))
        json_node_set_double(node, value);
    else
        json_node_init_null(node);
    return node;
}

// The current value of any field as JSON: numbers for scalars, nested arrays for arrays,
// objects keyed by member name for structs and unions, a string for char arrays.
static JsonNode *value_node(const dt_introspection_field_t *field, const char *p) {
    double scalar = 0;
    if (read_scalar(field, p, &scalar))
        return number_node(scalar);
    if (field->header.type == DT_INTROSPECTION_TYPE_FLOATCOMPLEX) {
        JsonArray *pair = json_array_new();
        json_array_add_element(pair, number_node(((const float *)p)[0]));
        json_array_add_element(pair, number_node(((const float *)p)[1]));
        JsonNode *node = json_node_new(JSON_NODE_ARRAY);
        json_node_take_array(node, pair);
        return node;
    }
    if (field->header.type == DT_INTROSPECTION_TYPE_ARRAY) {
        const dt_introspection_field_t *element = field->Array.field;
        JsonNode *node;
        if (element->header.type == DT_INTROSPECTION_TYPE_CHAR) {
            char *text = g_utf8_make_valid(p, strnlen(p, field->Array.count));
            node = json_node_new(JSON_NODE_VALUE);
            json_node_set_string(node, text);
            g_free(text);
            return node;
        }
        JsonArray *items = json_array_new();
        for (size_t i = 0; i < field->Array.count; ++i)
            json_array_add_element(items, value_node(element, p + i * element->header.size));
        node = json_node_new(JSON_NODE_ARRAY);
        json_node_take_array(node, items);
        return node;
    }
    if (is_compound(field)) {
        JsonObject *members = json_object_new();
        for (dt_introspection_field_t **child = children_of(field); child && *child; ++child)
            json_object_set_member(members, (*child)->header.field_name,
                                   value_node(*child, p + ((*child)->header.offset - field->header.offset)));
        JsonNode *node = json_node_new(JSON_NODE_OBJECT);
        json_node_take_object(node, members);
        return node;
    }
    JsonNode *node = json_node_new(JSON_NODE_NULL);
    return node;
}

// Range, default and choices of a scalar, as darktable's introspection states them.
static void describe_limits(JsonObject *row, const dt_introspection_field_t *field) {
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
    case DT_INTROSPECTION_TYPE_UINT:
        json_object_set_double_member(row, "minimum", field->UInt.Min);
        json_object_set_double_member(row, "maximum", field->UInt.Max);
        json_object_set_double_member(row, "default", field->UInt.Default);
        break;
    case DT_INTROSPECTION_TYPE_CHAR:
        json_object_set_double_member(row, "minimum", field->Char.Min);
        json_object_set_double_member(row, "maximum", field->Char.Max);
        json_object_set_double_member(row, "default", field->Char.Default);
        break;
    case DT_INTROSPECTION_TYPE_SHORT:
        json_object_set_double_member(row, "minimum", field->Short.Min);
        json_object_set_double_member(row, "maximum", field->Short.Max);
        json_object_set_double_member(row, "default", field->Short.Default);
        break;
    case DT_INTROSPECTION_TYPE_USHORT:
        json_object_set_double_member(row, "minimum", field->UShort.Min);
        json_object_set_double_member(row, "maximum", field->UShort.Max);
        json_object_set_double_member(row, "default", field->UShort.Default);
        break;
    case DT_INTROSPECTION_TYPE_DOUBLE:
        json_object_set_double_member(row, "minimum", field->Double.Min);
        json_object_set_double_member(row, "maximum", field->Double.Max);
        json_object_set_double_member(row, "default", field->Double.Default);
        break;
    case DT_INTROSPECTION_TYPE_BOOL:
        json_object_set_double_member(row, "default", field->Bool.Default);
        break;
    case DT_INTROSPECTION_TYPE_ENUM: {
        json_object_set_double_member(row, "default", field->Enum.Default);
        JsonArray *values = json_array_new();
        for (size_t i = 0; field->Enum.values && i < field->Enum.entries && field->Enum.values[i].name; ++i) {
            JsonObject *entry = json_object_new();
            json_object_set_int_member(entry, "value", field->Enum.values[i].value);
            json_object_set_string_member(entry, "name", field->Enum.values[i].name);
            json_object_set_string_member(entry, "label",
                                          field->Enum.values[i].description
                                              ? field->Enum.values[i].description
                                              : field->Enum.values[i].name);
            json_array_add_object_element(values, entry);
        }
        json_object_set_array_member(row, "values", values);
        break;
    }
    default:
        break;
    }
}

// What one element of an array is: its scalar limits, or the members of its struct.
static JsonObject *describe_item(const dt_introspection_field_t *element) {
    JsonObject *item = json_object_new();
    json_object_set_string_member(item, "type", type_name(element->header.type));
    if (element->header.description)
        json_object_set_string_member(item, "label", element->header.description);
    describe_limits(item, element);
    if (is_compound(element)) {
        JsonArray *members = json_array_new();
        for (dt_introspection_field_t **child = children_of(element); child && *child; ++child) {
            JsonObject *member = describe_item(*child);
            json_object_set_string_member(member, "field", (*child)->header.field_name);
            json_array_add_object_element(members, member);
        }
        json_object_set_array_member(item, "members", members);
    }
    return item;
}

static void describe_field(JsonArray *out, const dt_introspection_field_t *field, const char *p,
                           const char *path, const char *operation) {
    if (is_compound(field)) {
        for (dt_introspection_field_t **child = children_of(field); child && *child; ++child) {
            char *child_path = *path ? g_strdup_printf("%s.%s", path, (*child)->header.field_name)
                                     : g_strdup((*child)->header.field_name);
            describe_field(out, *child, p + ((*child)->header.offset - field->header.offset), child_path,
                           operation);
            g_free(child_path);
        }
        return;
    }
    JsonObject *row = json_object_new();
    json_object_set_string_member(row, "name", field->header.name ? field->header.name : "");
    json_object_set_string_member(row, "field", field->header.field_name ? field->header.field_name : "");
    json_object_set_string_member(row, "path", path);
    json_object_set_string_member(row, "label", field->header.description ? field->header.description : "");
    json_object_set_string_member(row, "type", type_name(field->header.type));
    json_object_set_int_member(row, "size", (int)field->header.size);
    json_object_set_boolean_member(row, "curated", is_curated(operation, field->header.field_name));
    if (field->header.type != DT_INTROSPECTION_TYPE_OPAQUE)
        json_object_set_member(row, "value", value_node(field, p));
    describe_limits(row, field);
    if (field->header.type == DT_INTROSPECTION_TYPE_ARRAY) {
        json_object_set_int_member(row, "count", (int)field->Array.count);
        json_object_set_string_member(row, "element", type_name(field->Array.type));
        JsonArray *dimensions = json_array_new();
        const dt_introspection_field_t *element = field;
        for (; element->header.type == DT_INTROSPECTION_TYPE_ARRAY; element = element->Array.field)
            json_array_add_int_element(dimensions, (gint64)element->Array.count);
        json_object_set_array_member(row, "dimensions", dimensions);
        json_object_set_object_member(row, "item", describe_item(element));
    }
    json_array_add_object_element(out, row);
}

static JsonObject *describe_module(dt_iop_module_t *module, int position) {
    JsonObject *entry = json_object_new();
    json_object_set_string_member(entry, "operation", module->op);
    json_object_set_string_member(entry, "label", module->name());
    json_object_set_int_member(entry, "instance", module->multi_priority);
    json_object_set_int_member(entry, "position", position);
    json_object_set_boolean_member(entry, "enabled", module->enabled);
    json_object_set_boolean_member(entry, "default_enabled", module->default_enabled);
    json_object_set_boolean_member(entry, "hidden", dt_iop_is_hidden(module));
    json_object_set_boolean_member(entry, "blending", (module->flags() & IOP_FLAGS_SUPPORTS_BLENDING) != 0);
    const dt_introspection_t *introspection = module->get_introspection ? module->get_introspection() : NULL;
    json_object_set_boolean_member(entry, "described", introspection != NULL);
    JsonArray *fields = json_array_new();
    if (introspection && introspection->field) {
        json_object_set_int_member(entry, "params_version", introspection->params_version);
        describe_field(fields, introspection->field, (const char *)module->params, "", module->op);
    }
    json_object_set_array_member(entry, "parameters", fields);
    // Displayed conversions ("derived") and runtime-list texts ("labels"), module_values.c.
    if (introspection)
        om_module_describe_values(module, entry);
    // Multi-instance state and the blend section (module_instances.c, blending.c).
    om_instance_describe(entry, module);
    om_blend_describe(entry, module);
    return entry;
}

static char *take_json(JsonNode *node) {
    char *result = json_to_string(node, FALSE);
    json_node_free(node);
    return result;
}

char *om_engine_modules(OmEngine *engine) {
    if (!engine->loaded)
        return NULL;
    JsonArray *modules = json_array_new();
    int position = 0;
    for (GList *it = engine->dev.iop; it; it = it->next)
        json_array_add_object_element(modules, describe_module(it->data, position++));
    JsonNode *node = json_node_new(JSON_NODE_ARRAY);
    json_node_take_array(node, modules);
    return take_json(node);
}

int om_engine_module_count(OmEngine *engine) {
    return engine->loaded ? (int)g_list_length(engine->dev.iop) : 0;
}

const char *om_engine_module_identity(OmEngine *engine, int position, int *instance) {
    dt_iop_module_t *module =
        engine->loaded && position >= 0 ? g_list_nth_data(engine->dev.iop, position) : NULL;
    if (!module)
        return NULL;
    *instance = module->multi_priority;
    return module->op;
}

char *om_engine_module_at(OmEngine *engine, int position) {
    dt_iop_module_t *module =
        engine->loaded && position >= 0 ? g_list_nth_data(engine->dev.iop, position) : NULL;
    if (!module)
        return NULL;
    JsonNode *node = json_node_new(JSON_NODE_OBJECT);
    json_node_take_object(node, describe_module(module, position));
    return take_json(node);
}

// One scalar inside the params blob, located by a path such as "tonecurve[0][1].x".
typedef struct {
    const dt_introspection_field_t *field;
    char *pointer;
} OmTarget;

static int resolve_path(dt_iop_module_t *module, const char *path, OmTarget *target) {
    const dt_introspection_t *introspection = module->get_introspection ? module->get_introspection() : NULL;
    if (!introspection || !introspection->field || !path || !*path)
        return 3;
    const dt_introspection_field_t *field = introspection->field;
    char *p = module->params;
    const char *c = path;
    for (;;) {
        if (!is_compound(field))
            return 3;
        const char *start = c;
        while (g_ascii_isalnum(*c) || *c == '_')
            ++c;
        if (c == start)
            return 3;
        const dt_introspection_field_t *found = NULL;
        for (dt_introspection_field_t **child = children_of(field); child && *child; ++child) {
            const char *name = (*child)->header.field_name;
            if (name && strlen(name) == (size_t)(c - start) && !strncmp(name, start, c - start)) {
                found = *child;
                break;
            }
        }
        if (!found)
            return 3;
        p += found->header.offset - field->header.offset;
        field = found;
        while (*c == '[') {
            char *end = NULL;
            ++c;
            if (!g_ascii_isdigit(*c))
                return 3;
            const guint64 index = g_ascii_strtoull(c, &end, 10);
            if (!end || *end != ']' || field->header.type != DT_INTROSPECTION_TYPE_ARRAY ||
                index >= field->Array.count)
                return 3;
            field = field->Array.field;
            p += index * field->header.size;
            c = end + 1;
        }
        if (*c == '\0')
            break;
        if (*c != '.')
            return 3;
        ++c;
    }
    target->field = field;
    target->pointer = p;
    return 0;
}

static gboolean writable(const dt_introspection_field_t *field) {
    switch (field->header.type) {
    case DT_INTROSPECTION_TYPE_FLOAT:
    case DT_INTROSPECTION_TYPE_DOUBLE:
    case DT_INTROSPECTION_TYPE_INT:
    case DT_INTROSPECTION_TYPE_UINT:
    case DT_INTROSPECTION_TYPE_CHAR:
    case DT_INTROSPECTION_TYPE_SHORT:
    case DT_INTROSPECTION_TYPE_USHORT:
    case DT_INTROSPECTION_TYPE_ENUM:
    case DT_INTROSPECTION_TYPE_BOOL:
        return TRUE;
    default:
        return FALSE;
    }
}

// Store a value with the limits darktable's introspection declares for the field.
static void write_value(const OmTarget *target, double value) {
    const dt_introspection_field_t *f = target->field;
    void *p = target->pointer;
    switch (f->header.type) {
    case DT_INTROSPECTION_TYPE_FLOAT:
        *(float *)p = (float)CLAMP(value, f->Float.Min, f->Float.Max);
        break;
    case DT_INTROSPECTION_TYPE_DOUBLE:
        *(double *)p = CLAMP(value, f->Double.Min, f->Double.Max);
        break;
    case DT_INTROSPECTION_TYPE_INT:
        *(int *)p = (int)CLAMP(lround(value), f->Int.Min, f->Int.Max);
        break;
    case DT_INTROSPECTION_TYPE_UINT:
        *(unsigned int *)p =
            (unsigned int)CLAMP(llround(value), (long long)f->UInt.Min, (long long)f->UInt.Max);
        break;
    case DT_INTROSPECTION_TYPE_CHAR:
        *(char *)p = (char)CLAMP(lround(value), f->Char.Min, f->Char.Max);
        break;
    case DT_INTROSPECTION_TYPE_SHORT:
        *(short *)p = (short)CLAMP(lround(value), f->Short.Min, f->Short.Max);
        break;
    case DT_INTROSPECTION_TYPE_USHORT:
        *(unsigned short *)p = (unsigned short)CLAMP(lround(value), f->UShort.Min, f->UShort.Max);
        break;
    case DT_INTROSPECTION_TYPE_ENUM:
        *(int *)p = (int)lround(value);
        break;
    case DT_INTROSPECTION_TYPE_BOOL:
        *(gboolean *)p = value > 0.5;
        break;
    default:
        break;
    }
}

typedef struct {
    OmTarget target;
    double value;
    const char *text;
} OmAssignment;

// A char array (file name, lens model, ...) takes a string that fits including its terminator.
static gboolean text_target(const dt_introspection_field_t *field) {
    return field->header.type == DT_INTROSPECTION_TYPE_ARRAY &&
           field->Array.type == DT_INTROSPECTION_TYPE_CHAR;
}

// Validate every assignment first, then write them and record one history item, so a
// rejected path never leaves the module half-edited. texts[i] (may be NULL) is a string value.
// Paths starting with "@" other than "@enabled" are darktable's displayed conversions and
// runtime choices (module_values.c); they are converted after the plain values.
static int apply_assignments(OmEngine *engine, dt_iop_module_t *module, const char *const *paths,
                             const double *values, const char *const *texts, size_t count) {
    OmAssignment *assignments = g_new0(OmAssignment, count ? count : 1);
    const char **derived_paths = g_new0(const char *, count ? count : 1);
    const char **derived_texts = g_new0(const char *, count ? count : 1);
    double *derived_values = g_new0(double, count ? count : 1);
    size_t derived = 0;
    int enable = -1, error = 0;
    // "blend.*" paths edit the blend section (blending.c), validated with the rest; they are
    // taken before the "@" conversions of module_values.c.
    OmBlendEdit blend;
    om_blend_begin(&blend, module);
    for (size_t i = 0; i < count && !error; ++i) {
        const char *text = texts ? texts[i] : NULL;
        if (om_blend_path(paths[i])) {
            error = text ? 5 : om_blend_assign(&blend, module, paths[i], values[i]);
            continue;
        }
        // "@enabled" is not a parameter of the module but the module itself being in the pipeline.
        if (!strcmp(paths[i], "@enabled")) {
            if (text || !isfinite(values[i]))
                error = 5;
            enable = values[i] > 0.5;
            continue;
        }
        if (paths[i][0] == '@') {
            derived_paths[derived] = paths[i];
            derived_texts[derived] = text;
            derived_values[derived++] = values[i];
            continue;
        }
        if ((error = resolve_path(module, paths[i], &assignments[i].target)))
            break;
        const dt_introspection_field_t *field = assignments[i].target.field;
        if (text ? !text_target(field) || strlen(text) >= field->header.size : !writable(field))
            error = text ? 5 : 4;
        else if (!text && !isfinite(values[i]))
            error = 5;
        assignments[i].value = values[i];
        assignments[i].text = text;
    }
    void *backup = error ? NULL : g_memdup2(module->params, module->params_size);
    gboolean changed = FALSE;
    for (size_t i = 0; i < count && !error; ++i)
        if (assignments[i].target.field) {
            if (assignments[i].text) {
                memset(assignments[i].target.pointer, 0, assignments[i].target.field->header.size);
                g_strlcpy(assignments[i].target.pointer, assignments[i].text,
                          assignments[i].target.field->header.size);
            } else
                write_value(&assignments[i].target, assignments[i].value);
            changed = TRUE;
        }
    if (!error && derived) {
        error = om_module_set_values(engine, module, derived, derived_paths, derived_values, derived_texts);
        changed = TRUE;
    }
    // A rejected conversion leaves the parameters as they were.
    if (error && backup)
        memcpy(module->params, backup, module->params_size);
    g_free(backup);
    g_free(assignments);
    g_free(derived_paths);
    g_free(derived_texts);
    g_free(derived_values);
    if (error)
        return error;
    if (blend.touched) {
        om_blend_commit(module, &blend);
        changed = TRUE;
    }
    if (enable >= 0 && module->enabled != enable) {
        module->enabled = enable;
        changed = TRUE;
    }
    if (!changed)
        return 0;
    // Like a darktable slider, editing a parameter switches its module on, unless this
    // edit states the enablement itself or the module has no enable button at all.
    const gboolean switch_on = enable < 0 && !module->hide_enable_button;
    // A selected retouch shape follows the module's blur and fill fields (canvas.c).
    om_engine_canvas_parameters_changed(engine, module);
    dt_dev_add_history_item_ext(&engine->dev, module, switch_on, FALSE);
    engine->dev.full.pipe->changed |= DT_DEV_PIPE_SYNCH;
    return om_engine_bind_controls(engine);
}

static int find_module(OmEngine *engine, const char *operation, int instance, dt_iop_module_t **module) {
    if (!engine->loaded)
        return 1;
    *module = dt_iop_get_module_by_op_priority(engine->dev.iop, operation, instance);
    return *module ? 0 : 2;
}

// Set one described parameter by module operation, instance and path, then record it in history.
int om_engine_set_parameter(OmEngine *engine, const char *operation, int instance, const char *path,
                            double value) {
    dt_iop_module_t *module = NULL;
    const int error = find_module(engine, operation, instance, &module);
    return error ? error : apply_assignments(engine, module, &path, &value, NULL, 1);
}

int om_engine_set_parameters(OmEngine *engine, const char *operation, int instance, const char *json) {
    dt_iop_module_t *module = NULL;
    int error = find_module(engine, operation, instance, &module);
    if (error)
        return error;
    JsonParser *parser = json_parser_new();
    if (!json || !json_parser_load_from_data(parser, json, -1, NULL) || !json_parser_get_root(parser) ||
        !JSON_NODE_HOLDS_OBJECT(json_parser_get_root(parser))) {
        g_object_unref(parser);
        return 5;
    }
    JsonObject *object = json_node_get_object(json_parser_get_root(parser));
    GList *members = json_object_get_members(object);
    const guint count = g_list_length(members);
    const char **paths = g_new0(const char *, count ? count : 1);
    const char **texts = g_new0(const char *, count ? count : 1);
    double *values = g_new0(double, count ? count : 1);
    guint i = 0;
    for (GList *it = members; it; it = it->next, ++i) {
        JsonNode *node = json_object_get_member(object, it->data);
        paths[i] = it->data;
        values[i] = NAN;
        if (JSON_NODE_HOLDS_VALUE(node)) {
            const GType type = json_node_get_value_type(node);
            if (type == G_TYPE_DOUBLE || type == G_TYPE_INT64 || type == G_TYPE_BOOLEAN)
                values[i] = type == G_TYPE_BOOLEAN ? json_node_get_boolean(node) : json_node_get_double(node);
            else if (type == G_TYPE_STRING)
                texts[i] = json_node_get_string(node);
        }
    }
    error = apply_assignments(engine, module, paths, values, texts, count);
    g_free(paths);
    g_free(texts);
    g_free(values);
    g_list_free(members);
    g_object_unref(parser);
    return error;
}

// darktable's reset button without its GUI (develop/imageop.c, _gui_reset_callback): reload
// image-dependent defaults, restore default parameters and blending, and record the module
// switched on. Modules that are always off cannot be reset, as in darktable.
int om_engine_reset_module(OmEngine *engine, const char *operation, int instance) {
    dt_iop_module_t *module = NULL;
    const int error = find_module(engine, operation, instance, &module);
    if (error)
        return error;
    if (!module->default_enabled && module->hide_enable_button)
        return 4;
    dt_iop_reload_defaults(module);
    dt_iop_commit_blend_params(module, module->default_blendop_params);
    dt_dev_add_history_item_ext(&engine->dev, module, TRUE, FALSE);
    engine->dev.full.pipe->changed |= DT_DEV_PIPE_SYNCH;
    return om_engine_bind_controls(engine);
}
