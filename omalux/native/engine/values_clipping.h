// SPDX-License-Identifier: GPL-3.0-or-later
// The deprecated crop and rotate module's displayed values @flip and @aspect (values_clipping.c).
// Never included by Qt code.
#pragma once
#include "engine_internal.h"
#include <json-glib/json-glib.h>

void om_values_clipping_read(dt_iop_module_t *module, JsonObject *out);
int om_values_clipping_write(dt_iop_module_t *module, size_t count, const char *const *paths, const double *values);
