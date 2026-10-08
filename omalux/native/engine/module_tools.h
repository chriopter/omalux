// SPDX-License-Identifier: GPL-3.0-or-later
#pragma once
#include "engine.h"
#ifdef __cplusplus
extern "C" {
#endif
// darktable's module tools without its GUI: colour pickers over an area or a point of the
// displayed image, the buttons that compute parameters (auto, flip, apply ...) and the
// histogram some modules draw behind their controls. The pixels come from the module's own
// input, picked in the colour space darktable picks in for that module.
//
// request is a JSON object:
//   {"tool": "<id>", "box": [x0, y0, x1, y1], "gui": {"<gui-only row>": value, ...}}
// box is normalised to the displayed (fully processed) image; a point is a box of zero size.
// *result receives a JSON object (free with om_engine_free_json):
//   {"tool": id, "changed": ["operation", ...], "picked": {"mean": [...], "min": [...],
//    "max": [...], "output": [...]}, "gui": {...}, "histogram": {...}, "message": "..."}
// "changed" lists every module whose parameters the tool wrote (one history item each).
// Returns 0, or 1 no image, 2 no such module, 3 unknown tool, 4 the module input could not be
// rendered, 5 bad request, 6 the picked area holds no pixels.
int om_engine_module_tool(OmEngine *engine, const char *operation, int instance, const char *request,
                          char **result);
// The tools this adapter implements, as a JSON array of {"operation", "tool"}.
char *om_engine_module_tool_list(void);
#ifdef __cplusplus
}
#endif
