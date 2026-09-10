// SPDX-License-Identifier: GPL-3.0-or-later
#pragma once
// Displayed value * scale + offset = native parameter. Actions are GTK paths.
// Registry ranges are darktable hard limits; values remain editable at both ends.
typedef struct {
  const char *id, *label, *module, *parameter;
  float minimum, maximum, step, initial, scale, offset;
  const char *unit;
  int decimals;
  const char *group, *section, *action, *colors;
  int detail;
  float soft_minimum, soft_maximum;
} OmControl;
static const OmControl om_controls[] = {
  {"exposure", "exposure", "exposure", "exposure", -18, 18, 0.01, 0, 1, 0, " EV", 3, "Basics", "exposure", "exposure", "light", 0, -3, 4},
  {"brightness", "brightness", "colisa", "brightness", -1, 1, 0.01, 0, 1, 0, "", 2, "Basics", "contrast brightness saturation", "brightness", "light", 0},
  {"contrast", "contrast", "colisa", "contrast", -1, 1, 0.01, 0, 1, 0, "", 2, "Basics", "contrast brightness saturation", "contrast", "", 0},
  {"detail", "detail", "bilat", "detail", -100, 400, 1, 25, 0.01, 0, "%", 0, "Basics", "local contrast", "detail", "", 0},
  {"shadows", "shadows", "shadhi", "shadows", -100, 100, 0.1, 50, 1, 0, "", 2, "Basics", "shadows and highlights", "shadows", "light", 0},
  {"highlights", "highlights", "shadhi", "highlights", -100, 100, 0.1, -50, 1, 0, "", 2, "Basics", "shadows and highlights", "highlights", "light", 0},
  {"whitepoint", "white point adjustment", "shadhi", "whitepoint", -10, 10, 0.1, 0, 1, 0, "", 2, "Basics", "shadows and highlights", "white point adjustment", "light", 0},
  {"black", "black level correction", "exposure", "black", -1, 1, 0.0001, 0, 1, 0, "", 4, "Basics", "exposure", "black level correction", "light", 0, -0.1, 0.1},
  {"saturation", "saturation", "colisa", "saturation", -1, 1, 0.01, 0, 1, 0, "", 2, "Color", "contrast brightness saturation", "saturation", "saturation", 0},
  {"vibrance", "global vibrance", "colorbalancergb", "vibrance", -100, 100, 0.1, 0, 0.01, 0, "%", 2, "Color", "color balance rgb", "global vibrance", "saturation", 0, -50, 50},
  {"highlights_chroma", "chroma", "colorbalancergb", "highlights_C", 0, 100, 1, 0, 0.01, 0, "%", 2, "Color", "color balance rgb \u00b7 highlights gain", "gain/chroma", "saturation", 0},
  {"highlights_hue", "hue", "colorbalancergb", "highlights_H", 0, 360, 1, 0, 1, 0, "\u00b0", 2, "Color", "color balance rgb \u00b7 highlights gain", "gain/hue", "hue", 0},
  {"shadows_chroma", "chroma", "colorbalancergb", "shadows_C", 0, 100, 1, 0, 0.01, 0, "%", 2, "Color", "color balance rgb \u00b7 shadows lift", "lift/chroma", "saturation", 0},
  {"shadows_hue", "hue", "colorbalancergb", "shadows_H", 0, 360, 1, 0, 1, 0, "\u00b0", 2, "Color", "color balance rgb \u00b7 shadows lift", "lift/hue", "hue", 0},
  {"offset", "luminance", "colorbalancergb", "global_Y", -100, 100, 1, 0, 0.01, 0, "%", 2, "Color", "color balance rgb \u00b7 global offset", "offset/luminance", "light", 0, -5, 5},
  {"bloom_strength", "strength", "bloom", "strength", 0, 100, 1, 25, 1, 0, "%", 2, "Effects", "bloom", "strength", "", 0},
  {"bloom_size", "size", "bloom", "size", 0, 100, 1, 20, 1, 0, "%", 2, "Effects", "bloom", "size", "", 1},
  {"bloom_threshold", "threshold", "bloom", "threshold", 0, 100, 1, 90, 1, 0, "%", 2, "Effects", "bloom", "threshold", "", 1},
  {"grain", "strength", "grain", "strength", 0, 100, 1, 25, 1, 0, "%", 2, "Effects", "grain", "strength", "", 0},
  {"grain_size", "coarseness", "grain", "scale", 20, 6400, 20, 1600, 0.004690431519699813, 0, " ISO", 0, "Effects", "grain", "coarseness", "", 1},
  {"grain_midtones", "mid-tones bias", "grain", "midtones_bias", 0, 100, 1, 100, 1, 0, "%", 2, "Effects", "grain", "mid-tones bias", "", 1},
  {"vignette", "brightness", "vignette", "brightness", -1, 1, 0.01, -0.5, 1, 0, "", 3, "Effects", "vignetting", "brightness", "light", 0},
  {"vignette_scale", "fall-off start", "vignette", "scale", 0, 200, 1, 80, 1, 0, "%", 2, "Effects", "vignetting", "fall-off start", "", 1},
  {"vignette_falloff_scale", "fall-off radius", "vignette", "falloff_scale", 0, 200, 1, 50, 1, 0, "%", 2, "Effects", "vignetting", "fall-off radius", "", 1},
  {"sharpen_amount", "amount", "sharpen", "amount", 0, 2, 0.01, 0.5, 1, 0, "", 3, "Effects", "sharpen", "amount", "", 0},
  {"sharpen_radius", "radius", "sharpen", "radius", 0, 99, 0.01, 2, 1, 0, "", 3, "Effects", "sharpen", "radius", "", 1, 0, 8},
  {"sharpen_threshold", "threshold", "sharpen", "threshold", 0, 100, 0.01, 0.5, 1, 0, "", 3, "Effects", "sharpen", "threshold", "", 1},
  {"denoise_strength", "strength", "denoiseprofile", "strength", 0.001, 1000, 0.01, 1, 1, 0, "", 3, "Denoise", "denoise (profiled)", "strength", "", 0, 0.001, 4},
  {"lut_opacity", "opacity", "lut3d", "@opacity", 0, 100, 1, 100, 1, 0, "%", 0, "Effects", "LUT 3D", "@recipe", "", 0},
  {"temperature", "temperature", "temperature", "@temperature", 1901, 25000, 50, 5000, 1, 0, " K", 0, "Color", "white balance", "temperature", "temperature", 0},
  {"tint", "tint", "temperature", "@tint", .135, 2.326, .001, 1, 1, 0, "", 3, "Color", "white balance", "tint", "tint", 0},
  {"rotation", "rotation", "ashift", "rotation", -180, 180, .01, 0, -1, 0, "°", 2, "Geometry", "rotate and perspective", "rotation", "", 0, -10, 10},

  {"exposure_enabled", "enable", "exposure", "@enabled", 0, 1, 1, 0, 1, 0, "", 0, "System", "exposure", "@enabled", "", 0},
  {"colisa_enabled", "enable", "colisa", "@enabled", 0, 1, 1, 0, 1, 0, "", 0, "System", "colisa", "@enabled", "", 0},
  {"bilat_enabled", "enable", "bilat", "@enabled", 0, 1, 1, 0, 1, 0, "", 0, "System", "bilat", "@enabled", "", 0},
  {"shadhi_enabled", "enable", "shadhi", "@enabled", 0, 1, 1, 0, 1, 0, "", 0, "System", "shadhi", "@enabled", "", 0},
  {"colorbalancergb_enabled", "enable", "colorbalancergb", "@enabled", 0, 1, 1, 0, 1, 0, "", 0, "System", "colorbalancergb", "@enabled", "", 0},
  {"bloom_enabled", "enable", "bloom", "@enabled", 0, 1, 1, 0, 1, 0, "", 0, "System", "bloom", "@enabled", "", 0},
  {"grain_enabled", "enable", "grain", "@enabled", 0, 1, 1, 0, 1, 0, "", 0, "System", "grain", "@enabled", "", 0},
  {"vignette_enabled", "enable", "vignette", "@enabled", 0, 1, 1, 0, 1, 0, "", 0, "System", "vignette", "@enabled", "", 0},
  {"sharpen_enabled", "enable", "sharpen", "@enabled", 0, 1, 1, 0, 1, 0, "", 0, "System", "sharpen", "@enabled", "", 0},
  {"denoiseprofile_enabled", "enable", "denoiseprofile", "@enabled", 0, 1, 1, 0, 1, 0, "", 0, "System", "denoiseprofile", "@enabled", "", 0},
  {"lut3d_enabled", "enable", "lut3d", "@enabled", 0, 1, 1, 0, 1, 0, "", 0, "System", "lut3d", "@enabled", "", 0},
  {"temperature_enabled", "enable", "temperature", "@enabled", 0, 1, 1, 0, 1, 0, "", 0, "System", "temperature", "@enabled", "", 0},
  {"ashift_enabled", "enable", "ashift", "@enabled", 0, 1, 1, 0, 1, 0, "", 0, "System", "ashift", "@enabled", "", 0},

  {"crop_left", "left", "crop", "cx", 0, 100, .01, 0, 0.01, 0, "%", 2, "Geometry", "crop", "left", "", 0},
  {"crop_top", "top", "crop", "cy", 0, 100, .01, 0, 0.01, 0, "%", 2, "Geometry", "crop", "top", "", 0},
  {"crop_right", "right", "crop", "cw", 0, 100, .01, 0, -0.01, 1, "%", 2, "Geometry", "crop", "right", "", 0},
  {"crop_bottom", "bottom", "crop", "ch", 0, 100, .01, 0, -0.01, 1, "%", 2, "Geometry", "crop", "bottom", "", 0},
  {"crop_enabled", "enable", "crop", "@enabled", 0, 1, 1, 0, 1, 0, "", 0, "System", "crop", "@enabled", "", 0},

  {"denoise_4_0", "Y0", "denoiseprofile", "@curve28", 0, 1, .01, .5, 1, 0, "", 2, "Curve", "denoise (profiled)", "@recipe", "", 0},
  {"denoise_4_1", "Y0", "denoiseprofile", "@curve29", 0, 1, .01, .5, 1, 0, "", 2, "Curve", "denoise (profiled)", "@recipe", "", 0},
  {"denoise_4_2", "Y0", "denoiseprofile", "@curve30", 0, 1, .01, .5, 1, 0, "", 2, "Curve", "denoise (profiled)", "@recipe", "", 0},
  {"denoise_4_3", "Y0", "denoiseprofile", "@curve31", 0, 1, .01, .5, 1, 0, "", 2, "Curve", "denoise (profiled)", "@recipe", "", 0},
  {"denoise_4_4", "Y0", "denoiseprofile", "@curve32", 0, 1, .01, .5, 1, 0, "", 2, "Curve", "denoise (profiled)", "@recipe", "", 0},
  {"denoise_4_5", "Y0", "denoiseprofile", "@curve33", 0, 1, .01, .5, 1, 0, "", 2, "Curve", "denoise (profiled)", "@recipe", "", 0},
  {"denoise_4_6", "Y0", "denoiseprofile", "@curve34", 0, 1, .01, .5, 1, 0, "", 2, "Curve", "denoise (profiled)", "@recipe", "", 0},
  {"denoise_5_0", "U0V0", "denoiseprofile", "@curve35", 0, 1, .01, .5, 1, 0, "", 2, "Curve", "denoise (profiled)", "@recipe", "", 0},
  {"denoise_5_1", "U0V0", "denoiseprofile", "@curve36", 0, 1, .01, .5, 1, 0, "", 2, "Curve", "denoise (profiled)", "@recipe", "", 0},
  {"denoise_5_2", "U0V0", "denoiseprofile", "@curve37", 0, 1, .01, .5, 1, 0, "", 2, "Curve", "denoise (profiled)", "@recipe", "", 0},
  {"denoise_5_3", "U0V0", "denoiseprofile", "@curve38", 0, 1, .01, .5, 1, 0, "", 2, "Curve", "denoise (profiled)", "@recipe", "", 0},
  {"denoise_5_4", "U0V0", "denoiseprofile", "@curve39", 0, 1, .01, .5, 1, 0, "", 2, "Curve", "denoise (profiled)", "@recipe", "", 0},
  {"denoise_5_5", "U0V0", "denoiseprofile", "@curve40", 0, 1, .01, .5, 1, 0, "", 2, "Curve", "denoise (profiled)", "@recipe", "", 0},
  {"denoise_5_6", "U0V0", "denoiseprofile", "@curve41", 0, 1, .01, .5, 1, 0, "", 2, "Curve", "denoise (profiled)", "@recipe", "", 0},
  {"denoise_mode", "mode", "denoiseprofile", "@int:mode", 0, 4, 1, 1, 1, 0, "", 0, "Curve", "denoise (profiled)", "@recipe", "", 0},
  {"denoise_color_mode", "color mode", "denoiseprofile", "@int:wavelet_color_mode", 0, 1, 1, 1, 1, 0, "", 0, "Curve", "denoise (profiled)", "@recipe", "", 0},
  {"diffuse_opacity", "opacity", "diffuse", "@opacity", 0, 100, 1, 100, 1, 0, "%", 0, "Effects", "diffuse or sharpen", "@recipe", "", 0},
  {"diffuse_radius", "radius span", "diffuse", "@int:radius", 0, 2048, 1, 8, 1, 0, " px", 0, "Effects", "diffuse or sharpen", "@recipe", "", 1, 1, 512},
  {"diffuse_enabled", "enable", "diffuse", "@enabled", 0, 1, 1, 0, 1, 0, "", 0, "System", "diffuse", "@enabled", "", 0},

};
#define OM_CONTROL_COUNT (sizeof(om_controls) / sizeof(om_controls[0]))
static inline float om_parameter_value(unsigned int index, float value) {
  return value * om_controls[index].scale + om_controls[index].offset;
}
