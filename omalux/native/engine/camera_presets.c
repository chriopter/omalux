// SPDX-License-Identifier: GPL-3.0-or-later
// The Omalux camera presets (catalog/camera/*.dtpreset, imported into data.presets) as a list
// the user can pick from by hand, whether or not darktable's auto-apply matched the image.
//
// Offering: darktable's module preset menu (gui/presets.c, release-5.6.1,
// dt_gui_presets_popup_menu_show_for_module, 1624) lists a preset for every image when its
// `filter` is 0, whatever its camera patterns and format flags say; those only steer auto-apply.
// All catalogue presets have filter 0. The menu greys a preset of another module version (1768)
// and ticks the one whose parameters, blending and enabled state the module carries (1746).
//
// Applying follows dt_gui_presets_apply_preset (gui/presets.c, 1044) without its GTK calls; see
// om_engine_camera_preset.
#include "engine_internal.h"
#include "common/database.h"
#include "common/debug.h"
#include "common/image.h"
#include "control/conf.h"
#include "gui/presets.h"

// What a module looked like before a preset was put on it by hand, so taking the preset off
// returns exactly there, and what the preset left behind, to notice later edits.
typedef struct {
    dt_iop_module_t *module;
    void *params, *applied;
    char *preset; // the name of the preset put on
    dt_develop_blend_params_t blend, applied_blend;
    gboolean enabled;
    char multi_name[sizeof(((dt_iop_module_t *)0)->multi_name)];
    gboolean multi_name_hand_edited;
} OmPresetUndo;

static void undo_free(OmPresetUndo *undo) {
    g_free(undo->params);
    g_free(undo->applied);
    g_free(undo->preset);
    g_free(undo);
}

void om_camera_presets_clear(OmEngine *engine) {
    g_list_free_full(engine->camera_preset_undo, (GDestroyNotify)undo_free);
    engine->camera_preset_undo = NULL;
}

static GList *undo_link(OmEngine *engine, const dt_iop_module_t *module) {
    for (GList *it = engine->camera_preset_undo; it; it = it->next)
        if (((OmPresetUndo *)it->data)->module == module)
            return it;
    return NULL;
}

void om_camera_presets_forget(OmEngine *engine, const dt_iop_module_t *module) {
    GList *link = undo_link(engine, module);
    if (!link)
        return;
    undo_free(link->data);
    engine->camera_preset_undo = g_list_delete_link(engine->camera_preset_undo, link);
}

// The module still is as the hand-applied preset left it.
static gboolean undo_current(const OmPresetUndo *undo) {
    const dt_iop_module_t *module = undo->module;
    return module->enabled && !memcmp(module->params, undo->applied, module->params_size) &&
           !memcmp(module->blend_params, &undo->applied_blend, sizeof(undo->applied_blend));
}

typedef struct {
    char *name, *description, *operation, *maker, *model, *multi_name;
    int version, enabled, multi_name_hand_edited, blend_version;
    void *params, *blend;
    int params_size, blend_size;
    gboolean matches, maker_matches;
} OmPreset;

static void preset_free(OmPreset *preset) {
    g_free(preset->name);
    g_free(preset->description);
    g_free(preset->operation);
    g_free(preset->maker);
    g_free(preset->model);
    g_free(preset->multi_name);
    g_free(preset->params);
    g_free(preset->blend);
    g_free(preset);
}

static char *column_text(sqlite3_stmt *stmt, int column) {
    const char *text = (const char *)sqlite3_column_text(stmt, column);
    return g_strdup(text ? text : "");
}

// Every Omalux camera preset (only `only` when given), with whether darktable's auto-apply
// matches this image (develop.c _dev_auto_apply_presets, as in camera_defaults.c) and whether
// its maker pattern fits the image's maker.
static GList *load_presets(OmEngine *engine, const char *only) {
    const dt_image_t *image = &engine->dev.image_storage;
    int iformat = dt_image_is_raw(image) ? FOR_RAW : FOR_LDR;
    if (dt_image_is_matrix_correction_supported(image))
        iformat |= FOR_MATRIX;
    if (dt_image_is_hdr(image))
        iformat |= FOR_HDR;
    const int excluded = dt_image_monochrome_flags(image) ? FOR_NOT_MONO : FOR_NOT_COLOR;
    sqlite3_stmt *stmt;
    DT_DEBUG_SQLITE3_PREPARE_V2(
        dt_database_get(darktable.db),
        "SELECT name, description, operation, op_version, op_params, enabled, blendop_params,"
        "       blendop_version, multi_name, multi_name_hand_edited, maker, model,"
        "       (autoapply=1"
        "        AND ((?1 LIKE model AND ?2 LIKE maker) OR (?3 LIKE model AND ?4 LIKE maker))"
        "        AND ?5 LIKE lens AND ?6 BETWEEN iso_min AND iso_max"
        "        AND ?7 BETWEEN exposure_min AND exposure_max"
        "        AND ?8 BETWEEN aperture_min AND aperture_max"
        "        AND ?9 BETWEEN focal_length_min AND focal_length_max"
        "        AND (format = 0 OR (format&?10 != 0 AND ~format&?11 != 0))),"
        "       (maker != '%' AND ((?2 != '' AND ?2 LIKE maker) OR (?4 != '' AND ?4 LIKE maker)))"
        " FROM data.presets"
        " WHERE name LIKE 'Omalux %' AND (?12 = '' OR name = ?12)"
        " ORDER BY LOWER(REPLACE(maker, '%', '')), LENGTH(model), LOWER(description)",
        -1, &stmt, NULL);
    DT_DEBUG_SQLITE3_BIND_TEXT(stmt, 1, image->exif_model, -1, SQLITE_TRANSIENT);
    DT_DEBUG_SQLITE3_BIND_TEXT(stmt, 2, image->exif_maker, -1, SQLITE_TRANSIENT);
    DT_DEBUG_SQLITE3_BIND_TEXT(stmt, 3, image->camera_alias, -1, SQLITE_TRANSIENT);
    DT_DEBUG_SQLITE3_BIND_TEXT(stmt, 4, image->camera_maker, -1, SQLITE_TRANSIENT);
    DT_DEBUG_SQLITE3_BIND_TEXT(stmt, 5, image->exif_lens, -1, SQLITE_TRANSIENT);
    DT_DEBUG_SQLITE3_BIND_DOUBLE(stmt, 6, fmaxf(0.0f, fminf(FLT_MAX, image->exif_iso)));
    DT_DEBUG_SQLITE3_BIND_DOUBLE(stmt, 7, fmaxf(0.0f, fminf(1000000, image->exif_exposure)));
    DT_DEBUG_SQLITE3_BIND_DOUBLE(stmt, 8, fmaxf(0.0f, fminf(1000000, image->exif_aperture)));
    DT_DEBUG_SQLITE3_BIND_DOUBLE(stmt, 9, fmaxf(0.0f, fminf(1000000, image->exif_focal_length)));
    DT_DEBUG_SQLITE3_BIND_INT(stmt, 10, iformat);
    DT_DEBUG_SQLITE3_BIND_INT(stmt, 11, excluded);
    DT_DEBUG_SQLITE3_BIND_TEXT(stmt, 12, only ? only : "", -1, SQLITE_TRANSIENT);
    GList *presets = NULL;
    while (sqlite3_step(stmt) == SQLITE_ROW) {
        OmPreset *preset = g_new0(OmPreset, 1);
        preset->name = column_text(stmt, 0);
        preset->description = column_text(stmt, 1);
        preset->operation = column_text(stmt, 2);
        preset->version = sqlite3_column_int(stmt, 3);
        preset->params_size = sqlite3_column_bytes(stmt, 4);
        preset->params =
            preset->params_size ? g_memdup2(sqlite3_column_blob(stmt, 4), preset->params_size) : NULL;
        preset->enabled = sqlite3_column_int(stmt, 5);
        preset->blend_size = sqlite3_column_bytes(stmt, 6);
        preset->blend =
            preset->blend_size ? g_memdup2(sqlite3_column_blob(stmt, 6), preset->blend_size) : NULL;
        preset->blend_version = sqlite3_column_int(stmt, 7);
        preset->multi_name = column_text(stmt, 8);
        preset->multi_name_hand_edited = sqlite3_column_int(stmt, 9);
        preset->maker = column_text(stmt, 10);
        preset->model = column_text(stmt, 11);
        preset->matches = sqlite3_column_int(stmt, 12);
        preset->maker_matches = sqlite3_column_int(stmt, 13);
        presets = g_list_prepend(presets, preset);
    }
    sqlite3_finalize(stmt);
    return g_list_reverse(presets);
}

// Why this preset cannot be put on this image, or NULL when it can. A preset darktable would
// apply without effect (or not at all) is refused with its reason instead.
static const char *unavailable(OmEngine *engine, const OmPreset *preset, dt_iop_module_t *module) {
    if (!module)
        return "this darktable has no such module";
    // gui/presets.c 1727, 1771.
    if (preset->version != module->version())
        return "wrong module version";
    // Modules darktable takes out of reach for this image (the raw-only ones on a JPEG) have
    // no enable button and are off.
    if (dt_iop_is_hidden(module) || (module->hide_enable_button && !module->enabled))
        return "this module does not run on this image";
    // lens.cc _get_method (3237): "embedded metadata" silently becomes Lensfun when the file
    // carries no correction data, which is a different correction than the preset names.
    if (dt_iop_module_is(module, "lens") && preset->params && preset->params_size == module->params_size &&
        module->get_p) {
        const int *method = module->get_p(preset->params, "method");
        if (method && *method == 0 && !engine->dev.image_storage.exif_correction_type)
            return "this photo carries no embedded lens data";
    }
    return NULL;
}

// The tick of darktable's preset menu (gui/presets.c 1746): the module carries the preset's
// parameters and blending, and both are enabled. One difference: the blending is compared as
// applying leaves it. A preset stored without a blend colour space gets the module's default
// one when it is committed (dt_iop_commit_blend_params), which darktable's byte comparison
// then takes for a difference, so its menu never ticks such a preset.
static gboolean carries(dt_iop_module_t *module, const OmPreset *preset) {
    const gboolean params =
        preset->params_size == 0
            ? !memcmp(module->params, module->default_params, module->params_size)
            : !memcmp(module->params, preset->params, MIN(preset->params_size, module->params_size));
    if (!params || !module->enabled || !preset->enabled)
        return FALSE;
    if (preset->blend_size != sizeof(dt_develop_blend_params_t))
        return !memcmp(module->blend_params, preset->blend,
                       MIN((size_t)preset->blend_size, sizeof(dt_develop_blend_params_t)));
    dt_develop_blend_params_t blend;
    memcpy(&blend, preset->blend, sizeof(blend));
    if (blend.blend_cst == DEVELOP_BLEND_CS_NONE)
        blend.blend_cst = dt_develop_blend_default_module_blend_colorspace(module);
    return !memcmp(module->blend_params, &blend, sizeof(blend));
}

// The name a preset leaves on its module (dt_presets_get_multi_name).
static const char *module_label(const OmPreset *preset) {
    return preset->multi_name[0] ? preset->multi_name : preset->name;
}

// "Fujifilm X-T10: exposure" → camera "Fujifilm X-T10", title "exposure".
static char *camera_of(const OmPreset *preset, const char **title) {
    const char *colon = strstr(preset->description, ": ");
    *title = colon ? colon + 2 : preset->description;
    return colon ? g_strndup(preset->description, colon - preset->description) : g_strdup("");
}

// Presets of one maker share its pattern without the wildcards ("FUJIFILM", "FUJIFILM%").
static char *maker_key(const OmPreset *preset) {
    GString *key = g_string_new(NULL);
    for (const char *c = preset->maker; *c; ++c)
        if (*c != '%')
            g_string_append_c(key, g_ascii_tolower(*c));
    return g_strstrip(g_string_free(key, FALSE));
}

// A readable maker name: what a maker-wide preset calls it ("OM System: …"), else the words the
// camera name shares with the pattern ("Google Pixel" and "Google" → "Google").
static char *maker_label(GList *presets, const char *key) {
    char *shared = NULL;
    for (GList *it = presets; it; it = it->next) {
        const OmPreset *preset = it->data;
        char *other = maker_key(preset);
        const gboolean same = !strcmp(other, key);
        g_free(other);
        if (!same)
            continue;
        const char *title;
        char *camera = camera_of(preset, &title);
        if (!strcmp(preset->model, "%") && *camera) {
            g_free(shared);
            return camera;
        }
        size_t length = 0;
        while (camera[length] && key[length] && g_ascii_tolower(camera[length]) == key[length])
            ++length;
        if (!shared && length)
            shared = g_strstrip(g_strndup(camera, length));
        g_free(camera);
    }
    return shared ? shared : g_strdup(*key ? key : "Every camera");
}

char *om_engine_camera_presets(OmEngine *engine) {
    if (!engine->loaded)
        return NULL;
    GList *presets = load_presets(engine, NULL);
    JsonArray *rows = json_array_new();
    for (GList *it = presets; it; it = it->next) {
        const OmPreset *preset = it->data;
        dt_iop_module_t *module = dt_iop_get_module_by_op_priority(engine->dev.iop, preset->operation, 0);
        const char *reason = unavailable(engine, preset, module);
        gboolean applied = module && !reason && carries(module, preset);
        // Several presets may hold the same settings (the embedded lens correction of different
        // makers, the same exposure offset for two cameras). The module's label cannot tell
        // them apart (see apply), so the one put on by hand counts, else those that came with
        // the image.
        if (applied) {
            GList *link = undo_link(engine, module);
            const OmPresetUndo *undo = link && undo_current(link->data) ? link->data : NULL;
            gboolean matched_twin = FALSE;
            for (GList *other = presets; other; other = other->next) {
                const OmPreset *twin = other->data;
                if (twin != preset && twin->matches && !strcmp(twin->operation, preset->operation) &&
                    carries(module, twin))
                    matched_twin = TRUE;
            }
            if (undo)
                applied = !strcmp(undo->preset, preset->name);
            else if (!preset->matches && matched_twin)
                applied = FALSE;
        }
        const char *title;
        char *camera = camera_of(preset, &title);
        char *key = maker_key(preset);
        char *maker = maker_label(presets, key);
        // The camera without its maker: "Fujifilm X-T10" → "X-T10"; empty for a whole maker.
        const size_t skip = g_ascii_strncasecmp(camera, maker, strlen(maker)) ? 0 : strlen(maker);
        char *model = g_strstrip(g_strdup(camera + skip));
        JsonObject *row = json_object_new();
        json_object_set_string_member(row, "name", preset->name);
        json_object_set_string_member(row, "title", title);
        json_object_set_string_member(row, "description", preset->description);
        json_object_set_string_member(row, "maker", maker);
        json_object_set_string_member(row, "model", model);
        json_object_set_string_member(row, "operation", preset->operation);
        json_object_set_string_member(row, "module", module ? module->name() : preset->operation);
        json_object_set_boolean_member(row, "matches", preset->matches);
        json_object_set_boolean_member(row, "makerMatches", preset->maker_matches);
        json_object_set_boolean_member(row, "general", !strcmp(preset->maker, "%"));
        json_object_set_boolean_member(row, "applied", applied);
        json_object_set_boolean_member(row, "available", reason == NULL);
        json_object_set_string_member(row, "reason", reason ? reason : "");
        json_array_add_object_element(rows, row);
        g_free(camera);
        g_free(key);
        g_free(maker);
        g_free(model);
    }
    g_list_free_full(presets, (GDestroyNotify)preset_free);
    JsonNode *node = json_node_new(JSON_NODE_ARRAY);
    json_node_take_array(node, rows);
    char *result = json_to_string(node, FALSE);
    json_node_free(node);
    return result;
}

// dt_gui_presets_apply_preset (gui/presets.c 1044) on the module's base instance, without GTK:
// the preset's parameters (the defaults when their size does not fit), its enabled state, its
// name as the module's label unless that was typed by hand (dt_iop_update_multi_name), its
// blending (converted from an older version, or the defaults), then one history item. Writing
// the item, darktable derives the label again from the presets whose parameters and blending
// equal the module's byte for byte (develop.c _dev_auto_module_label); a preset stored without a
// blend colour space never does (see carries), so for the catalogue's presets the label ends
// up empty, here as in darktable. Left out
// are the GUI refresh, the "last preset" memory of the menu and DT_SIGNAL_PRESET_APPLIED, whose
// only listener is demosaic's GUI. dt_dev_add_history_item needs an attached GUI, so the item is
// written with dt_dev_add_history_item_ext, as everywhere in this adapter.
static void apply(OmEngine *engine, dt_iop_module_t *module, const OmPreset *preset) {
    GList *link = undo_link(engine, module);
    // A second preset on the same module keeps the state from before the first.
    if (link && !undo_current(link->data)) {
        om_camera_presets_forget(engine, module);
        link = NULL;
    }
    OmPresetUndo *undo = link ? link->data : g_new0(OmPresetUndo, 1);
    if (!link) {
        undo->module = module;
        undo->params = g_memdup2(module->params, module->params_size);
        undo->blend = *module->blend_params;
        undo->enabled = module->enabled;
        g_strlcpy(undo->multi_name, module->multi_name, sizeof(undo->multi_name));
        undo->multi_name_hand_edited = module->multi_name_hand_edited;
        engine->camera_preset_undo = g_list_prepend(engine->camera_preset_undo, undo);
    }
    if (preset->params && preset->params_size == module->params_size)
        memcpy(module->params, preset->params, module->params_size);
    else
        memcpy(module->params, module->default_params, module->params_size);
    module->enabled = preset->enabled;
    if (dt_conf_get_bool("darkroom/ui/auto_module_name_update") && !module->multi_name_hand_edited) {
        char *label = g_strstrip(g_strdup(module_label(preset)));
        g_strlcpy(module->multi_name, label, sizeof(module->multi_name));
        module->multi_name_hand_edited = preset->multi_name_hand_edited;
        g_free(label);
    }
    if (preset->blend && preset->blend_version == dt_develop_blend_version() &&
        preset->blend_size == sizeof(dt_develop_blend_params_t))
        dt_iop_commit_blend_params(module, preset->blend);
    else if (preset->blend && dt_develop_blend_legacy_params(module, preset->blend, preset->blend_version,
                                                             module->blend_params, dt_develop_blend_version(),
                                                             preset->blend_size) == 0) {
        // converted in place
    } else
        dt_iop_commit_blend_params(module, module->default_blendop_params);
    g_free(undo->applied);
    g_free(undo->preset);
    undo->preset = g_strdup(preset->name);
    undo->applied = g_memdup2(module->params, module->params_size);
    undo->applied_blend = *module->blend_params;
}

// Taking a preset off: back to what the module was before it was put on by hand; a preset that
// came with the image (auto-applied) or was edited since leaves the module at its defaults.
static void take_off(OmEngine *engine, dt_iop_module_t *module) {
    GList *link = undo_link(engine, module);
    const OmPresetUndo *undo = link && undo_current(link->data) ? link->data : NULL;
    memcpy(module->params, undo ? undo->params : module->default_params, module->params_size);
    dt_iop_commit_blend_params(module, undo ? &undo->blend : module->default_blendop_params);
    module->enabled = undo ? undo->enabled : module->default_enabled;
    if (undo) {
        g_strlcpy(module->multi_name, undo->multi_name, sizeof(module->multi_name));
        module->multi_name_hand_edited = undo->multi_name_hand_edited;
    } else if (!module->multi_name_hand_edited)
        module->multi_name[0] = '\0';
    om_camera_presets_forget(engine, module);
}

int om_engine_camera_preset(OmEngine *engine, const char *name, int on) {
    if (!engine->loaded)
        return 1;
    GList *presets = name && *name ? load_presets(engine, name) : NULL;
    if (!presets)
        return 2;
    const OmPreset *preset = presets->data;
    dt_iop_module_t *module = dt_iop_get_module_by_op_priority(engine->dev.iop, preset->operation, 0);
    int result = 0;
    if (unavailable(engine, preset, module))
        result = !module ? 2 : preset->version != module->version() ? 3 : 4;
    else if (on)
        apply(engine, module, preset);
    else if (carries(module, preset))
        take_off(engine, module);
    else
        result = 5; // not on the image: nothing to take off
    g_list_free_full(presets, (GDestroyNotify)preset_free);
    if (result)
        return result;
    dt_dev_add_history_item_ext(&engine->dev, module, FALSE, FALSE);
    engine->dev.full.pipe->changed |= DT_DEV_PIPE_SYNCH;
    return om_engine_bind_controls(engine);
}
