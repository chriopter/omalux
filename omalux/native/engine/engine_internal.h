// SPDX-License-Identifier: GPL-3.0-or-later
#pragma once
#include "engine.h"
#include "controls.h"
#include "common/darktable.h"
#include "common/styles.h"
#include "common/iop_order.h"
#include "develop/develop.h"
#include "develop/imageop.h"
#include "develop/pixelpipe.h"
#include "develop/pixelpipe_hb.h"
#include "develop/blend.h"
#include <json-glib/json-glib.h>
enum {
    OM_PREVIEW_WIDTH = 1400,
    OM_PREVIEW_HEIGHT = 1000,
    OM_FAST_PREVIEW_WIDTH = 700,
    OM_FAST_PREVIEW_HEIGHT = 500
};
struct OmEngine {
    dt_develop_t dev;
    dt_iop_module_t *modules[OM_CONTROL_COUNT];
    float *parameters[OM_CONTROL_COUNT];
    int loaded;
    float wb_temperature, wb_tint;
    float enabled_values[OM_CONTROL_COUNT], integer_values[OM_CONTROL_COUNT];
    int *integer_parameters[OM_CONTROL_COUNT];
    GList *style_baseline;
    // Module states from before a camera preset was applied by hand (camera_presets.c).
    GList *camera_preset_undo;
    // Drawn shapes last rendered (canvas.c): their change invalidates the pipe cache.
    guint64 canvas_forms_hash;
};
typedef struct {
    dt_iop_module_t *module;
    void *params;
    dt_develop_blend_params_t blend;
    gboolean enabled;
} OmStyleBaseline;
extern const char darktable_package_version[];
int om_engine_bind_controls(OmEngine *engine);
void om_style_baseline_clear(OmEngine *engine);
void om_style_baseline_capture(OmEngine *engine);
void om_style_baseline_restore(OmEngine *engine);
// Blending and instances (blending.c, module_instances.c).
void om_style_baseline_forget(OmEngine *engine, const dt_iop_module_t *module);
void om_camera_presets_clear(OmEngine *engine);
void om_camera_presets_forget(OmEngine *engine, const dt_iop_module_t *module);
void om_camera_presets_add_matched(OmEngine *engine, JsonArray *rows);
void om_instance_describe(JsonObject *entry, dt_iop_module_t *module);
char *om_snapshot(OmEngine *engine, const char *name, const char *prefix, const char *only_module);

void om_preview_geometry(const dt_dev_pixelpipe_t *pipe, OmPreviewGeometry *geometry);

// ---- module values and choices (module_values.c, module_choices.c) ----
// darktable's displayed conversions ("@" paths) and runtime lists. describe adds "derived"
// {path: value} and "labels" {field: text} to a module's catalog entry; set writes "@" paths
// (numbers, or texts for file and lens choices) into module->params without a history item
// and returns 0, 3 unknown path or 5 invalid value.
void om_module_describe_values(dt_iop_module_t *module, JsonObject *entry);
void om_module_describe_choices(dt_iop_module_t *module, JsonObject *entry);
int om_module_set_values(OmEngine *engine, dt_iop_module_t *module, size_t count, const char *const *paths,
                         const double *values, const char *const *texts);
// The choice paths (lens, LUT, raster and overlay files); -1 when none of the paths is one.
int om_module_set_choice(OmEngine *engine, dt_iop_module_t *module, size_t count, const char *const *paths,
                         const double *values, const char *const *texts);
void om_wb_list(dt_iop_module_t *module, int *current, JsonArray *items);
