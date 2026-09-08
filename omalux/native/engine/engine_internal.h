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
    GList *preset_baseline;
};
typedef struct {
    dt_iop_module_t *module;
    void *params;
    dt_develop_blend_params_t blend;
    gboolean enabled;
} OmPresetBaseline;
extern const char darktable_package_version[];
int om_engine_bind_controls(OmEngine *engine);
void om_preset_baseline_clear(OmEngine *engine);
void om_preset_baseline_capture(OmEngine *engine);
void om_preset_baseline_restore(OmEngine *engine);
char *om_snapshot(OmEngine *engine, const char *name, const char *prefix, const char *only_module);

void om_preview_geometry(const dt_dev_pixelpipe_t *pipe, OmPreviewGeometry *geometry);
