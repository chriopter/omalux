// SPDX-License-Identifier: GPL-3.0-or-later
#pragma once
// darktable's multi-instance menu (develop/imageop.c, _gui_multiinstance_callback) for one
// module instance, addressed by operation and multi_priority like the generic edits.
#include "engine.h"
#ifdef __cplusplus
extern "C" {
#endif
// action: "new" (new instance), "duplicate" (duplicate instance), "up" (move up: later in the
// pipe), "down" (move down: earlier), "delete", "rename" (name; empty restores the automatic
// name). On success returns 0 and stores the multi_priority of the instance the action leaves
// in view (the new one, the moved or renamed one, or the one that took the deleted one's
// place) in *result. Errors: 1 no image, 2 no such module, 3 unknown action, 4 not allowed for
// this module or position, 5 invalid name.
int om_engine_module_instance(OmEngine *engine, const char *operation, int instance, const char *action,
                              const char *name, int *result);
#ifdef __cplusplus
}
#endif
