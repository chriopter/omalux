// SPDX-License-Identifier: GPL-3.0-or-later
// tone equalizer: the two magic-wand buttons on "mask exposure compensation" and "mask
// contrast compensation", ported from darktable 5.6.1 (src/iop/toneequal.c,
// auto_adjust_exposure_boost / auto_adjust_contrast_boost, lines 1784-1925).
//
// darktable reads the luminance mask the module computed on the preview pipe
// (toneeq_process caches it in thumb_preview_buf) and its log histogram deciles
// (compute_log_histogram_and_stats). Headless, the module input is rendered (module_tools.c
// capture), the mask is computed with the module's own helpers (common/luminance_mask.h,
// fast_guided_filter.h, eigf.h) from the parameters exactly as commit_params and
// modify_roi_in prepare them, and the same statistics drive the same formulas.
#include "module_tools_internal.h"
#include "common/eigf.h"
#include "common/fast_guided_filter.h"
#include "common/luminance_mask.h"

#define P(module, name) ((float *)(module)->get_p((module)->params, (name)))
#define PI(module, name) ((int *)(module)->get_p((module)->params, (name)))

// toneequal.c:130-131, 161-165
#define TE_UI_SAMPLES 256
#define TE_CONTRAST_FULCRUM exp2f(-4.0f)
enum { TE_NONE = 0, TE_AVG_GUIDED, TE_GUIDED, TE_AVG_EIGF, TE_EIGF };

// The members of dt_iop_toneequalizer_data_t the mask needs (toneequal.c:192-202), filled as
// commit_params (1581-1606) and modify_roi_in (1182-1193) do. "scale" is never set by
// darktable: the piece data is calloc'ed (init_pipe, 1649), so it stays 0.
typedef struct {
    float blending, feathering, contrast_boost, exposure_boost, quantization, scale;
    int radius, iterations;
    dt_iop_luminance_mask_method_t method;
    int details;
} OmToneMask;

// toneequal.c compute_luminance_mask (lines 865-942).
static void te_luminance_mask(const float *in, float *luminance, size_t width, size_t height, const OmToneMask *d) {
    switch (d->details) {
    case TE_AVG_GUIDED:
        luminance_mask(in, luminance, width, height, d->method, d->exposure_boost, 0.0f, 1.0f);
        fast_surface_blur(luminance, width, height, d->radius, d->feathering, d->iterations, DT_GF_BLENDING_GEOMEAN,
                          d->scale, d->quantization, exp2f(-14.0f), 4.0f);
        break;
    case TE_GUIDED:
        luminance_mask(in, luminance, width, height, d->method, d->exposure_boost, TE_CONTRAST_FULCRUM,
                       d->contrast_boost);
        fast_surface_blur(luminance, width, height, d->radius, d->feathering, d->iterations, DT_GF_BLENDING_LINEAR,
                          d->scale, d->quantization, exp2f(-14.0f), 4.0f);
        break;
    case TE_AVG_EIGF:
        luminance_mask(in, luminance, width, height, d->method, d->exposure_boost, 0.0f, 1.0f);
        fast_eigf_surface_blur(luminance, width, height, d->radius, d->feathering, d->iterations,
                               DT_GF_BLENDING_GEOMEAN, d->scale, d->quantization, exp2f(-14.0f), 4.0f);
        break;
    case TE_EIGF:
        luminance_mask(in, luminance, width, height, d->method, d->exposure_boost, TE_CONTRAST_FULCRUM,
                       d->contrast_boost);
        fast_eigf_surface_blur(luminance, width, height, d->radius, d->feathering, d->iterations,
                               DT_GF_BLENDING_LINEAR, d->scale, d->quantization, exp2f(-14.0f), 4.0f);
        break;
    case TE_NONE:
    default:
        luminance_mask(in, luminance, width, height, d->method, d->exposure_boost, 0.0f, 1.0f);
        break;
    }
}

// toneequal.c compute_log_histogram_and_stats (lines 1396-1468): the 5 % and 95 % positions
// of the mask's exposure in [-10, +6] EV, and the [-8, 0] EV histogram the graph draws.
static void te_histogram(const float *luminance, size_t num_elem, int histogram[TE_UI_SAMPLES], int *max_histogram,
                         float *first_decile, float *last_decile) {
    memset(histogram, 0, sizeof(int) * TE_UI_SAMPLES);
    enum { TEMP_SAMPLES = 2 * TE_UI_SAMPLES };
    int temp_hist[TEMP_SAMPLES];
    memset(temp_hist, 0, sizeof(temp_hist));
    for (size_t k = 0; k < num_elem; k++) {
        const int index =
            CLAMP((int)(((log2f(luminance[k]) + 10.0f) / 16.0f) * (float)TEMP_SAMPLES), 0, TEMP_SAMPLES - 1);
        temp_hist[index] += 1;
    }
    const int first = (int)((float)num_elem * 0.05f);
    const int last = (int)((float)num_elem * (1.0f - 0.95f));
    int population = 0, first_pos = 0, last_pos = 0;
    for (int k = 0; k < TEMP_SAMPLES; ++k) {
        const size_t prev_population = population;
        population += temp_hist[k];
        if (prev_population < first && first <= population) {
            first_pos = k;
            break;
        }
    }
    population = 0;
    for (int k = TEMP_SAMPLES - 1; k >= 0; --k) {
        const size_t prev_population = population;
        population += temp_hist[k];
        if (prev_population < last && last <= population) {
            last_pos = k;
            break;
        }
    }
    *first_decile = 16.0 * (float)first_pos / (float)(TEMP_SAMPLES - 1) - 10.0;
    *last_decile = 16.0 * (float)last_pos / (float)(TEMP_SAMPLES - 1) - 10.0;
    for (size_t k = 0; k < TEMP_SAMPLES; ++k) {
        const float EV = 16.0 * (float)k / (float)(TEMP_SAMPLES - 1) - 10.0;
        const int i = CLAMP((int)(((EV + 8.0f) / 8.0f) * (float)TE_UI_SAMPLES), 0, TE_UI_SAMPLES - 1);
        histogram[i] += temp_hist[k];
        *max_histogram = histogram[i] > *max_histogram ? histogram[i] : *max_histogram;
    }
}

// The mask's deciles of the current module input; also reports the histogram for the graph.
static int te_deciles(OmToolContext *ctx, float *first_decile, float *last_decile) {
    dt_iop_module_t *m = ctx->module;
    OmCapture *c = ctx->capture;
    if (!c || !c->input || c->dsc.channels != 4)
        return 4;
    OmToneMask d = {0};
    d.method = *PI(m, "method");
    d.details = *PI(m, "details");
    d.iterations = *PI(m, "iterations");
    d.quantization = *P(m, "quantization");
    d.blending = *P(m, "blending") / 100.0f;
    d.feathering = 1.f / *P(m, "feathering");
    d.contrast_boost = exp2f(*P(m, "contrast_boost"));
    d.exposure_boost = exp2f(*P(m, "exposure_boost"));
    const int max_size = MAX(c->piece->iwidth, c->piece->iheight);
    const float diameter = d.blending * max_size * c->roi.scale;
    d.radius = (int)((diameter - 1.0f) / (2.0f));
    const size_t width = c->roi.width, height = c->roi.height;
    float *luminance = dt_alloc_align_float(width * height);
    if (!luminance)
        return 4;
    te_luminance_mask(c->input, luminance, width, height, &d);
    int histogram[TE_UI_SAMPLES], max_histogram = 0;
    te_histogram(luminance, width * height, histogram, &max_histogram, first_decile, last_decile);
    dt_free_align(luminance);
    JsonObject *out = json_object_new();
    JsonArray *bins = json_array_new();
    for (int i = 0; i < TE_UI_SAMPLES; ++i)
        json_array_add_int_element(bins, histogram[i]);
    json_object_set_array_member(out, "bins", bins);
    json_object_set_int_member(out, "max", max_histogram);
    json_object_set_double_member(out, "first_decile", *first_decile);
    json_object_set_double_member(out, "last_decile", *last_decile);
    json_object_set_object_member(ctx->extra, "mask_histogram", out);
    return 0;
}

static int toneequal_auto(OmToolContext *ctx, gboolean contrast) {
    dt_iop_module_t *m = ctx->module;
    if (!m->get_f || !m->get_f("exposure_boost") || !m->get_f("contrast_boost"))
        return 3;
    // "activate module and do nothing" (lines 1793-1803): switching on is the history item.
    if (!m->enabled)
        return 0;
    float first = 0.f, last = 0.f;
    const int error = te_deciles(ctx, &first, &last);
    if (error)
        return error;
    float *exposure_boost = P(m, "exposure_boost"), *contrast_boost = P(m, "contrast_boost");
    const float fd_new = exp2f(first);
    const float ld_new = exp2f(last);
    const float e = exp2f(*exposure_boost);
    float c = exp2f(*contrast_boost);
    const float fd_old = ((fd_new - TE_CONTRAST_FULCRUM) / c + TE_CONTRAST_FULCRUM) / e;
    const float ld_old = ((ld_new - TE_CONTRAST_FULCRUM) / c + TE_CONTRAST_FULCRUM) / e;
    const float s1 = TE_CONTRAST_FULCRUM - exp2f(-7.0);
    const float s2 = exp2f(-1.0) - TE_CONTRAST_FULCRUM;
    const float mix = fd_old * s2 + ld_old * s1;
    if (!contrast) {
        *exposure_boost = log2f(TE_CONTRAST_FULCRUM * (s1 + s2) / mix);
        return 0;
    }
    c = log2f(mix / (TE_CONTRAST_FULCRUM * (ld_old - fd_old)) / c);
    const int details = *PI(m, "details");
    const float feathering = *P(m, "feathering");
    if (details == TE_EIGF && c > 0.0f) {
        const float correction = -0.0276f + 0.01823 * feathering + (0.7566f - 1.0f) * c;
        if (feathering < 5.0f)
            c += correction;
        else if (feathering < 10.0f)
            c += correction * (2.0f - feathering / 5.0f);
    } else if (details == TE_GUIDED && c > 0.0f)
        c = 0.0235f + 1.1225f * c;
    *contrast_boost += c;
    return 0;
}

static int toneequal_exposure_boost(OmToolContext *ctx) {
    return toneequal_auto(ctx, FALSE);
}
static int toneequal_contrast_boost(OmToolContext *ctx) {
    return toneequal_auto(ctx, TRUE);
}
// The mask histogram darktable draws behind the advanced tab's graph (update_histogram).
static int toneequal_histogram(OmToolContext *ctx) {
    float first = 0.f, last = 0.f;
    return te_deciles(ctx, &first, &last);
}

const OmToolSpec om_tools_toneequal[] = {
    // toneequal.c:3326, 3338: dt_bauhaus_widget_set_quad with dtgtk_cairo_paint_wand
    {"toneequal", "exposure_boost", OM_TOOL_BUTTON, -1, OM_TOOL_CAPTURE, toneequal_exposure_boost},
    {"toneequal", "contrast_boost", OM_TOOL_BUTTON, -1, OM_TOOL_CAPTURE, toneequal_contrast_boost},
    {"toneequal", "mask_histogram", OM_TOOL_BUTTON, -1, OM_TOOL_CAPTURE | OM_TOOL_KEEP_OFF, toneequal_histogram},
    {NULL, NULL, 0, 0, 0, NULL},
};
