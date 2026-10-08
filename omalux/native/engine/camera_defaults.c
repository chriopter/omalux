// SPDX-License-Identifier: GPL-3.0-or-later
// What darktable set up for this camera before any style: colour, lens and base tone.
// Read from the loaded modules, so it reports the actual state, not what we expect.
#include "engine_internal.h"
#include "common/colorspaces.h"
#include "common/image.h"
#include "common/presets.h"
#include "gui/presets.h"
#include "common/database.h"
#include "common/debug.h"
#include "white_balance.h"

static dt_iop_module_t *module_of(OmEngine *engine, const char *op) {
    return dt_iop_get_module_by_op_priority(engine->dev.iop, op, 0);
}

static void add_entry(JsonArray *rows, const char *group, const char *module, const char *label,
                      const char *value, gboolean enabled, gboolean present) {
    JsonObject *row = json_object_new();
    json_object_set_string_member(row, "group", group);
    json_object_set_string_member(row, "module", module);
    json_object_set_string_member(row, "label", label);
    json_object_set_string_member(row, "value", value);
    json_object_set_boolean_member(row, "enabled", enabled);
    json_object_set_boolean_member(row, "present", present);
    json_array_add_object_element(rows, row);
}

// The tone mapper darktable's workflow enabled for this image, if any.
static dt_iop_module_t *active_tone_mapper(OmEngine *engine, const char **name) {
    static const char *candidates[][2] = {
        {"sigmoid", "sigmoid"}, {"filmicrgb", "filmic rgb"}, {"agx", "AgX"}, {"basecurve", "base curve"}};
    for (unsigned int i = 0; i < G_N_ELEMENTS(candidates); ++i) {
        dt_iop_module_t *module = module_of(engine, candidates[i][0]);
        if (module && module->enabled) {
            *name = candidates[i][1];
            return module;
        }
    }
    return NULL;
}

// Omalux camera presets (catalog/camera/*.dtpreset) that match this image the way darktable's
// auto-apply does (camera, lens, ISO, exposure, aperture, focal length and the raw/LDR/HDR,
// matrix and monochrome format flags, as in develop.c _dev_auto_apply_presets). "enabled" is whether that module is on in the loaded pipeline.
static void add_camera_presets(OmEngine *engine, JsonArray *rows) {
    const dt_image_t *image = &engine->dev.image_storage;
    int iformat = dt_image_is_raw(image) ? FOR_RAW : FOR_LDR;
    if (dt_image_is_matrix_correction_supported(image))
        iformat |= FOR_MATRIX;
    if (dt_image_is_hdr(image))
        iformat |= FOR_HDR;
    const int excluded = dt_image_monochrome_flags(image) ? FOR_NOT_MONO : FOR_NOT_COLOR;
    sqlite3_stmt *stmt;
    DT_DEBUG_SQLITE3_PREPARE_V2(dt_database_get(darktable.db),
                                "SELECT operation, description FROM data.presets"
                                " WHERE autoapply=1 AND name LIKE 'Omalux %'"
                                "   AND ((?1 LIKE model AND ?2 LIKE maker) OR (?3 LIKE model AND ?4 LIKE maker))"
                                "   AND ?5 LIKE lens AND ?6 BETWEEN iso_min AND iso_max"
                                "   AND ?7 BETWEEN exposure_min AND exposure_max"
                                "   AND ?8 BETWEEN aperture_min AND aperture_max"
                                "   AND ?9 BETWEEN focal_length_min AND focal_length_max"
                                "   AND (format = 0 OR (format&?10 != 0 AND ~format&?11 != 0))"
                                " ORDER BY LENGTH(maker), LENGTH(model), operation",
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
    while (sqlite3_step(stmt) == SQLITE_ROW) {
        const char *op = (const char *)sqlite3_column_text(stmt, 0);
        const char *description = (const char *)sqlite3_column_text(stmt, 1);
        dt_iop_module_t *module = op ? module_of(engine, op) : NULL;
        // "Fujifilm X-T10: input profile" → "input profile"; the camera is shown once above.
        const char *colon = description ? strstr(description, ": ") : NULL;
        add_entry(rows, "Camera presets", op ? op : "", module ? module->name() : (op ? op : ""),
                  colon ? colon + 2 : (description ? description : ""), module && module->enabled, module != NULL);
    }
    sqlite3_finalize(stmt);
}

char *om_engine_camera_defaults(OmEngine *engine) {
    if (!engine->loaded)
        return NULL;
    const dt_image_t *image = &engine->dev.image_storage;
    const gboolean is_raw = dt_image_is_raw(image);
    JsonObject *root = json_object_new();
    json_object_set_string_member(root, "camera", image->camera_makermodel);
    json_object_set_boolean_member(root, "raw", is_raw);
    JsonArray *rows = json_array_new();

    // Colour: the input profile darktable chose for this camera, plus the white balance it read.
    dt_iop_module_t *colorin = module_of(engine, "colorin");
    if (colorin) {
        const int *type = colorin->get_p(colorin->params, "type");
        const char *filename = colorin->get_p(colorin->params, "filename");
        char value[256];
        const char *profile = dt_colorspaces_get_name(*type, filename);
        // For a profile from color/in, prefer its own description over the file name.
        if (*type == DT_COLORSPACE_FILE)
            for (GList *it = darktable.color_profiles->profiles; it; it = it->next) {
                const dt_colorspaces_color_profile_t *p = it->data;
                if (p->type != DT_COLORSPACE_FILE)
                    continue;
                // The profile list keeps the full path, the module only the file name.
                char *base = g_path_get_basename(p->filename);
                const gboolean match = !strcmp(base, filename);
                g_free(base);
                if (match) {
                    profile = p->name;
                    break;
                }
            }
        // The standard/enhanced matrices are per camera; say so, they are not a generic fallback.
        if (is_raw && (*type == DT_COLORSPACE_STANDARD_MATRIX || *type == DT_COLORSPACE_ENHANCED_MATRIX))
            snprintf(value, sizeof(value), "%s · %s", profile, image->camera_makermodel);
        else
            g_strlcpy(value, profile, sizeof(value));
        add_entry(rows, "Colour", "colorin", "input profile", value, colorin->enabled, TRUE);
    }
    dt_iop_module_t *calibration = module_of(engine, "channelmixerrgb");
    if (calibration && calibration->enabled && is_raw)
        add_entry(rows, "Colour", "channelmixerrgb", "color calibration", "scene-referred default", TRUE, TRUE);
    dt_iop_module_t *temperature = module_of(engine, "temperature");
    if (temperature && is_raw) {
        float kelvin = 0, tint = 0;
        char value[64] = "camera reference";
        if (om_wb_read(temperature, temperature->params, &kelvin, &tint))
            snprintf(value, sizeof(value), "%.0f K, tint %.2f", (double)kelvin, (double)tint);
        add_entry(rows, "Colour", "temperature", "white balance", value, temperature->enabled, TRUE);
    }

    // Lens: correction for this camera and lens, from embedded metadata or the Lensfun database.
    dt_iop_module_t *lens = module_of(engine, "lens");
    if (lens) {
        const int *method = lens->get_p(lens->params, "method");
        const char *source = *method == 0 ? "embedded metadata" : *method == 1 ? "Lensfun database" : "manual vignette";
        add_entry(rows, "Lens", "lens", "correction", lens->enabled ? source : "not applied",
                  lens->enabled, TRUE);
    }

    // Base tone: what the scene-referred workflow set up before any look.
    dt_iop_module_t *exposure = module_of(engine, "exposure");
    if (exposure) {
        char value[64];
        snprintf(value, sizeof(value), "%+.2f EV", (double)*(float *)exposure->get_p(exposure->params, "exposure"));
        add_entry(rows, "Base tone", "exposure", "exposure", value, exposure->enabled, TRUE);
    }
    const char *tone_name = NULL;
    dt_iop_module_t *tone = active_tone_mapper(engine, &tone_name);
    add_entry(rows, "Base tone", tone ? tone->op : "", "tone mapping", tone ? tone_name : "none",
              tone != NULL, TRUE);
    dt_iop_module_t *highlights = module_of(engine, "highlights");
    if (highlights && is_raw)
        add_entry(rows, "Base tone", "highlights", "highlight reconstruction",
                  highlights->enabled ? "on" : "not applied", highlights->enabled, TRUE);
    dt_iop_module_t *denoise = module_of(engine, "denoiseprofile");
    if (denoise && is_raw)
        add_entry(rows, "Base tone", "denoiseprofile", "denoising",
                  denoise->enabled ? "camera noise profile" : "not applied", denoise->enabled, TRUE);
    dt_iop_module_t *sharpen = module_of(engine, "sharpen");
    if (sharpen)
        add_entry(rows, "Base tone", "sharpen", "sharpening",
                  sharpen->enabled ? "on" : "not applied", sharpen->enabled, TRUE);

    add_camera_presets(engine, rows);

    json_object_set_array_member(root, "entries", rows);
    JsonNode *node = json_node_new(JSON_NODE_OBJECT);
    json_node_take_object(node, root);
    char *result = json_to_string(node, FALSE);
    json_node_free(node);
    return result;
}

// darktable applies module presets marked "autoapply" when an image is first developed. Ours
// live as .dtpreset files next to the styles; import them into the session database before any
// image is opened, exactly as a darktable user would from preferences.
int om_engine_import_camera_presets(const char *directory) {
    if (!directory || !*directory)
        return 0;
    GDir *dir = g_dir_open(directory, 0, NULL);
    if (!dir)
        return 0;
    int count = 0;
    const char *entry;
    while ((entry = g_dir_read_name(dir))) {
        char *path = g_build_filename(directory, entry, NULL);
        if (g_file_test(path, G_FILE_TEST_IS_DIR))
            count += om_engine_import_camera_presets(path);
        else if (g_str_has_suffix(entry, ".dtpreset"))
            count += dt_presets_import_from_file(path) ? 1 : 0;
        g_free(path);
    }
    g_dir_close(dir);
    return count;
}
