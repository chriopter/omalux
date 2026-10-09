// SPDX-License-Identifier: GPL-3.0-or-later
#pragma once
#ifdef __cplusplus
extern "C" {
#endif
// One worker owns this adapter. libdarktable itself remains process-global.
typedef struct OmEngine OmEngine;
typedef struct {
    double aspect_ratio;
    float scale_x, scale_y, offset_x, offset_y;
} OmPreviewGeometry;
OmEngine *om_engine_create(int argc, char **argv);
const char *om_engine_gpu_warning(OmEngine *engine);
int om_engine_open(OmEngine *engine, const char *path);
int om_engine_apply_style(OmEngine *engine, const char *path, const char *name, float *values);
char *om_engine_style_details(OmEngine *engine, const char *path, const char *name);
void om_engine_free_json(char *value);
void om_engine_read_controls(OmEngine *engine, float *values);
// The displayed value each control resets to for the open image: its module's default_params
// (what darktable's reset restores), the registry value where a control has no such parameter.
void om_engine_default_controls(OmEngine *engine, float *values);
int om_engine_update_controls(OmEngine *engine, const float *values, const unsigned char *changed);
int om_engine_render(OmEngine *engine, const unsigned char **pixels, int *width, int *height, int interactive,
                     OmPreviewGeometry *geometry);
void om_engine_cleanup(OmEngine *engine);
int om_engine_halation(OmEngine *engine);
int om_engine_preview_style(OmEngine *engine, const char *path, const char *name, unsigned char **pixels,
                            int *width, int *height, OmPreviewGeometry *geometry);
void om_engine_free_preview(unsigned char *pixels);
char *om_engine_metadata(OmEngine *engine);
// What darktable set up for this camera before any style: colour, lens and base tone.
char *om_engine_camera_defaults(OmEngine *engine);
// Every module and every parameter darktable describes about itself, as a JSON array.
char *om_engine_modules(OmEngine *engine);
// The same description per pipeline position, so a caller can refresh single modules.
int om_engine_module_count(OmEngine *engine);
const char *om_engine_module_identity(OmEngine *engine, int position, int *instance);
char *om_engine_module_at(OmEngine *engine, int position);
// Set one described scalar by module operation, instance and path relative to the params,
// e.g. "exposure", "tonecurve[0][1].x" or "@enabled". Returns 0, or 1 no image, 2 no such
// module, 3 unknown path, 4 unsupported type, 5 invalid value.
int om_engine_set_parameter(OmEngine *engine, const char *operation, int instance, const char *path,
                            double value);
// Set several scalars of one module from a JSON object {path: number}; one history item.
int om_engine_set_parameters(OmEngine *engine, const char *operation, int instance, const char *json);
// darktable's module reset: default parameters and blending, module switched on; one history item.
int om_engine_reset_module(OmEngine *engine, const char *operation, int instance);
// A list darktable fills at runtime for one row (profiles, lenses, LUT files, ...), filtered by
// query: {"items": [{"label", "detail", "section", "set": {path: value}}], "current", "more"}.
// Choosing an item is om_engine_set_parameters with its "set" object (module_choices.c).
char *om_engine_module_choices(OmEngine *engine, const char *operation, int instance, const char *list,
                               const char *query);
// Import camera presets (.dtpreset) so darktable auto-applies them; call before opening images.
int om_engine_import_camera_presets(const char *directory);
// Every imported Omalux camera preset for the open image, as a JSON array of {"name" (the
// preset's key), "title", "description", "maker", "model" (readable, "" for a whole maker),
// "operation", "module" (darktable's module name), "matches" (darktable auto-applies it to
// this image), "makerMatches", "general" (meant for every maker), "applied" (the module
// carries it now), "available", "reason" (why it cannot be put on this image)}.
char *om_engine_camera_presets(OmEngine *engine);
// Put a camera preset on the open image by hand (on) or take it off again, as one history
// item; applying is darktable's preset menu (camera_presets.c). Returns 0, or 1 no image,
// 2 no such preset or module, 3 wrong module version, 4 the module cannot run on this image,
// 5 the preset is not on the image.
int om_engine_camera_preset(OmEngine *engine, const char *name, int on);
int om_engine_export(OmEngine *engine, const char *filename, const char *format_name, int quality);
char *om_engine_snapshot(OmEngine *engine, const char *name, const char *prefix);
char *om_engine_module_snapshot(OmEngine *engine, const char *name, const char *module);
const char *om_engine_version(void);
char *om_engine_history(OmEngine *engine);
int om_engine_history_select(OmEngine *engine, int step);
char *om_engine_history_snapshot(OmEngine *engine, const char *name);
#ifdef __cplusplus
}
#endif
