// SPDX-License-Identifier: GPL-3.0-or-later
// G'MIC compressed LUTs (.gmz) for LUT 3D without darktable's GUI: the reader of
// src/iop/lut3dgmic.cpp (lut3d_read_gmz, lines 158-226) with its GTK list callbacks replaced by
// a plain list of names. The decompression itself runs inside darktable's lut3d plugin when the
// pipe commits the parameters (lut3d_decompress_clut), as in darktable.
#include "lut3d_gmz.h"
#include <gmic.h>
#include <cstdio>
#include <cstring>
#include <glib.h>

extern "C" int om_lut3d_gmz_read(const char *filename, const char *lutname, int *nb_keypoints,
                                 unsigned char *keypoints, char **found_name, char ***names, int *count) {
    gmic_list<float> image_list;
    gmic_list<char> image_names;
    char cmd[4096];
    gmic instance;
    instance.verbosity = -1;
    if (found_name)
        *found_name = nullptr;
    if (names)
        *names = nullptr;
    if (count)
        *count = 0;
    try {
        std::snprintf(cmd, sizeof(cmd), "-i \"%s\"", filename);
        instance.run(cmd, image_list, image_names);
    } catch (gmic_exception &e) {
        std::fprintf(stderr, "[lut3d gmic] error: \"%s\"\n", e.what());
        return 2;
    }
    if (!image_names._width)
        return 2;
    unsigned int l = 0;
    if (lutname && lutname[0])
        for (unsigned int i = 0; i < image_names._width; ++i)
            if (!std::strcmp(image_names[i]._data, lutname)) {
                l = i;
                break;
            }
    if (names) {
        *names = g_new0(char *, image_names._width + 1);
        for (unsigned int i = 0; i < image_names._width; ++i)
            (*names)[i] = g_strdup(image_names[i]._data);
    }
    if (count)
        *count = (int)image_names._width;
    if (found_name)
        *found_name = g_strdup(image_names[l]._data);
    if (!keypoints || !nb_keypoints)
        return 0;
    const gmic_image<float> &img = image_list[l];
    const int nb_kp = (int)img._height;
    if (img._width == 1 && img._height <= 2048 && img._depth == 1 && img._spectrum == 6) {
        *nb_keypoints = nb_kp;
        for (int i = 0; i < nb_kp * 6; ++i)
            keypoints[i] = (unsigned char)img[i];
    } else if (img._width == 1 && img._height <= 2048 && img._depth == 1 && img._spectrum == 4) {
        *nb_keypoints = nb_kp;
        for (int i = 0; i < nb_kp * 3; ++i)
            keypoints[i] = (unsigned char)img[i];
        for (int i = 0; i < nb_kp; ++i)
            keypoints[nb_kp * 3 + i] = keypoints[nb_kp * 4 + i] = keypoints[nb_kp * 5 + i] =
                (unsigned char)img[nb_kp * 3 + i];
    } else {
        std::fprintf(stderr, "[lut3d gmic] error: incompatible compressed LUT [%u] %s\n", l, image_names[l]._data);
        return 3;
    }
    return 0;
}
