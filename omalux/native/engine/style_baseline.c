// SPDX-License-Identifier: GPL-3.0-or-later
#include "engine_internal.h"

// Preserve the image's loaded camera/workflow state, not UI registry defaults.
// Module pointers stay owned by this develop context; clear before its cleanup.
void om_style_baseline_clear(OmEngine *engine) {
    for (GList *it = engine->style_baseline; it; it = it->next) {
        OmStyleBaseline *state = it->data;
        g_free(state->params);
        g_free(state);
    }
    g_list_free(engine->style_baseline);
    engine->style_baseline = NULL;
}

void om_style_baseline_capture(OmEngine *engine) {
    om_style_baseline_clear(engine);
    for (GList *it = engine->dev.iop; it; it = it->next) {
        dt_iop_module_t *module = it->data;
        OmStyleBaseline *state = g_new0(OmStyleBaseline, 1);
        state->module = module;
        state->params = g_memdup2(module->params, module->params_size);
        state->blend = *module->blend_params;
        state->enabled = module->enabled;
        engine->style_baseline = g_list_prepend(engine->style_baseline, state);
    }
}

void om_style_baseline_restore(OmEngine *engine) {
    for (GList *it = engine->dev.iop; it; it = it->next) {
        dt_iop_module_t *module = it->data;
        OmStyleBaseline *state = NULL;
        for (GList *saved = engine->style_baseline; saved; saved = saved->next)
            if (((OmStyleBaseline *)saved->data)->module == module) {
                state = saved->data;
                break;
            }
        const void *params = state ? state->params : module->default_params;
        const dt_develop_blend_params_t *blend = state ? &state->blend : module->default_blendop_params;
        const gboolean enabled = state ? state->enabled : FALSE;
        if (module->enabled == enabled && !memcmp(module->params, params, module->params_size) &&
            !memcmp(module->blend_params, blend, sizeof(*blend)))
            continue;
        memcpy(module->params, params, module->params_size);
        dt_iop_commit_blend_params(module, blend);
        module->enabled = enabled;
        // Record the reset so older edits remain reachable in the history pane.
        dt_dev_add_history_item_ext(&engine->dev, module, FALSE, FALSE);
    }
}
