// SPDX-License-Identifier: GPL-3.0-or-later
#pragma once
// Private helpers shared by the on-canvas tools (canvas.c, shapes.c, liquify_canvas.c).
#include "engine_internal.h"
#include "canvas.h"
#include "develop/masks.h"

// The coordinate spaces of the full pipe after its last render.
typedef struct {
    dt_develop_t *dev;
    dt_dev_pixelpipe_t *pipe;
    float width, height;   // processed (output) size in pipe pixels: preview 0..1 maps onto it
    float iwidth, iheight; // input size in pipe pixels: darktable's drawn forms are normalised by it
} OmSpace;

gboolean om_space_init(OmEngine *engine, OmSpace *space);
// Preview 0..1 ↔ output pixels.
void om_space_from_preview(const OmSpace *space, float *points, int count);
void om_space_to_preview(const OmSpace *space, float *points, int count);
// Output pixels ↔ input (raw) pixels through every distorting module.
gboolean om_space_backtransform(const OmSpace *space, float *points, int count);
gboolean om_space_transform(const OmSpace *space, float *points, int count);
// Restricted transforms relative to one module (darktable's DT_DEV_TRANSFORM_DIR_*).
gboolean om_space_transform_dir(const OmSpace *space, double iop_order, int direction, gboolean back,
                                float *points, int count);

// JSON helpers.
JsonObject *om_gesture_parse(const char *json, JsonParser **parser);
gboolean om_gesture_point(JsonObject *gesture, const char *member, float *xy);
double om_gesture_number(JsonObject *gesture, const char *member, double fallback);
void om_json_point(JsonBuilder *builder, const char *member, float x, float y);
void om_json_polyline(JsonBuilder *builder, const char *member, const float *xy, int count);
char *om_json_finish(JsonBuilder *builder);

// Record a parameter change of `module` (and its forms when `masks`) in history.
int om_canvas_commit(OmEngine *engine, dt_iop_module_t *module, gboolean masks);

// Per-module tools.
void om_shapes_overlay(OmEngine *engine, const OmSpace *space, dt_iop_module_t *module, JsonBuilder *builder);
int om_shapes_edit(OmEngine *engine, const OmSpace *space, dt_iop_module_t *module, JsonObject *gesture);
gboolean om_shapes_module(const dt_iop_module_t *module);
guint64 om_shapes_hash(OmEngine *engine);
void om_shapes_selection_changed(OmEngine *engine, dt_iop_module_t *module);
void om_liquify_overlay(OmEngine *engine, const OmSpace *space, dt_iop_module_t *module, JsonBuilder *builder);
int om_liquify_edit(OmEngine *engine, const OmSpace *space, dt_iop_module_t *module, JsonObject *gesture);
