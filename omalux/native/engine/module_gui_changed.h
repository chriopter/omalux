// SPDX-License-Identifier: GPL-3.0-or-later
// darktable's module GUI rules that change parameters, without GTK: the parts of a module's
// gui_changed that keep its parameters consistent after a slider edit, and the parts of
// reload_defaults that read GUI state. Never included by Qt code.
#pragma once
#include "engine_internal.h"

// After a generic edit of module (before: its parameters before the edit).
void om_module_gui_changed(dt_iop_module_t *module, const void *before);
// After an image was opened (only NULL) or one module was reset (only that module).
void om_module_defaults_loaded(dt_develop_t *dev, dt_iop_module_t *only);

// The ports these dispatch to (tools_*.c).
void om_tools_colormapping_defaults(dt_develop_t *dev, dt_iop_module_t *only);
void om_tools_colormapping_clusters_changed(dt_iop_module_t *module);
