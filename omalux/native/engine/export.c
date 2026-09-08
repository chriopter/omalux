// SPDX-License-Identifier: GPL-3.0-or-later
#include "engine_internal.h"
#include "imageio/imageio_common.h"
#include "imageio/imageio_module.h"
int om_engine_export(OmEngine *engine, const char *filename, const char *format_name, int quality) {
    if (!engine->loaded)
        return 1;
    dt_imageio_module_format_t *format = dt_imageio_get_format_by_name(format_name);
    if (!format)
        return 2;
    dt_imageio_module_data_t *params = format->get_params(format);
    if (!params)
        return 3;
    params->max_width = 0;
    params->max_height = 0;
    params->style[0] = 0;
    // jpeg's stable leading fields: global data followed by integer quality.
    if (!strcmp(format_name, "jpeg"))
        *(int *)((char *)params + sizeof(*params)) = CLAMP(quality, 1, 100);
    dt_dev_write_history_ext(&engine->dev, engine->dev.image_storage.id);
    int result = dt_imageio_export(engine->dev.image_storage.id, filename, format, params, TRUE, FALSE, FALSE,
                                   1.0, TRUE, FALSE, DT_COLORSPACE_SRGB, NULL, DT_INTENT_PERCEPTUAL, NULL,
                                   NULL, 1, 1, NULL);
    format->free_params(format, params);
    return result;
}
