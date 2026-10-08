// SPDX-License-Identifier: GPL-3.0-or-later
// The mask previews darktable draws from inside a module (module_display.c).
// Never included by Qt code.
#pragma once
#include "engine_internal.h"

// Create the dt_conf keys of the previews as the modules' gui_init does (once, at startup,
// before any processing reads them).
void om_module_display_init(void);
// Forget the displayed module (a new image was opened, or the blend section's mask is shown).
void om_module_display_reset(OmEngine *engine);
// Around om_engine_render: wrap the displayed module for the full pipe, restore afterwards.
void om_module_display_before_render(OmEngine *engine);
void om_module_display_after_render(OmEngine *engine);
