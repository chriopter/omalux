// SPDX-License-Identifier: GPL-3.0-or-later
// Values darktable shows through a conversion in its GUI code: the "@" paths of
// omalux/design/layout.json. darktable keeps only the parameters (an illuminant's x/y,
// colour balance's RGB factors, colour harmonizer's UCS hues, ...) and converts them for its
// sliders in gui_update and back in the slider callbacks. The conversions below are those
// callbacks without GTK, each citing the darktable 5.6.1 lines it ports. Reading adds a
// "derived" object {path: displayed value} to a module's catalog entry; writing converts the
// displayed values back into parameters before module_catalog.c records the history item.
#include "engine_internal.h"
#include "common/colorspaces.h"
#include "common/colorspaces_inline_conversions.h"
#include "common/illuminants.h"
#include "common/wb_presets.h"
#if __has_include("common/color_ryb.h")
#include "common/color_ryb.h"
#define OM_HAVE_RYB 1
#endif
#include <math.h>

// A parameter by its introspection name, NULL when the module does not have it.
static void *param(dt_iop_module_t *module, const char *name) {
    return module->get_p ? module->get_p(module->params, name) : NULL;
}
static float *param_f(dt_iop_module_t *module, const char *name) {
    const dt_introspection_field_t *f = module->get_f ? module->get_f(name) : NULL;
    if (!f)
        return NULL;
    if (f->header.type == DT_INTROSPECTION_TYPE_FLOAT ||
        (f->header.type == DT_INTROSPECTION_TYPE_ARRAY && f->Array.type == DT_INTROSPECTION_TYPE_FLOAT))
        return param(module, name);
    return NULL;
}
static int *param_i(dt_iop_module_t *module, const char *name) {
    const dt_introspection_field_t *f = module->get_f ? module->get_f(name) : NULL;
    if (!f || (f->header.type != DT_INTROSPECTION_TYPE_INT && f->header.type != DT_INTROSPECTION_TYPE_ENUM &&
               f->header.type != DT_INTROSPECTION_TYPE_UINT))
        return NULL;
    return param(module, name);
}
static void put(JsonObject *out, const char *path, double value) {
    if (isfinite(value))
        json_object_set_double_member(out, path, value);
}
// "@name[3]" → index 3; "@name[1][3]" → 1 and 3. Returns how many indices were read.
static int indices(const char *path, const char *name, int *a, int *b) {
    const size_t n = strlen(name);
    if (strncmp(path, name, n))
        return -1;
    const char *c = path + n;
    if (!*c)
        return 0;
    if (sscanf(c, "[%d][%d]", a, b) == 2)
        return 2;
    if (sscanf(c, "[%d]", a) == 1 && !strchr(c + 1, '['))
        return 1;
    return -1;
}

// ---- color calibration: illuminant hue and chroma ------------------------------------------
// The custom illuminant is stored as CIE x, y; darktable shows its hue and chroma in CIE LCh
// (channelmixerrgb.c:3300-3320). Reading: gui_changed, channelmixerrgb.c:4097-4107, with
// Y = 1 (L = 100). Writing: _illum_xy_callback, channelmixerrgb.c:3716-3740, which also
// derives the temperature (xy_to_CCT, below 3000 K CCT_reverse_lookup).
static gboolean illuminant_read(dt_iop_module_t *module, float *hue, float *chroma) {
    const float *x = param_f(module, "x"), *y = param_f(module, "y");
    if (!x || !y)
        return FALSE;
    const dt_aligned_pixel_t xyY = {*x, *y, 1.f, 0.f};
    dt_aligned_pixel_t Lch;
    dt_xyY_to_Lch(xyY, Lch);
    // With zero chroma the hue is meaningless; darktable leaves its slider where it was.
    *hue = Lch[1] > 0 ? rad2degf(Lch[2]) : 0.f;
    *chroma = Lch[1];
    return TRUE;
}
static void channelmixerrgb_read(dt_iop_module_t *module, JsonObject *out) {
    float hue, chroma;
    if (!illuminant_read(module, &hue, &chroma))
        return;
    put(out, "@x", hue);
    put(out, "@y", chroma);
}
// Hue and chroma are written together: a batch may carry both, and with zero chroma the hue
// could not be read back between two single writes.
static int channelmixerrgb_write(dt_iop_module_t *module, size_t count, const char *const *paths,
                                 const double *values) {
    float hue, chroma;
    float *x = param_f(module, "x"), *y = param_f(module, "y"), *t = param_f(module, "temperature");
    if (!illuminant_read(module, &hue, &chroma) || !t)
        return 3;
    for (size_t i = 0; i < count; ++i) {
        if (!strcmp(paths[i], "@x"))
            hue = CLAMP(values[i], 0., 360.);
        else if (!strcmp(paths[i], "@y"))
            chroma = CLAMP(values[i], 0., 300.);
        else
            return 3;
    }
    const dt_aligned_pixel_t Lch = {100.f, chroma, deg2radf(hue), 0.f};
    dt_aligned_pixel_t xyY = {0.f};
    dt_Lch_to_xyY(Lch, xyY);
    *x = xyY[0];
    *y = xyY[1];
    float temperature = xy_to_CCT(*x, *y);
    if (temperature < 3000.f)
        temperature = CCT_reverse_lookup(*x, *y);
    *t = temperature;
    return 0;
}

// ---- color balance: hue and saturation of lift, gamma and gain -----------------------------
// Only the RGB factors are stored (CHANNEL_RED..BLUE = 1..3, range 0..2). set_HSL_sliders,
// colorbalance.c:902-926: hue = h × 360, saturation = s × 100 of rgb2hsl(RGB / 2). The
// HSL_CALLBACK, colorbalance.c:1814-1832 with set_RGB_sliders :882-900, writes
// hsl2rgb(hue / 360, saturation / 100, 0.5) × 2 back.
static const char *const colorbalance_ranges[] = {"lift", "gamma", "gain"};
static void colorbalance_read(dt_iop_module_t *module, JsonObject *out) {
    for (int k = 0; k < 3; ++k) {
        const float *f = param_f(module, colorbalance_ranges[k]);
        if (!f)
            continue;
        const dt_aligned_pixel_t rgb = {f[1] / 2.f, f[2] / 2.f, f[3] / 2.f, 0.f};
        float h, s, l;
        rgb2hsl(rgb, &h, &s, &l);
        char name[32];
        snprintf(name, sizeof(name), "@%s_hue", colorbalance_ranges[k]);
        put(out, name, h * 360.f);
        snprintf(name, sizeof(name), "@%s_saturation", colorbalance_ranges[k]);
        put(out, name, s * 100.f);
    }
}
static int colorbalance_write(dt_iop_module_t *module, size_t count, const char *const *paths,
                              const double *values) {
    // Hue and saturation of one range are written together, as the callback reads both sliders.
    for (int k = 0; k < 3; ++k) {
        char hue_name[32], sat_name[32];
        snprintf(hue_name, sizeof(hue_name), "@%s_hue", colorbalance_ranges[k]);
        snprintf(sat_name, sizeof(sat_name), "@%s_saturation", colorbalance_ranges[k]);
        float *f = param_f(module, colorbalance_ranges[k]);
        if (!f)
            return 3;
        const dt_aligned_pixel_t current = {f[1] / 2.f, f[2] / 2.f, f[3] / 2.f, 0.f};
        float h, s, l;
        rgb2hsl(current, &h, &s, &l);
        gboolean touched = FALSE;
        for (size_t i = 0; i < count; ++i) {
            if (!strcmp(paths[i], hue_name))
                h = CLAMP(values[i], 0., 360.) / 360.f;
            else if (!strcmp(paths[i], sat_name))
                s = CLAMP(values[i], 0., 100.) / 100.f;
            else
                continue;
            touched = TRUE;
        }
        if (!touched)
            continue;
        dt_aligned_pixel_t rgb = {0.f};
        hsl2rgb(rgb, h, s, 0.5f);
        f[1] = rgb[0] * 2.f;
        f[2] = rgb[1] * 2.f;
        f[3] = rgb[2] * 2.f;
    }
    for (size_t i = 0; i < count; ++i)
        if (strncmp(paths[i], "@lift_", 6) && strncmp(paths[i], "@gamma_", 7) &&
            strncmp(paths[i], "@gain_", 6))
            return 3;
    return 0;
}

// ---- color harmonizer: hues on the painter's (RYB) wheel -----------------------------------
// Stored as darktable UCS hues in [0, 1); the sliders show RYB degrees through two 720-entry
// lookup tables built in init_global. Ported from colorharmonizer.c: _find_max_chroma
// :698-719, _hue_to_srgb :723-738, _ucs_hue_to_ryb_hue :802-812, _hue_lerp :513-520, the
// lookups :524-539 and _build_hue_luts :546-563. Display: _ucs_to_ryb_fast(hue) × 360
// (gui_update :1156-1159); writing: _ryb_to_ucs_fast(degrees / 360) (:855, :995).
#ifdef OM_HAVE_RYB
enum { OM_RYB_STEPS = 720 };
static float ucs_to_ryb_lut[OM_RYB_STEPS], ryb_to_ucs_lut[OM_RYB_STEPS];
static gboolean ryb_ready = FALSE;
static float find_max_chroma(const float hue) {
    const float L_white = Y_to_dt_UCS_L_star(1.0f);
    const float H = hue * DT_2PI_F - M_PI_F;
    const float J = 0.65f;
    float C_lo = 0.f, C_hi = 2.f;
    for (int iter = 0; iter < 16; iter++) {
        const float C_mid = (C_lo + C_hi) * 0.5f;
        dt_aligned_pixel_t JCH = {J, C_mid, H, 0.f};
        dt_aligned_pixel_t sRGB;
        dt_UCS_JCH_to_sRGB(JCH, L_white, sRGB);
        if (sRGB[0] >= 0.f && sRGB[1] >= 0.f && sRGB[2] >= 0.f && sRGB[0] <= 1.f && sRGB[1] <= 1.f &&
            sRGB[2] <= 1.f)
            C_lo = C_mid;
        else
            C_hi = C_mid;
    }
    return C_lo;
}
static float ucs_hue_to_ryb_hue(const float ucs_hue) {
    const float L_white = Y_to_dt_UCS_L_star(1.0f);
    dt_aligned_pixel_t JCH = {0.65f, find_max_chroma(ucs_hue) * 0.85f, ucs_hue * DT_2PI_F - M_PI_F, 0.f};
    dt_aligned_pixel_t sRGB;
    dt_UCS_JCH_to_sRGB(JCH, L_white, sRGB);
    const dt_aligned_pixel_t srgb = {CLAMP(sRGB[0], 0.f, 1.f), CLAMP(sRGB[1], 0.f, 1.f),
                                     CLAMP(sRGB[2], 0.f, 1.f), 0.f};
    dt_aligned_pixel_t lrgb, HCV;
    dt_sRGB_to_linear_sRGB(srgb, lrgb);
    dt_RGB_2_HCV(lrgb, HCV);
    return dt_rgb_hue_to_ryb_hue(HCV[0]);
}
static float hue_lerp(float a, float b, const float t) {
    if (b - a > 0.5f)
        b -= 1.0f;
    else if (a - b > 0.5f)
        a -= 1.0f;
    float r = a + t * (b - a);
    if (r < 0.0f)
        r += 1.0f;
    return r;
}
static float ryb_lookup(const float *lut, const float hue) {
    const float pos = hue * OM_RYB_STEPS;
    const int i0 = (int)pos % OM_RYB_STEPS;
    const int i1 = (i0 + 1) % OM_RYB_STEPS;
    return hue_lerp(lut[i0], lut[i1], pos - (int)pos);
}
static void ryb_tables(void) {
    if (ryb_ready)
        return;
    for (int i = 0; i < OM_RYB_STEPS; i++)
        ucs_to_ryb_lut[i] = ucs_hue_to_ryb_hue(i / (float)OM_RYB_STEPS);
    for (int j = 0; j < OM_RYB_STEPS; j++) {
        const float target = j / (float)OM_RYB_STEPS;
        float best_dist = 1.0f, best_ucs = 0.0f;
        for (int i = 0; i < OM_RYB_STEPS; i++) {
            float d = fabsf(ucs_to_ryb_lut[i] - target);
            if (d > 0.5f)
                d = 1.0f - d;
            if (d < best_dist) {
                best_dist = d;
                best_ucs = i / (float)OM_RYB_STEPS;
            }
        }
        ryb_to_ucs_lut[j] = best_ucs;
    }
    ryb_ready = TRUE;
}
static void colorharmonizer_read(dt_iop_module_t *module, JsonObject *out) {
    const float *anchor = param_f(module, "anchor_hue"), *custom = param_f(module, "custom_hue");
    if (!anchor || !custom)
        return;
    ryb_tables();
    put(out, "@anchor_hue", ryb_lookup(ucs_to_ryb_lut, *anchor) * 360.f);
    for (int i = 0; i < 4; ++i) {
        char name[32];
        snprintf(name, sizeof(name), "@custom_hue[%d]", i);
        put(out, name, ryb_lookup(ucs_to_ryb_lut, custom[i]) * 360.f);
    }
}
static int colorharmonizer_write(dt_iop_module_t *module, const char *path, double value) {
    float *anchor = param_f(module, "anchor_hue"), *custom = param_f(module, "custom_hue");
    if (!anchor || !custom)
        return 3;
    ryb_tables();
    const float hue = ryb_lookup(ryb_to_ucs_lut, CLAMP(value, 0., 360.) / 360.f);
    int i = 0, unused = 0;
    if (!strcmp(path, "@anchor_hue"))
        *anchor = hue;
    else if (indices(path, "@custom_hue", &i, &unused) == 1 && i >= 0 && i < 4)
        custom[i] = hue;
    else
        return 3;
    return 0;
}
#endif

// ---- split-toning: the colour buttons ------------------------------------------------------
// The swatch shows hsl2rgb(hue, saturation, 0.5) (update_colorpicker_color, splittoning.c
// :253-260); picking a colour stores rgb2hsl's hue and saturation (colorpick_callback
// :320-350). The three components are written together.
static gboolean splittoning_names(const char *path, const char **hue, const char **sat, int *component) {
    int unused = 0;
    if (indices(path, "@shadow_color", component, &unused) == 1) {
        *hue = "shadow_hue";
        *sat = "shadow_saturation";
    } else if (indices(path, "@highlight_color", component, &unused) == 1) {
        *hue = "highlight_hue";
        *sat = "highlight_saturation";
    } else
        return FALSE;
    return *component >= 0 && *component < 3;
}
static void splittoning_read(dt_iop_module_t *module, JsonObject *out) {
    const char *names[2][3] = {{"shadow_hue", "shadow_saturation", "@shadow_color"},
                               {"highlight_hue", "highlight_saturation", "@highlight_color"}};
    for (int k = 0; k < 2; ++k) {
        const float *h = param_f(module, names[k][0]), *s = param_f(module, names[k][1]);
        if (!h || !s)
            continue;
        dt_aligned_pixel_t rgb;
        hsl2rgb(rgb, *h, *s, 0.5f);
        for (int c = 0; c < 3; ++c) {
            char name[32];
            snprintf(name, sizeof(name), "%s[%d]", names[k][2], c);
            put(out, name, rgb[c]);
        }
    }
}
static int splittoning_write(dt_iop_module_t *module, size_t count, const char *const *paths,
                             const double *values) {
    // Collect the colour of each button from its current value and the written components.
    const char *hue_name[2] = {NULL, NULL}, *sat_name[2] = {NULL, NULL};
    dt_aligned_pixel_t rgb[2];
    gboolean touched[2] = {FALSE, FALSE};
    for (size_t i = 0; i < count; ++i) {
        const char *hue, *sat;
        int component = 0;
        if (!splittoning_names(paths[i], &hue, &sat, &component))
            return 3;
        const int k = !strcmp(hue, "shadow_hue") ? 0 : 1;
        if (!touched[k]) {
            const float *h = param_f(module, hue), *s = param_f(module, sat);
            if (!h || !s)
                return 3;
            hsl2rgb(rgb[k], *h, *s, 0.5f);
            hue_name[k] = hue;
            sat_name[k] = sat;
            touched[k] = TRUE;
        }
        rgb[k][component] = CLAMP(values[i], 0., 1.);
    }
    for (int k = 0; k < 2; ++k) {
        if (!touched[k])
            continue;
        float h, s, l;
        rgb2hsl(rgb[k], &h, &s, &l);
        *param_f(module, hue_name[k]) = h;
        *param_f(module, sat_name[k]) = s;
    }
    return 0;
}

// ---- color look up table: patch targets relative to the source ------------------------------
// darktable's "target color" combobox (GUI state, default relative) decides whether the
// lightness, a, b and saturation sliders show the target itself or its offset from the
// source patch (_colorchecker_update_sliders, colorchecker.c:1009-1044). Paths carry the
// mode: "@target_L[mode][patch]" with mode 0 relative, 1 absolute. Writing follows
// target_L_callback, target_a/b_callback and target_C_callback (:1131-1264), including their
// clamping of a and b to ±128 and of the saturation to 0.01…128.
static void colorchecker_read(dt_iop_module_t *module, JsonObject *out) {
    const float *sL = param_f(module, "source_L"), *sa = param_f(module, "source_a"),
                *sb = param_f(module, "source_b"), *tL = param_f(module, "target_L"),
                *ta = param_f(module, "target_a"), *tb = param_f(module, "target_b");
    const int *n = param_i(module, "num_patches");
    const dt_introspection_field_t *field = module->get_f ? module->get_f("target_L") : NULL;
    if (!sL || !sa || !sb || !tL || !ta || !tb || !n || !field)
        return;
    const int patches = CLAMP(*n, 0, (int)field->Array.count);
    for (int i = 0; i < patches; ++i) {
        // The patch grid shows each source colour as darktable draws it (checker_draw,
        // colorchecker.c:1321-1327) and frames the patches whose target differs (:1335-1355).
        const dt_aligned_pixel_t Lab = {sL[i], sa[i], sb[i], 0.f};
        dt_aligned_pixel_t XYZ, rgb;
        dt_Lab_to_XYZ(Lab, XYZ);
        dt_XYZ_to_sRGB(XYZ, rgb);
        for (int c = 0; c < 3; ++c) {
            char name[48];
            snprintf(name, sizeof(name), "@source_rgb[%d][%d]", i, c);
            put(out, name, rgb[c]);
        }
        char changed[48];
        snprintf(changed, sizeof(changed), "@changed[%d]", i);
        put(out, changed,
            fabsf(tL[i] - sL[i]) > 1e-5f || fabsf(ta[i] - sa[i]) > 1e-5f || fabsf(tb[i] - sb[i]) > 1e-5f);
        const float Cin = sqrtf(sa[i] * sa[i] + sb[i] * sb[i]);
        const float Cout = sqrtf(ta[i] * ta[i] + tb[i] * tb[i]);
        for (int mode = 0; mode < 2; ++mode) {
            char name[48];
            snprintf(name, sizeof(name), "@target_L[%d][%d]", mode, i);
            put(out, name, mode ? tL[i] : tL[i] - sL[i]);
            snprintf(name, sizeof(name), "@target_a[%d][%d]", mode, i);
            put(out, name, mode ? ta[i] : ta[i] - sa[i]);
            snprintf(name, sizeof(name), "@target_b[%d][%d]", mode, i);
            put(out, name, mode ? tb[i] : tb[i] - sb[i]);
            snprintf(name, sizeof(name), "@target_C[%d][%d]", mode, i);
            put(out, name, mode ? Cout : Cout - Cin);
        }
    }
}
static int colorchecker_write(dt_iop_module_t *module, const char *path, double value) {
    float *sL = param_f(module, "source_L"), *sa = param_f(module, "source_a"),
          *sb = param_f(module, "source_b"), *tL = param_f(module, "target_L"),
          *ta = param_f(module, "target_a"), *tb = param_f(module, "target_b");
    const int *n = param_i(module, "num_patches");
    if (!sL || !sa || !sb || !tL || !ta || !tb || !n)
        return 3;
    int mode = 0, i = 0;
    char which = 0;
    if (indices(path, "@target_L", &mode, &i) == 2)
        which = 'L';
    else if (indices(path, "@target_a", &mode, &i) == 2)
        which = 'a';
    else if (indices(path, "@target_b", &mode, &i) == 2)
        which = 'b';
    else if (indices(path, "@target_C", &mode, &i) == 2)
        which = 'C';
    if (!which || mode < 0 || mode > 1 || i < 0 || i >= *n)
        return 3;
    const gboolean absolute = mode == 1;
    switch (which) {
    case 'L':
        tL[i] = absolute ? value : sL[i] + value;
        break;
    case 'a':
        ta[i] = CLAMP(absolute ? value : sa[i] + value, -128.0, 128.0);
        break;
    case 'b':
        tb[i] = CLAMP(absolute ? value : sb[i] + value, -128.0, 128.0);
        break;
    default: {
        const float Cin = sqrtf(sa[i] * sa[i] + sb[i] * sb[i]);
        const float Cout = MAX(1e-4f, sqrtf(ta[i] * ta[i] + tb[i] * tb[i]));
        const float Cnew = CLAMP(absolute ? value : Cin + value, 0.01, 128.0);
        ta[i] = CLAMP(ta[i] * Cnew / Cout, -128.0, 128.0);
        tb[i] = CLAMP(tb[i] * Cnew / Cout, -128.0, 128.0);
    }
    }
    return 0;
}

// ---- white balance: settings and finetune -------------------------------------------------
// The "settings" combobox (temperature.c:740-806, _generate_preset_combo) lists the five
// standard entries, then for raw files a section "<maker> <model>" and up to 50 camera
// presets from darktable's wb presets; p->preset stores the combobox position. A camera
// preset with fine-tuning variants shows the finetune slider in mired (:2133-2137).
enum { OM_WB_STANDARD = 5, OM_WB_SPOT = 1, OM_WB_USER = 2, OM_WB_D65 = 3, OM_WB_D65_LATE = 4 };
typedef struct {
    int position, no_ft, min_ft, max_ft;
    const char *name;
} OmWbPreset;
// The camera presets of this image, with the combobox position each one has in darktable.
static int om_wb_presets(dt_iop_module_t *module, OmWbPreset *out, int capacity) {
    const dt_image_t *img = &module->dev->image_storage;
    int found = 0, position = OM_WB_STANDARD;
    const char *wb_name = NULL;
    if (dt_image_is_ldr(img))
        return 0;
    for (int i = 0; i < dt_wb_presets_count() && found < MIN(capacity, 50); i++) {
        const dt_wb_data *wbp = dt_wb_preset(i);
        if (strcmp(wbp->make, img->camera_maker) || strcmp(wbp->model, img->camera_model))
            continue;
        if (!wb_name)
            ++position; // the "<maker> <model>" section takes a position
        if (wb_name && !strcmp(wb_name, wbp->name))
            continue;
        OmWbPreset preset = {position++, i, i, i, wbp->name};
        wb_name = wbp->name;
        if (wbp->tuning != 0) {
            int ft_pos = i, last_ft = wbp->tuning;
            preset.min_ft = ft_pos++;
            while (ft_pos < dt_wb_presets_count() && !strcmp(wb_name, dt_wb_preset(ft_pos)->name)) {
                if (dt_wb_preset(ft_pos)->tuning == 0)
                    preset.no_ft = ft_pos;
                if (dt_wb_preset(ft_pos)->tuning > last_ft) {
                    preset.max_ft = ft_pos;
                    last_ft = dt_wb_preset(ft_pos)->tuning;
                }
                ft_pos++;
            }
        }
        out[found++] = preset;
    }
    return found;
}
static gboolean wb_same_camera(const dt_image_t *img, const dt_wb_data *a, const char *name) {
    return !strcmp(a->make, img->camera_maker) && !strcmp(a->model, img->camera_model) &&
           !strcmp(a->name, name);
}
// The entry darktable selects for the current coefficients (gui_update, temperature.c
// :1205-1342): as shot, camera reference, an exact or interpolated camera preset with its
// tuning, otherwise user modified.
static int om_wb_current(dt_iop_module_t *module, int *tuning, OmWbPreset *preset_out) {
    const float *coeffs = param_f(module, "red");
    const int *stored = param_i(module, "preset");
    const dt_dev_chroma_t *chr = &module->dev->chroma;
    const dt_image_t *img = &module->dev->image_storage;
    *tuning = 0;
    if (!coeffs || !stored)
        return OM_WB_USER;
    if (dt_dev_equal_chroma(coeffs, chr->as_shot) && *stored == OM_WB_D65_LATE)
        return OM_WB_D65_LATE;
    if (dt_dev_equal_chroma(coeffs, chr->as_shot))
        return 0;
    if (dt_dev_equal_chroma(coeffs, chr->D65coeffs))
        return OM_WB_D65;
    OmWbPreset presets[50];
    const int count = om_wb_presets(module, presets, 50);
    for (int j = 0; j < count; ++j)
        for (int i = presets[j].min_ft;
             i < dt_wb_presets_count() && wb_same_camera(img, dt_wb_preset(i), presets[j].name); i++)
            if (dt_dev_equal_chroma(coeffs, dt_wb_preset(i)->channels)) {
                *tuning = dt_wb_preset(i)->tuning;
                if (preset_out)
                    *preset_out = presets[j];
                return presets[j].position;
            }
    for (int j = 0; j < count; ++j)
        for (int i = presets[j].min_ft + 1;
             i < dt_wb_presets_count() && wb_same_camera(img, dt_wb_preset(i), presets[j].name); i++) {
            if (dt_wb_preset(i - 1)->tuning + 1 == dt_wb_preset(i)->tuning)
                continue;
            for (int tune = dt_wb_preset(i - 1)->tuning + 1; tune < dt_wb_preset(i)->tuning; tune++) {
                dt_wb_data interpolated = {.tuning = tune};
                dt_wb_preset_interpolate(dt_wb_preset(i - 1), dt_wb_preset(i), &interpolated);
                if (dt_dev_equal_chroma(coeffs, interpolated.channels)) {
                    *tuning = tune;
                    if (preset_out)
                        *preset_out = presets[j];
                    return presets[j].position;
                }
            }
        }
    return OM_WB_USER;
}
static void temperature_read(dt_iop_module_t *module, JsonObject *out) {
    int tuning = 0;
    OmWbPreset preset = {-1, 0, 0, 0, NULL};
    const int position = om_wb_current(module, &tuning, &preset);
    put(out, "@preset", position);
    // The finetune slider exists only for a camera preset with fine-tuning variants.
    if (position > OM_WB_STANDARD && preset.min_ft != preset.max_ft) {
        put(out, "@finetune", tuning);
        put(out, "@finetune_min", dt_wb_preset(preset.min_ft)->tuning);
        put(out, "@finetune_max", dt_wb_preset(preset.max_ft)->tuning);
    }
}
static void wb_store(dt_iop_module_t *module, const double coeffs[4]) {
    float *f = param_f(module, "red");
    const char *names[4] = {"red", "green", "blue", "various"};
    for (int c = 0; c < 4; ++c)
        if ((f = param_f(module, names[c])))
            *f = (float)coeffs[c];
}
// _preset_tune_callback, temperature.c:1782-1915, for a chosen position and tuning.
static int temperature_apply(dt_iop_module_t *module, int position, int tune) {
    int *stored = param_i(module, "preset");
    const dt_dev_chroma_t *chr = &module->dev->chroma;
    const dt_image_t *img = &module->dev->image_storage;
    if (!stored)
        return 3;
    switch (position) {
    case 0:
    case OM_WB_D65_LATE:
        wb_store(module, chr->as_shot);
        break;
    case OM_WB_USER: // darktable restores its remembered user coefficients; they are the current ones here
        break;
    case OM_WB_D65:
        wb_store(module, chr->D65coeffs);
        break;
    case OM_WB_SPOT: // needs the image picker
        return 5;
    default: {
        OmWbPreset presets[50], *preset = NULL;
        const int count = om_wb_presets(module, presets, 50);
        for (int j = 0; j < count; ++j)
            if (presets[j].position == position)
                preset = &presets[j];
        if (!preset)
            return 5;
        gboolean found = FALSE;
        for (int i = preset->min_ft;
             i < preset->max_ft + 1 && wb_same_camera(img, dt_wb_preset(i), preset->name); i++)
            if (dt_wb_preset(i)->tuning == tune) {
                wb_store(module, dt_wb_preset(i)->channels);
                found = TRUE;
                break;
            }
        if (!found) {
            int min_id = INT_MIN, max_id = INT_MIN;
            for (int i = preset->min_ft + 1;
                 i < preset->max_ft + 1 && wb_same_camera(img, dt_wb_preset(i), preset->name); i++)
                if (dt_wb_preset(i - 1)->tuning < tune && dt_wb_preset(i)->tuning > tune) {
                    min_id = i - 1;
                    max_id = i;
                    break;
                }
            if (min_id == INT_MIN || max_id == INT_MIN || min_id == max_id)
                break;
            dt_wb_data interpolated = {.tuning = tune};
            dt_wb_preset_interpolate(dt_wb_preset(min_id), dt_wb_preset(max_id), &interpolated);
            wb_store(module, interpolated.channels);
        }
    }
    }
    *stored = position; // _update_preset; commit_params derives late_correction from it
    return 0;
}

// The "settings" entries as a list (reload_defaults, temperature.c:1662-1678, then the camera
// presets). "from image area" is the image picker and is not offered here.
void om_wb_list(dt_iop_module_t *module, int *current, JsonArray *items) {
    static const char *const standard[OM_WB_STANDARD] = {"as shot", "from image area", "user modified",
                                                         "camera reference", "as shot to reference"};
    int tuning = 0;
    const int position = om_wb_current(module, &tuning, NULL);
    *current = -1;
    for (int i = 0; i < OM_WB_STANDARD; ++i) {
        if (i == OM_WB_SPOT)
            continue;
        JsonObject *item = json_object_new(), *set = json_object_new();
        json_object_set_string_member(item, "label", standard[i]);
        json_object_set_int_member(set, "@preset", i);
        json_object_set_object_member(item, "set", set);
        if (i == position)
            *current = (int)json_array_get_length(items);
        json_array_add_object_element(items, item);
    }
    OmWbPreset presets[50];
    const int count = om_wb_presets(module, presets, 50);
    gchar *section = g_strdup_printf("%s %s", module->dev->image_storage.camera_maker,
                                     module->dev->image_storage.camera_model);
    for (int j = 0; j < count; ++j) {
        JsonObject *item = json_object_new(), *set = json_object_new();
        json_object_set_string_member(item, "label", _(presets[j].name));
        json_object_set_string_member(item, "section", section);
        json_object_set_int_member(set, "@preset", presets[j].position);
        json_object_set_object_member(item, "set", set);
        if (presets[j].position == position)
            *current = (int)json_array_get_length(items);
        json_array_add_object_element(items, item);
    }
    g_free(section);
}

// ---- dispatch ------------------------------------------------------------------------------
void om_module_describe_values(dt_iop_module_t *module, JsonObject *entry) {
    JsonObject *out = json_object_new();
    if (!strcmp(module->op, "channelmixerrgb"))
        channelmixerrgb_read(module, out);
    else if (!strcmp(module->op, "colorbalance"))
        colorbalance_read(module, out);
#ifdef OM_HAVE_RYB
    else if (!strcmp(module->op, "colorharmonizer"))
        colorharmonizer_read(module, out);
#endif
    else if (!strcmp(module->op, "splittoning"))
        splittoning_read(module, out);
    else if (!strcmp(module->op, "colorchecker"))
        colorchecker_read(module, out);
    else if (!strcmp(module->op, "temperature"))
        temperature_read(module, out);
    if (json_object_get_size(out))
        json_object_set_object_member(entry, "derived", out);
    else
        json_object_unref(out);
    om_module_describe_choices(module, entry);
}

int om_module_set_values(OmEngine *engine, dt_iop_module_t *module, size_t count, const char *const *paths,
                         const double *values, const char *const *texts) {
    if (!count)
        return 0;
    // Lists and file choices with side effects (lens, overlay image, ...) live in module_choices.c.
    const int choice = om_module_set_choice(engine, module, count, paths, values, texts);
    if (choice >= 0)
        return choice;
    for (size_t i = 0; i < count; ++i)
        if (texts[i] || !isfinite(values[i]))
            return 5;
    if (!strcmp(module->op, "splittoning"))
        return splittoning_write(module, count, paths, values);
    if (!strcmp(module->op, "channelmixerrgb"))
        return channelmixerrgb_write(module, count, paths, values);
    if (!strcmp(module->op, "colorbalance"))
        return colorbalance_write(module, count, paths, values);
    if (!strcmp(module->op, "temperature")) {
        int position = -1, tune = 0, tuning = 0;
        gboolean has_tune = FALSE;
        for (size_t i = 0; i < count; ++i) {
            if (!strcmp(paths[i], "@preset"))
                position = (int)lround(values[i]);
            else if (!strcmp(paths[i], "@finetune")) {
                tune = (int)lround(values[i]);
                has_tune = TRUE;
            } else
                return 3;
        }
        // darktable reads the finetune slider, which shows the current tuning (gui_update).
        const int current = om_wb_current(module, &tuning, NULL);
        if (position < 0)
            position = current;
        if (!has_tune)
            tune = tuning;
        if (has_tune && position <= OM_WB_STANDARD)
            return 5;
        return temperature_apply(module, position, tune);
    }
    for (size_t i = 0; i < count; ++i) {
        int error = 3;
#ifdef OM_HAVE_RYB
        if (!strcmp(module->op, "colorharmonizer"))
            error = colorharmonizer_write(module, paths[i], values[i]);
        else
#endif
            if (!strcmp(module->op, "colorchecker"))
            error = colorchecker_write(module, paths[i], values[i]);
        if (error)
            return error;
    }
    return 0;
}
