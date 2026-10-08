// SPDX-License-Identifier: GPL-3.0-or-later
// rotate and perspective's fit and automatic crop (ashift_fit.c). Never included by Qt code.
#pragma once
#include "engine_internal.h"

// darktable's gui_changed: refit the automatic crop of an ashift instance after its parameters
// changed (rotation, lens shift, shear, focal length, automatic cropping ...).
void om_ashift_autocrop(dt_develop_t *dev, dt_iop_module_t *module);
