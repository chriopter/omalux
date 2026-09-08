// SPDX-License-Identifier: GPL-3.0-or-later
#pragma once

// Float parameters: UI value * scale + offset = darktable parameter value.
// Add a row here to expose a slider in Qt, the engine and the split bridge.
typedef struct {
  const char *id, *label, *module, *parameter;
  float minimum, maximum, step, initial, scale, offset;
} OmControl;

static const OmControl om_controls[] = {
  {"brightness", "BRIGHTNESS", "colisa", "brightness", -100, 100, 1, 0, .01f, 0},
  {"contrast", "CONTRAST", "colisa", "contrast", -100, 100, 1, 0, .01f, 0},
  {"saturation", "SATURATION", "colisa", "saturation", -100, 100, 1, 0, .01f, 0},
};
#define OM_CONTROL_COUNT (sizeof(om_controls) / sizeof(om_controls[0]))

static inline float om_parameter_value(unsigned int index, float value) {
  return value * om_controls[index].scale + om_controls[index].offset;
}
