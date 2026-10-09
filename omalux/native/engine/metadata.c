// SPDX-License-Identifier: GPL-3.0-or-later
#include "engine_internal.h"
// GNU C predefines `unix`, which datetime.h uses as a parameter name.
#undef unix
#include "common/datetime.h"
#include <glib/gstdio.h>
// darktable composes the name from maker and model with printf, so a file without them reads
// "(null) (null)"; show it as missing instead.
static char *camera_name(const char *makermodel) {
    gchar **words = g_strsplit(makermodel ? makermodel : "", " ", -1);
    GString *name = g_string_new(NULL);
    for (gchar **word = words; *word; ++word)
        if (**word && strcmp(*word, "(null)")) {
            if (name->len)
                g_string_append_c(name, ' ');
            g_string_append(name, *word);
        }
    g_strfreev(words);
    return g_string_free(name, FALSE);
}
char *om_engine_metadata(OmEngine *engine) {
    JsonObject *object = json_object_new();
    dt_image_t *image = &engine->dev.image_storage;
    char *camera = camera_name(image->camera_makermodel);
    json_object_set_string_member(object, "camera", camera);
    g_free(camera);
    json_object_set_string_member(object, "lens", image->exif_lens);
    json_object_set_int_member(object, "width", image->width);
    json_object_set_int_member(object, "height", image->height);
    json_object_set_double_member(object, "ISO", isfinite(image->exif_iso) ? image->exif_iso : 0);
    json_object_set_double_member(object, "aperture",
                                  isfinite(image->exif_aperture) ? image->exif_aperture : 0);
    json_object_set_double_member(object, "exposure seconds",
                                  isfinite(image->exif_exposure) ? image->exif_exposure : 0);
    json_object_set_double_member(object, "focal length mm",
                                  isfinite(image->exif_focal_length) ? image->exif_focal_length : 0);
    // What the Info pane shows besides the exposure: file, date, and the EXIF words darktable
    // read. Numbers stay raw (0 or absent means unknown); the pane formats them.
    json_object_set_string_member(object, "maker", image->exif_maker);
    json_object_set_string_member(object, "model", image->exif_model);
    char taken[DT_DATETIME_EXIF_LENGTH] = "";
    if (image->exif_datetime_taken)
        dt_datetime_img_to_exif(taken, sizeof(taken), image);
    json_object_set_string_member(object, "datetime", taken);
    json_object_set_double_member(object, "exposure bias",
                                  isfinite(image->exif_exposure_bias) ? image->exif_exposure_bias : 0);
    json_object_set_boolean_member(object, "exposure bias known", isfinite(image->exif_exposure_bias));
    json_object_set_double_member(object, "focus distance",
                                  isfinite(image->exif_focus_distance) ? image->exif_focus_distance : 0);
    json_object_set_double_member(object, "crop factor", isfinite(image->exif_crop) ? image->exif_crop : 0);
    json_object_set_string_member(object, "exposure program", image->exif_exposure_program);
    json_object_set_string_member(object, "metering", image->exif_metering_mode);
    json_object_set_string_member(object, "flash", image->exif_flash);
    json_object_set_string_member(object, "white balance", image->exif_whitebalance);
    json_object_set_boolean_member(object, "raw", (image->flags & DT_IMAGE_RAW) != 0);
    json_object_set_boolean_member(object, "hdr", (image->flags & DT_IMAGE_HDR) != 0);
    json_object_set_boolean_member(object, "monochrome", (image->flags & DT_IMAGE_MONOCHROME) != 0);
    if (isfinite(image->geoloc.latitude) && isfinite(image->geoloc.longitude)) {
        json_object_set_double_member(object, "latitude", image->geoloc.latitude);
        json_object_set_double_member(object, "longitude", image->geoloc.longitude);
        if (isfinite(image->geoloc.elevation))
            json_object_set_double_member(object, "elevation", image->geoloc.elevation);
    }
    char path[PATH_MAX] = "";
    gboolean from_cache = FALSE;
    dt_image_full_path(image->id, path, sizeof(path), &from_cache);
    json_object_set_string_member(object, "path", path);
    GStatBuf file;
    if (*path && !g_stat(path, &file))
        json_object_set_int_member(object, "bytes", file.st_size);
    JsonNode *node = json_node_new(JSON_NODE_OBJECT);
    json_node_take_object(node, object);
    char *result = json_to_string(node, FALSE);
    json_node_free(node);
    return result;
}
