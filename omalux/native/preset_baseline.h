// SPDX-License-Identifier: GPL-3.0-or-later
#pragma once

// Preserve the image's loaded camera/workflow state, not UI registry defaults.
// Module pointers stay owned by this develop context; clear before its cleanup.
typedef struct {
  dt_iop_module_t *module;
  void *params;
  dt_develop_blend_params_t blend;
  gboolean enabled;
} OmPresetBaseline;
static GList *preset_baseline;

static void om_preset_baseline_clear(void) {
  for(GList *it=preset_baseline; it; it=it->next) {
    OmPresetBaseline *state=it->data;
    g_free(state->params);
    g_free(state);
  }
  g_list_free(preset_baseline);
  preset_baseline=NULL;
}

static void om_preset_baseline_capture(void) {
  om_preset_baseline_clear();
  for(GList *it=dev.iop; it; it=it->next) {
    dt_iop_module_t *module=it->data;
    OmPresetBaseline *state=g_new0(OmPresetBaseline,1);
    state->module=module;
    state->params=g_memdup2(module->params,module->params_size);
    state->blend=*module->blend_params;
    state->enabled=module->enabled;
    preset_baseline=g_list_prepend(preset_baseline,state);
  }
}

static void om_preset_baseline_restore(void) {
  for(GList *it=dev.iop; it; it=it->next) {
    dt_iop_module_t *module=it->data;
    OmPresetBaseline *state=NULL;
    for(GList *saved=preset_baseline;saved;saved=saved->next)
      if(((OmPresetBaseline *)saved->data)->module==module) {state=saved->data;break;}
    const void *params=state ? state->params : module->default_params;
    const dt_develop_blend_params_t *blend=state ? &state->blend : module->default_blendop_params;
    const gboolean enabled=state ? state->enabled : FALSE;
    if(module->enabled==enabled && !memcmp(module->params,params,module->params_size)
       && !memcmp(module->blend_params,blend,sizeof(*blend))) continue;
    memcpy(module->params,params,module->params_size);
    dt_iop_commit_blend_params(module,blend);
    module->enabled=enabled;
    // Record the reset so older edits remain reachable in the history pane.
    dt_dev_add_history_item_ext(&dev,module,FALSE,FALSE);
  }
}
