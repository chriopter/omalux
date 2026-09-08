// SPDX-License-Identifier: GPL-3.0-or-later
#pragma once
#include "develop/imageop.h"
gboolean om_wb_read(dt_iop_module_t *module, const void *params, float *temperature, float *tint);
gboolean om_wb_write(dt_iop_module_t *module, float temperature, float tint);
