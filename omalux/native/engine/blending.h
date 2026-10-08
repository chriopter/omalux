// SPDX-License-Identifier: GPL-3.0-or-later
#pragma once
// A module's blend section (darktable's develop/blend_gui.c) without its GTK widgets.
// Generic edits address it with paths starting with "blend." next to the params paths; the
// path list is documented in development/docs/reference/controls.md ("Blending").
#include "engine_internal.h"

typedef struct {
    dt_develop_blend_params_t params; // working copy, written by om_blend_commit
    gboolean touched;
    // darktable shows the output channels of a parametric mask once they are used or the
    // user asked for them; while hidden, some edits reset them (_blendif_clean_output_channels).
    gboolean outputs_shown;
    gboolean reprocess;
} OmBlendEdit;

gboolean om_blend_path(const char *path);
void om_blend_begin(OmBlendEdit *edit, dt_iop_module_t *module);
// Apply one "blend.*" assignment to the working copy with darktable's GUI rules. Returns 0, or
// 3 unknown path, 4 not supported by this module, 5 invalid value.
int om_blend_assign(OmBlendEdit *edit, dt_iop_module_t *module, const char *path, double value);
void om_blend_commit(dt_iop_module_t *module, OmBlendEdit *edit);
// The module's blend state and what its blend section offers, as the catalog's "blend" member.
void om_blend_describe(JsonObject *entry, dt_iop_module_t *module);
