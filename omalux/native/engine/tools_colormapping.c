// SPDX-License-Identifier: GPL-3.0-or-later
// color mapping: "acquire as source" and "acquire as target", ported from darktable 5.6.1
// (src/iop/colormapping.c) without its GTK code.
//
// darktable sets an ACQUIRE flag, lets the preview pipe copy the module input (Lab) into its GUI
// data and, when the preview pipe finished, computes the L histogram and n colour clusters
// (process_clusters, lines 896-978: capture_histogram, invert_histogram, kmeans). Headless the
// module input is rendered by module_tools.c and the same functions run on it at once.
//
// Cross-image hand-over: darktable remembers the last acquired source in its GUI data and in
// /tmp/dt_colormapping_loaded ("flowback"), and reload_defaults (lines 815-829) makes that
// source the module's default on every image opened later, so switching color mapping on there
// starts with the source and only the target has to be acquired. Omalux keeps the flowback in
// the engine process for the session (no file): om_tools_colormapping_defaults applies it as
// darktable's reload_defaults does, after an image is opened and when the module is reset.
#include "module_tools_internal.h"
#include "common/points.h"

#define P(module, name) ((float *)(module)->get_p((module)->params, (name)))
#define PI(module, name) ((int *)(module)->get_p((module)->params, (name)))

// colormapping.c:59-84
#define CM_HISTN (1 << 11)
#define CM_MAXN 5
enum { CM_NEUTRAL = 0, CM_HAS_SOURCE = 1 << 0, CM_HAS_TARGET = 1 << 1, CM_ACQUIRE = 1 << 2,
       CM_GET_SOURCE = 1 << 3, CM_GET_TARGET = 1 << 4 };
typedef float cm_float2[2];

static struct {
    gboolean set;
    float hist[CM_HISTN];
    cm_float2 mean[CM_MAXN], var[CM_MAXN];
    float weight[CM_MAXN];
    int n;
} flowback;

// colormapping.c capture_histogram (lines 175-192).
static void capture_histogram(const float *col, const int width, const int height, int *hist) {
    memset(hist, 0, sizeof(int) * CM_HISTN);
    for (int k = 0; k < height; k++)
        for (int i = 0; i < width; i++) {
            const int bin = CLAMP(CM_HISTN * col[4 * (k * width + i) + 0] / 100.0, 0, CM_HISTN - 1);
            hist[bin]++;
        }
    for (int k = 1; k < CM_HISTN; k++)
        hist[k] += hist[k - 1];
    for (int k = 0; k < CM_HISTN; k++)
        hist[k] = (int)CLAMP(hist[k] * (CM_HISTN / (float)hist[CM_HISTN - 1]), 0, CM_HISTN - 1);
}

// colormapping.c invert_histogram (lines 194-213).
static void invert_histogram(const int *hist, float *inv_hist) {
    int last = 31;
    for (int i = 0; i <= last; i++)
        inv_hist[i] = 100.0f * i / (float)CM_HISTN;
    for (int i = last + 1; i < CM_HISTN; i++)
        for (int k = last; k < CM_HISTN; k++)
            if (hist[k] >= i) {
                last = k;
                inv_hist[i] = 100.0f * k / (float)CM_HISTN;
                break;
            }
}

// colormapping.c get_cluster (lines 267-282).
static int get_cluster(const float *col, const int n, cm_float2 *mean) {
    float mdist = FLT_MAX;
    int cluster = 0;
    for (int k = 0; k < n; k++) {
        const float dist = (col[1] - mean[k][0]) * (col[1] - mean[k][0]) + (col[2] - mean[k][1]) * (col[2] - mean[k][1]);
        if (dist < mdist) {
            mdist = dist;
            cluster = k;
        }
    }
    return cluster;
}

// colormapping.c kmeans (lines 284-430), with the per-thread sums of darktable's OpenMP loop
// accumulated in one pass. Clusters start at random points of the a/b range (dt_points_get).
static void kmeans(const float *col, const int width, const int height, const int n, cm_float2 *mean_out,
                   cm_float2 *var_out, float *weight_out) {
    const int nit = 40;
    cm_float2 mean[CM_MAXN], var[CM_MAXN];
    int cnt[CM_MAXN];
    float a_min = FLT_MAX, b_min = FLT_MAX, a_max = -FLT_MAX, b_max = -FLT_MAX;
    const size_t npixels = (size_t)height * width;
    for (size_t k = 0; k < npixels; k++) {
        const float a = col[4 * k + 1], b = col[4 * k + 2];
        a_min = fminf(a, a_min);
        a_max = fmaxf(a, a_max);
        b_min = fminf(b, b_min);
        b_max = fmaxf(b, b_max);
    }
    for (int k = 0; k < n; k++) {
        mean_out[k][0] = 0.9f * (a_min + (a_max - a_min) * dt_points_get());
        mean_out[k][1] = 0.9f * (b_min + (b_max - b_min) * dt_points_get());
        var_out[k][0] = var_out[k][1] = weight_out[k] = 0.0f;
        mean[k][0] = mean[k][1] = var[k][0] = var[k][1] = 0.0f;
    }
    for (int it = 0; it < nit; it++) {
        for (int k = 0; k < n; k++) {
            cnt[k] = 0;
            mean[k][0] = mean[k][1] = var[k][0] = var[k][1] = 0.0f;
        }
        for (size_t k = 0; k < npixels; k++) {
            const float *Lab = col + 4 * k;
            const int c = get_cluster(Lab, n, mean_out);
            cnt[c]++;
            var[c][0] += Lab[1] * Lab[1];
            var[c][1] += Lab[2] * Lab[2];
            mean[c][0] += Lab[1];
            mean[c][1] += Lab[2];
        }
        for (int k = 0; k < n; k++) {
            if (cnt[k] == 0)
                continue;
            mean_out[k][0] = mean[k][0] / cnt[k];
            mean_out[k][1] = mean[k][1] / cnt[k];
            var_out[k][0] = var[k][0] / cnt[k] - mean_out[k][0] * mean_out[k][0];
            var_out[k][1] = var[k][1] / cnt[k] - mean_out[k][1] * mean_out[k][1];
        }
        int count = 0;
        for (int k = 0; k < n; k++)
            count += cnt[k];
        for (int k = 0; k < n; k++)
            weight_out[k] = (count > 0) ? (float)cnt[k] / count : 0.0f;
    }
    for (int k = 0; k < n; k++) {
        if (var_out[k][0] == 0.0f || var_out[k][1] == 0.0f)
            mean_out[k][0] = mean_out[k][1] = var_out[k][0] = var_out[k][1] = weight_out[k] = 0;
        var_out[k][0] = sqrtf(var_out[k][0]);
        var_out[k][1] = sqrtf(var_out[k][1]);
    }
    for (int i = 0; i < n - 1; i++)
        for (int j = 0; j < n - 1 - i; j++)
            if (weight_out[j] > weight_out[j + 1]) {
                const float tm0 = mean_out[j + 1][0], tm1 = mean_out[j + 1][1];
                const float tv0 = var_out[j + 1][0], tv1 = var_out[j + 1][1], tw = weight_out[j + 1];
                mean_out[j + 1][0] = mean_out[j][0];
                mean_out[j + 1][1] = mean_out[j][1];
                var_out[j + 1][0] = var_out[j][0];
                var_out[j + 1][1] = var_out[j][1];
                weight_out[j + 1] = weight_out[j];
                mean_out[j][0] = tm0;
                mean_out[j][1] = tm1;
                var_out[j][0] = tv0;
                var_out[j][1] = tv1;
                weight_out[j] = tw;
            }
}

static gboolean colormapping_fields(dt_iop_module_t *m) {
    return m->get_f && m->get_f("source_ihist") && m->get_f("target_hist") && m->get_f("n") && m->get_f("flag");
}

// acquire_source_button_pressed / acquire_target_button_pressed (lines 764-784) followed by
// process_clusters (896-978) on the captured module input.
static int colormapping_acquire(OmToolContext *ctx, gboolean source) {
    dt_iop_module_t *m = ctx->module;
    OmCapture *c = ctx->capture;
    if (!colormapping_fields(m))
        return 3;
    if (!c || !c->input || c->dsc.channels != 4)
        return 4;
    int *flag = PI(m, "flag");
    const int n = CLAMP(*PI(m, "n"), 1, CM_MAXN);
    const int width = c->roi.width, height = c->roi.height;
    if (source) {
        int hist[CM_HISTN];
        capture_histogram(c->input, width, height, hist);
        invert_histogram(hist, P(m, "source_ihist"));
        kmeans(c->input, width, height, n, (cm_float2 *)P(m, "source_mean"), (cm_float2 *)P(m, "source_var"),
               P(m, "source_weight"));
        *flag |= CM_HAS_SOURCE;
        memcpy(flowback.hist, P(m, "source_ihist"), sizeof(float) * CM_HISTN);
        memcpy(flowback.mean, P(m, "source_mean"), sizeof(float) * CM_MAXN * 2);
        memcpy(flowback.var, P(m, "source_var"), sizeof(float) * CM_MAXN * 2);
        memcpy(flowback.weight, P(m, "source_weight"), sizeof(float) * CM_MAXN);
        flowback.n = n;
        flowback.set = TRUE;
    } else {
        capture_histogram(c->input, width, height, PI(m, "target_hist"));
        kmeans(c->input, width, height, n, (cm_float2 *)P(m, "target_mean"), (cm_float2 *)P(m, "target_var"),
               P(m, "target_weight"));
        *flag |= CM_HAS_TARGET;
    }
    *flag &= ~(CM_GET_TARGET | CM_GET_SOURCE | CM_ACQUIRE);
    om_tool_set_gui(ctx, "flowback", 1);
    return 0;
}
static int colormapping_source(OmToolContext *ctx) {
    return colormapping_acquire(ctx, TRUE);
}
static int colormapping_target(OmToolContext *ctx) {
    return colormapping_acquire(ctx, FALSE);
}

// colormapping.c reload_defaults (lines 815-829): the remembered source becomes the default
// of every color mapping instance; modules without history items take it as well, since
// darktable loads them from those defaults.
void om_tools_colormapping_defaults(dt_develop_t *dev, dt_iop_module_t *only) {
    if (!flowback.set)
        return;
    for (GList *it = dev->iop; it; it = g_list_next(it)) {
        dt_iop_module_t *m = it->data;
        if (!dt_iop_module_is(m, "colormapping") || (only && m != only) || !colormapping_fields(m))
            continue;
        gboolean in_history = FALSE;
        for (GList *h = dev->history; h && !only; h = g_list_next(h))
            in_history = in_history || ((dt_dev_history_item_t *)h->data)->module == m;
        for (int pass = 0; pass < (in_history ? 1 : 2); ++pass) {
            void *params = pass ? m->params : m->default_params;
            memcpy(m->get_p(params, "source_ihist"), flowback.hist, sizeof(float) * CM_HISTN);
            memcpy(m->get_p(params, "source_mean"), flowback.mean, sizeof(float) * CM_MAXN * 2);
            memcpy(m->get_p(params, "source_var"), flowback.var, sizeof(float) * CM_MAXN * 2);
            memcpy(m->get_p(params, "source_weight"), flowback.weight, sizeof(float) * CM_MAXN);
            *(int *)m->get_p(params, "n") = flowback.n;
            *(int *)m->get_p(params, "flag") = CM_HAS_SOURCE;
        }
    }
}

// colormapping.c gui_changed (lines 742-760): changing the number of clusters resets source,
// target and the flags.
void om_tools_colormapping_clusters_changed(dt_iop_module_t *m) {
    if (!colormapping_fields(m))
        return;
    memset(P(m, "source_ihist"), 0, sizeof(float) * CM_HISTN);
    memset(P(m, "source_mean"), 0, sizeof(float) * CM_MAXN * 2);
    memset(P(m, "source_var"), 0, sizeof(float) * CM_MAXN * 2);
    memset(P(m, "source_weight"), 0, sizeof(float) * CM_MAXN);
    memset(PI(m, "target_hist"), 0, sizeof(int) * CM_HISTN);
    memset(P(m, "target_mean"), 0, sizeof(float) * CM_MAXN * 2);
    memset(P(m, "target_var"), 0, sizeof(float) * CM_MAXN * 2);
    memset(P(m, "target_weight"), 0, sizeof(float) * CM_MAXN);
    *PI(m, "flag") = CM_NEUTRAL;
}

const OmToolSpec om_tools_colormapping[] = {
    // colormapping.c:1003, 1010: the module input in Lab (its default colour space)
    {"colormapping", "acquire_source", OM_TOOL_BUTTON, -1, OM_TOOL_CAPTURE, colormapping_source},
    {"colormapping", "acquire_target", OM_TOOL_BUTTON, -1, OM_TOOL_CAPTURE, colormapping_target},
    {NULL, NULL, 0, 0, 0, NULL},
};
