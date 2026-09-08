// SPDX-License-Identifier: GPL-3.0-or-later
#include "engine_internal.h"
void om_preview_geometry(const dt_dev_pixelpipe_t *pipe, OmPreviewGeometry *geometry) {
    const double scale = pipe->backbuf_scale;
    geometry->aspect_ratio = (double)pipe->processed_width / MAX(1, pipe->processed_height);
    // darktable samples source coordinate x / scale (integer pixel centers).
    // Qt texture centers are (x + .5) / raster size. Preserve the fractional
    // processed extent instead of stretching truncated rows to the whole image.
    geometry->scale_x = scale * pipe->processed_width / MAX(1, pipe->backbuf_width);
    geometry->scale_y = scale * pipe->processed_height / MAX(1, pipe->backbuf_height);
    geometry->offset_x = (.5 - .5 * scale) / MAX(1, pipe->backbuf_width);
    geometry->offset_y = (.5 - .5 * scale) / MAX(1, pipe->backbuf_height);
}
