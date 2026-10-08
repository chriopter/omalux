// SPDX-License-Identifier: GPL-3.0-or-later
#include "engine_internal.h"
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
    JsonNode *node = json_node_new(JSON_NODE_OBJECT);
    json_node_take_object(node, object);
    char *result = json_to_string(node, FALSE);
    json_node_free(node);
    return result;
}
