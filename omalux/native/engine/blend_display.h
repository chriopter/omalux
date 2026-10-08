// SPDX-License-Identifier: GPL-3.0-or-later
// The blend section's display mask and switch-off-mask view state (blend_display.c).
// Never included by Qt code.
#pragma once
#include "engine_internal.h"

// Forget the displayed module (a new image was opened).
void om_blend_display_reset(OmEngine *engine);
// Around om_engine_render: suppress the mask while rendering, compose the mask overlay after.
// Returns the pixels to show (pixels itself, or the composed overlay owned by this file).
void om_blend_display_before_render(OmEngine *engine);
const unsigned char *om_blend_display_after_render(OmEngine *engine, const unsigned char *pixels, int width,
                                                    int height);
