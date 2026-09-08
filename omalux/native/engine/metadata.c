// SPDX-License-Identifier: GPL-3.0-or-later
#include "engine_internal.h"
char *om_engine_metadata(OmEngine *engine) {
    JsonObject *object = json_object_new();
    dt_image_t *image = &engine->dev.image_storage;
    json_object_set_string_member(object, "camera", image->camera_makermodel);
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
