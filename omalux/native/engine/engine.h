// SPDX-License-Identifier: GPL-3.0-or-later
#pragma once
#ifdef __cplusplus
extern "C" {
#endif
// One worker owns this adapter. libdarktable itself remains process-global.
typedef struct OmEngine OmEngine;
OmEngine *om_engine_create(int argc, char **argv);
const char *om_engine_gpu_warning(OmEngine *engine);
int om_engine_open(OmEngine *engine, const char *path);
int om_engine_apply_style(OmEngine *engine, const char *path, const char *name, float *values);
char *om_engine_style_details(OmEngine *engine, const char *path, const char *name);
void om_engine_free_json(char *value);
void om_engine_read_controls(OmEngine *engine, float *values);
int om_engine_update_controls(OmEngine *engine, const float *values, const unsigned char *changed);
int om_engine_render(OmEngine *engine, const unsigned char **pixels, int *width, int *height,
                     int interactive);
void om_engine_cleanup(OmEngine *engine);
int om_engine_halation(OmEngine *engine);
int om_engine_preview_style(OmEngine *engine, const char *path, const char *name, unsigned char **pixels,
                            int *width, int *height);
void om_engine_free_preview(unsigned char *pixels);
char *om_engine_metadata(OmEngine *engine);
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
