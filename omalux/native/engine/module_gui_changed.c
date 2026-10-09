// SPDX-License-Identifier: GPL-3.0-or-later
// darktable's module GUI rules that change parameters (see module_gui_changed.h).
#include "module_gui_changed.h"
#include "lut3d_gmz.h"
#include "ashift_fit.h"

static gboolean field_changed(dt_iop_module_t *module, const void *before, const char *name) {
    const dt_introspection_field_t *f = module->get_f ? module->get_f(name) : NULL;
    if (!f)
        return FALSE;
    return memcmp((const char *)before + f->header.offset, (const char *)module->params + f->header.offset,
                  f->header.size) != 0;
}

void om_module_gui_changed(dt_iop_module_t *module, const void *before) {
    if (!before)
        return;
    // colormapping.c gui_changed (742): a new number of clusters resets source and target.
    if (dt_iop_module_is(module, "colormapping") && field_changed(module, before, "n"))
        om_tools_colormapping_clusters_changed(module);
    // ashift.c gui_changed (5273): the automatic crop is refitted after a slider or combobox
    // change (the crop box itself and the stored structure lines are not widgets).
    if (dt_iop_module_is(module, "ashift")) {
        static const char *const widgets[] = {"rotation", "lensshift_v", "lensshift_h", "shear", "f_length",
                                              "crop_factor", "orthocorr", "aspect", "mode", "cropmode"};
        gboolean changed = FALSE;
        for (size_t i = 0; i < G_N_ELEMENTS(widgets); i++)
            changed = changed || field_changed(module, before, widgets[i]);
        if (changed)
            om_ashift_autocrop(module->dev, module);
    }
    // lens.cc: until the user changes something, commit_params (3260) ignores the parameters and
    // corrects with the defaults detected for the image; gui_update (4629) shows those defaults
    // and gui_changed (4326) then marks the parameters as set. Without this an edit here (the
    // manual vignette, a fine-tuning slider, the corrections list) had no effect: start from the
    // detected defaults, apply what the edit changed and mark the parameters as set.
    if (dt_iop_module_is(module, "lens")) {
        gboolean *set = module->get_p ? module->get_p(module->params, "has_been_set") : NULL;
        int *method = module->get_p ? module->get_p(module->params, "method") : NULL;
        if (set && method && !*set && module->default_params &&
            memcmp(before, module->params, module->params_size) != 0) {
            const int chosen = *method;
            unsigned char *after = g_memdup2(module->params, module->params_size);
            memcpy(module->params, module->default_params, module->params_size);
            for (int i = 0; i < module->params_size; ++i)
                if (after[i] != ((const unsigned char *)before)[i])
                    ((unsigned char *)module->params)[i] = after[i];
            g_free(after);
            *method = chosen;
            *set = TRUE;
        }
    }
    // lut3d.c _filepath_callback / _lutname_callback: a .gmz file is read into the parameters.
    if (dt_iop_module_is(module, "lut3d"))
        om_lut3d_gmz_changed(module, before);
}

void om_module_defaults_loaded(dt_develop_t *dev, dt_iop_module_t *only) {
    // colormapping.c reload_defaults (815): the source acquired on an earlier image.
    om_tools_colormapping_defaults(dev, only);
}
