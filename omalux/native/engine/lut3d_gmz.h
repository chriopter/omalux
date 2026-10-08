// SPDX-License-Identifier: GPL-3.0-or-later
// G'MIC compressed LUT reader for LUT 3D (lut3d_gmz.cpp). Never included by Qt code.
#pragma once
#ifdef __cplusplus
extern "C" {
#endif
// Read the LUT named lutname (the first when empty or not found) of a .gmz file: its keypoints
// (6 × nb_keypoints bytes, LUT 3D's c_clut layout), the name actually read (g_free) and every
// name in the file (g_strfreev). Returns 0, 2 unreadable, 3 incompatible LUT.
int om_lut3d_gmz_read(const char *filename, const char *lutname, int *nb_keypoints, unsigned char *keypoints,
                      char **found_name, char ***names, int *count);
// After an edit of LUT 3D: a .gmz file (or another LUT of it) is read into the parameters, as
// darktable's _filepath_callback / _lutname_callback with _get_compressed_clut (lut3d.c:1355-1490).
struct dt_iop_module_t;
void om_lut3d_gmz_changed(struct dt_iop_module_t *module, const void *before);
// The LUT names of the current .gmz file as JSON array text (g_free), NULL for other files.
char *om_lut3d_gmz_names(struct dt_iop_module_t *module);
#ifdef __cplusplus
}
#endif
