// SPDX-License-Identifier: GPL-3.0-or-later
#include "engine_internal.h"

// Worker-only scratch develop context. Never apply a hover to the editor's
// modules/history, never write image history to the database or comparison UI.
int om_engine_preview_style(OmEngine *engine, const char *path, const char *name, unsigned char **pixels,
                            int *width, int *height, OmPreviewGeometry *geometry) {
    *pixels = NULL;
    if (!engine->loaded)
        return 1;
    if (!dt_styles_exists(name))
        dt_styles_import_from_file(path);
    GList *items = dt_styles_get_item_list(name, FALSE, -1, TRUE);
    if (!items)
        return 2;
    dt_develop_t scratch;
    dt_dev_init(&scratch, TRUE);
    scratch.gui_attached = FALSE;
    dt_dev_load_image(&scratch, engine->dev.image_storage.id);
    scratch.full.dev = &scratch;
    scratch.full.zoom = DT_ZOOM_FIT;
    scratch.full.ppd = 1.0;
    scratch.full.width = OM_PREVIEW_WIDTH;
    scratch.full.height = OM_PREVIEW_HEIGHT;
    scratch.full.color_assessment = FALSE;
    int result = 3;
    // Match click-to-apply semantics: the image's opening baseline plus style,
    // not the current slider edits. Copy by operation/instance, never pointers.
    for (GList *it = engine->style_baseline; it; it = it->next) {
        const OmStyleBaseline *state = it->data;
        dt_iop_module_t *module =
            dt_iop_get_module_by_op_priority(scratch.iop, state->module->op, state->module->multi_priority);
        if (!module || module->params_size != state->module->params_size)
            goto cleanup;
        if (module->enabled == state->enabled &&
            !memcmp(module->params, state->params, module->params_size) &&
            !memcmp(module->blend_params, &state->blend, sizeof(state->blend)))
            continue;
        memcpy(module->params, state->params, module->params_size);
        dt_iop_commit_blend_params(module, &state->blend);
        module->enabled = state->enabled;
        dt_dev_add_history_item_ext(&scratch, module, FALSE, FALSE);
    }
    for (GList *it = items; it; it = it->next) {
        const dt_style_item_t *item = it->data;
        dt_iop_module_t *module = dt_iop_get_module_by_op_priority(scratch.iop, item->operation, -1);
        if (!module || module->version() != item->module_version || module->params_size != item->params_size)
            goto cleanup;
    }
    GList *used = NULL;
    dt_ioppr_update_for_style_items(&scratch, items, FALSE);
    for (GList *it = items; it; it = it->next)
        dt_styles_apply_style_item(&scratch, it->data, &used, FALSE);
    g_list_free(used);
    scratch.full.pipe->changed |= DT_DEV_PIPE_SYNCH;
    dt_dev_process_image_job(&scratch, &scratch.full, scratch.full.pipe, -1, DT_DEVICE_NONE);
    if (scratch.full.pipe->status == DT_DEV_PIXELPIPE_VALID && scratch.full.pipe->backbuf &&
        scratch.full.pipe->backbuf_width > 0 && scratch.full.pipe->backbuf_height > 0) {
        om_preview_geometry(scratch.full.pipe, geometry);
        *width = scratch.full.pipe->backbuf_width;
        *height = scratch.full.pipe->backbuf_height;
        *pixels = g_memdup2(scratch.full.pipe->backbuf, (size_t)*width * *height * 4);
        result = 0;
    }
cleanup:
    g_list_free_full(items, dt_style_item_free);
    dt_dev_cleanup(&scratch);
    return result;
}
void om_engine_free_preview(unsigned char *pixels) {
    g_free(pixels);
}
