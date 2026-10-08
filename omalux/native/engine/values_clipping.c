// SPDX-License-Identifier: GPL-3.0-or-later
// The deprecated crop and rotate module (clipping) shows two values its parameters store
// differently, converted as darktable 5.6.1's GUI callbacks do (src/iop/clipping.c):
//   @flip   none / horizontal / vertical / both: the signs of cw and ch (gui_update 1942-1957,
//           hvflip_callback 2021-2030)
//   @aspect the position in darktable's aspect list (gui_init 2124-2213: the built-in ratios,
//           special entries first, then sorted from square to wide by _aspect_ratio_cmp), stored
//           as ratio_d (its sign is the orientation) and ratio_n (aspect_presets_changed
//           1691-1790), which then fits the crop box to the ratio (apply_box_aspect 1536-1640,
//           GRAB_HORIZONTAL: the height changes around the centre, then the box is clamped).
// darktable fits the box drawn on the rotated output; headless it is fitted in the module's
// parameter box, which is the same while the module neither rotates nor corrects keystone.
// darktable's user ratios from plugins/darkroom/clipping/extra_aspect_ratios are not listed.
#include "values_clipping.h"
#include "develop/develop.h"

typedef struct {
    int d, n;
} OmAspect;
// clipping.c:2124-2143 sorted by _aspect_ratio_cmp (2040-2058); index = the row's value
static const OmAspect aspects[] = {
    {0, 0},    {1, 0},    {1, 1},      {2445, 2032}, {5, 4},       {14, 11}, {110, 85},
    {4, 3},    {7, 5},    {14142136, 10000000},      {3, 2},       {16, 10}, {16180340, 10000000},
    {16, 9},   {185, 100}, {2, 1},     {235, 100},   {237, 100},   {239, 100}, {300, 100},
};

static void *pp(dt_iop_module_t *m, const char *name) {
    return m->get_f && m->get_f(name) ? m->get_p(m->params, name) : NULL;
}

void om_values_clipping_read(dt_iop_module_t *module, JsonObject *out) {
    const float *cw = pp(module, "cw"), *ch = pp(module, "ch");
    const int *rd = pp(module, "ratio_d"), *rn = pp(module, "ratio_n");
    if (!cw || !ch || !rd || !rn)
        return;
    json_object_set_double_member(out, "@flip", (*cw < 0 ? 1 : 0) | (*ch < 0 ? 2 : 0));
    int d = abs(*rd), n = *rn;
    if (*rd == -1 && *rn == -1) {
        d = dt_conf_get_int("plugins/darkroom/clipping/ratio_d");
        n = dt_conf_get_int("plugins/darkroom/clipping/ratio_n");
    }
    for (size_t i = 0; i < G_N_ELEMENTS(aspects); i++)
        if (aspects[i].d == d && aspects[i].n == n) {
            json_object_set_double_member(out, "@aspect", (double)i);
            break;
        }
}

// apply_box_aspect with GRAB_HORIZONTAL in the parameter box ([0, 1] as clip_max).
static void fit_box(dt_iop_module_t *module, int d, int n, int ratio_d) {
    float *cx = pp(module, "cx"), *cy = pp(module, "cy"), *cw = pp(module, "cw"), *ch = pp(module, "ch");
    int iwd = 0, iht = 0;
    dt_dev_get_processed_size(&module->dev->full, &iwd, &iht);
    if (iwd <= 0 || iht <= 0)
        return;
    float aspect;
    if (d == 1 && n == 0) // original image (_ratio_get_aspect 1410-1421)
        aspect = (ratio_d > 0 && iwd > iht) || (ratio_d < 0 && iwd < iht) ? (float)iwd / iht : (float)iht / iwd;
    else if (n == 0)
        return;          // freehand
    else
        aspect = ratio_d < 0 ? (float)n / d : (float)d / n;
    if (iwd < iht)
        aspect = 1.0f / aspect;
    if (aspect <= 0)
        return;
    double x = *cx, y = *cy, w = fabsf(*cw) - *cx, h = fabsf(*ch) - *cy;
    const double target_h = (double)iwd * w / ((double)iht * aspect);
    const double off = target_h - h;
    h += off;
    y -= .5 * off;
    if (y < 0.0) {
        w *= (h + y) / h;
        h = h + y;
        y = 0.0;
    }
    if (y + h > 1.0) {
        w *= (1.0 - y) / h;
        h = 1.0 - y;
    }
    // commit_box clamps cx, cy to 0..0.9 and the right/bottom edges to 0.1..1, keeping the flip
    *cx = CLAMPF(x, 0.0f, 0.9f);
    *cy = CLAMPF(y, 0.0f, 0.9f);
    *cw = copysignf(CLAMPF(x + w, 0.1f, 1.0f), *cw);
    *ch = copysignf(CLAMPF(y + h, 0.1f, 1.0f), *ch);
}

int om_values_clipping_write(dt_iop_module_t *module, size_t count, const char *const *paths, const double *values) {
    float *cw = pp(module, "cw"), *ch = pp(module, "ch");
    int *rd = pp(module, "ratio_d"), *rn = pp(module, "ratio_n");
    if (!cw || !ch || !rd || !rn)
        return 3;
    for (size_t i = 0; i < count; i++) {
        const int v = (int)lround(values[i]);
        if (!strcmp(paths[i], "@flip")) {
            if (v < 0 || v > 3)
                return 5;
            // commit_box: a module switched off starts from the full box
            if (!module->enabled) {
                *(float *)pp(module, "cx") = *(float *)pp(module, "cy") = 0.0f;
                *cw = *ch = 1.0f;
            }
            *cw = copysignf(*cw, (v & 1) ? -1.0f : 1.0f);
            *ch = copysignf(*ch, (v & 2) ? -1.0f : 1.0f);
        } else if (!strcmp(paths[i], "@aspect")) {
            if (v < 0 || v >= (int)G_N_ELEMENTS(aspects))
                return 5;
            const int d = aspects[v].d, n = aspects[v].n;
            // gui_update (1964-1968): an unset ratio (-1, -1) shows the configured one
            if (*rd == -1 && *rn == -1) {
                *rd = dt_conf_get_int("plugins/darkroom/clipping/ratio_d");
                *rn = dt_conf_get_int("plugins/darkroom/clipping/ratio_n");
            }
            if (d == abs(*rd) && n == *rn)
                continue;
            *rd = *rd >= 0 ? d : -d;
            *rn = n;
            dt_conf_set_int("plugins/darkroom/clipping/ratio_d", abs(*rd));
            dt_conf_set_int("plugins/darkroom/clipping/ratio_n", abs(*rn));
            fit_box(module, d, n, *rd);
        } else
            return 3;
    }
    return 0;
}
