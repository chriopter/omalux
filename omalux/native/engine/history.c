// SPDX-License-Identifier: GPL-3.0-or-later
#include "engine_internal.h"
#include "common/history.h"

// Called only by the develop worker. Copy values; never expose module pointers to Qt.
char *om_engine_history(OmEngine *engine) {
    JsonArray *rows = json_array_new();
    int index = g_list_length(engine->dev.history);
    for (GList *it = g_list_last(engine->dev.history); it; it = it->prev) {
        const dt_dev_history_item_t *item = it->data;
        JsonObject *row = json_object_new();
        char *label = dt_history_get_name_label(item->module ? item->module->name() : item->op_name,
                                                item->multi_name, FALSE, item->multi_name_hand_edited);
        json_object_set_int_member(row, "step", index);
        json_object_set_string_member(row, "label", label);
        json_object_set_string_member(row, "operation", item->op_name);
        json_object_set_boolean_member(row, "enabled", item->enabled);
        json_object_set_boolean_member(row, "active", index <= engine->dev.history_end);
        json_object_set_boolean_member(row, "current", index == engine->dev.history_end);
        json_array_add_object_element(rows, row);
        g_free(label);
        --index;
    }
    JsonObject *original = json_object_new();
    json_object_set_int_member(original, "step", 0);
    json_object_set_string_member(original, "label", "original");
    json_object_set_string_member(original, "operation", "");
    json_object_set_boolean_member(original, "enabled", FALSE);
    json_object_set_boolean_member(original, "active", TRUE);
    json_object_set_boolean_member(original, "current", engine->dev.history_end == 0);
    json_array_add_object_element(rows, original);
    JsonNode *node = json_node_new(JSON_NODE_ARRAY);
    json_node_take_array(node, rows);
    char *result = json_to_string(node, FALSE);
    json_node_free(node);
    return result;
}

int om_engine_history_select(OmEngine *engine, int step) {
    if (!engine->loaded || step < 0 || step > g_list_length(engine->dev.history))
        return 1;
    dt_dev_pop_history_items_ext(&engine->dev, step);
    dt_dev_pixelpipe_rebuild(&engine->dev);
    dt_dev_invalidate_all(&engine->dev);
    return om_engine_bind_controls(engine);
}

char *om_engine_history_snapshot(OmEngine *engine, const char *name) {
    return om_snapshot(engine, name, "", "*");
}
