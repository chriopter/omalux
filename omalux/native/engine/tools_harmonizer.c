// SPDX-License-Identifier: GPL-3.0-or-later
// color harmonizer: the anchor and custom hue pickers and the harmony auto-detection
// (camera button), ported from darktable 5.6.1 (src/iop/colorharmonizer.c) without its GTK
// code. "set from vectorscope" reads darktable's scopes panel, which Omalux does not have.
#include "module_tools_internal.h"
#include "common/chromatic_adaptation.h"
#include "common/color_harmony.h"
#include "common/color_ryb.h"
#include "common/colorspaces_inline_conversions.h"
#include "common/math.h"

#define P(module, name) ((float *)(module)->get_p((module)->params, (name)))
#define PI(module, name) ((int *)(module)->get_p((module)->params, (name)))

// colorharmonizer.c:44-66
#define HARMONIZER_HUE_BINS 360
#define HARMONIZER_MAX_NODES 4
#define HARMONIZER_RYB_INVERSE_STEPS 720
enum { HARMONIZER_COMPLEMENTARY = 3, HARMONIZER_SQUARE = 8, HARMONIZER_CUSTOM = 9 };

static float ucs_to_ryb_lut[HARMONIZER_RYB_INVERSE_STEPS];
static float ryb_to_ucs_lut[HARMONIZER_RYB_INVERSE_STEPS];
static gsize luts_ready = 0;

// colorharmonizer.c _find_max_chroma, _hue_to_srgb and _ucs_hue_to_ryb_hue (lines 697-812).
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
    const dt_aligned_pixel_t srgb = {CLAMP(sRGB[0], 0.f, 1.f), CLAMP(sRGB[1], 0.f, 1.f), CLAMP(sRGB[2], 0.f, 1.f), 0.f};
    dt_aligned_pixel_t lrgb, HCV;
    dt_sRGB_to_linear_sRGB(srgb, lrgb);
    dt_RGB_2_HCV(lrgb, HCV);
    return dt_rgb_hue_to_ryb_hue(HCV[0]);
}

// colorharmonizer.c _hue_lerp, _ucs_to_ryb_fast, _ryb_to_ucs_fast and _build_hue_luts
// (lines 514-563).
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
static float ucs_to_ryb_fast(const float ucs) {
    const float pos = ucs * HARMONIZER_RYB_INVERSE_STEPS;
    const int i0 = (int)pos % HARMONIZER_RYB_INVERSE_STEPS;
    const int i1 = (i0 + 1) % HARMONIZER_RYB_INVERSE_STEPS;
    return hue_lerp(ucs_to_ryb_lut[i0], ucs_to_ryb_lut[i1], pos - (int)pos);
}
static float ryb_to_ucs_fast(const float ryb) {
    const float pos = ryb * HARMONIZER_RYB_INVERSE_STEPS;
    const int i0 = (int)pos % HARMONIZER_RYB_INVERSE_STEPS;
    const int i1 = (i0 + 1) % HARMONIZER_RYB_INVERSE_STEPS;
    return hue_lerp(ryb_to_ucs_lut[i0], ryb_to_ucs_lut[i1], pos - (int)pos);
}
static void build_hue_luts(void) {
    if (!g_once_init_enter(&luts_ready))
        return;
    for (int i = 0; i < HARMONIZER_RYB_INVERSE_STEPS; i++)
        ucs_to_ryb_lut[i] = ucs_hue_to_ryb_hue(i / (float)HARMONIZER_RYB_INVERSE_STEPS);
    for (int j = 0; j < HARMONIZER_RYB_INVERSE_STEPS; j++) {
        const float target = j / (float)HARMONIZER_RYB_INVERSE_STEPS;
        float best_dist = 1.0f, best_ucs = 0.0f;
        for (int i = 0; i < HARMONIZER_RYB_INVERSE_STEPS; i++) {
            float d = fabsf(ucs_to_ryb_lut[i] - target);
            if (d > 0.5f)
                d = 1.0f - d;
            if (d < best_dist) {
                best_dist = d;
                best_ucs = i / (float)HARMONIZER_RYB_INVERSE_STEPS;
            }
        }
        ryb_to_ucs_lut[j] = best_ucs;
    }
    g_once_init_leave(&luts_ready, 1);
}

// colorharmonizer.c get_harmony_nodes (lines 224-247).
static void harmony_nodes(const int rule, const float anchor_hue, float *nodes, int *num_nodes) {
    const int rotation = (int)roundf(ucs_to_ryb_fast(anchor_hue) * 360.0f) % 360;
    float node_angles[HARMONIZER_MAX_NODES];
    dt_color_harmony_get_sector_angles((dt_color_harmony_type_t)(rule + 1), rotation, node_angles, num_nodes);
    for (int i = 0; i < *num_nodes; i++)
        nodes[i] = ryb_to_ucs_fast(node_angles[i]);
}

// colorharmonizer.c _score_harmony and _auto_detect_harmony (lines 1240-1330).
static float score_harmony(const float *histo, const int num_bins, const int rule, const float anchor_hue) {
    float nodes[HARMONIZER_MAX_NODES];
    int num_nodes = 1;
    harmony_nodes(rule, anchor_hue, nodes, &num_nodes);
    if (num_nodes <= 0)
        return 0.0f;
    const float sigma = 0.5f / (float)num_nodes;
    const float inv_2sigma2 = 1.0f / (2.0f * sigma * sigma);
    float total = 0.0f, covered = 0.0f;
    for (int b = 0; b < num_bins; b++) {
        if (histo[b] <= 0.0f)
            continue;
        const float h = (b + 0.5f) / (float)num_bins;
        float max_w = 0.0f;
        for (int i = 0; i < num_nodes; i++) {
            float d = fabsf(h - nodes[i]);
            if (d > 0.5f)
                d = 1.0f - d;
            const float w = expf(-d * d * inv_2sigma2);
            if (w > max_w)
                max_w = w;
        }
        covered += histo[b] * max_w;
        total += histo[b];
    }
    return (total > 1e-6f) ? (covered / total) : 0.0f;
}

// colorharmonizer.c _update_histogram (lines 261-310) on the module input and
// _auto_detect_callback (lines 1350-1378).
static int harmonizer_auto_detect(OmToolContext *ctx) {
    dt_iop_module_t *m = ctx->module;
    OmCapture *c = ctx->capture;
    if (!m->get_f || !m->get_f("anchor_hue") || !m->get_f("rule") || !c || c->dsc.channels != 4)
        return 3;
    build_hue_luts();
    const dt_iop_order_iccprofile_info_t *const work_profile = dt_ioppr_get_pipe_work_profile_info(&c->pipe);
    if (!work_profile)
        return 4;
    const float L_white = Y_to_dt_UCS_L_star(1.0f);
    float histo[HARMONIZER_HUE_BINS] = {0.0f};
    for (int j = 0; j < c->roi.height; j++) {
        const float *src = c->input + (size_t)4 * c->roi.width * j;
        for (int i = 0; i < c->roi.width; i++, src += 4) {
            dt_aligned_pixel_t px_rgb = {fmaxf(src[0], 0.0f), fmaxf(src[1], 0.0f), fmaxf(src[2], 0.0f), 0.0f};
            dt_aligned_pixel_t px_JCH;
            dt_ioppr_rgb_matrix_to_dt_UCS_JCH(px_rgb, px_JCH, work_profile->matrix_in_transposed, L_white);
            const float chroma = px_JCH[1];
            if (chroma > 0.01f) {
                const float hue = (px_JCH[2] + M_PI_F) / DT_2PI_F;
                const int bin = (int)(hue * HARMONIZER_HUE_BINS) % HARMONIZER_HUE_BINS;
                histo[bin] += chroma;
            }
        }
    }
    float smooth[HARMONIZER_HUE_BINS];
    memcpy(smooth, histo, sizeof(smooth));
    for (int pass = 0; pass < 3; pass++) {
        float tmp[HARMONIZER_HUE_BINS];
        for (int b = 0; b < HARMONIZER_HUE_BINS; b++) {
            const int prev = (b - 1 + HARMONIZER_HUE_BINS) % HARMONIZER_HUE_BINS;
            const int next = (b + 1) % HARMONIZER_HUE_BINS;
            tmp[b] = (smooth[prev] + smooth[b] + smooth[next]) * (1.0f / 3.0f);
        }
        memcpy(smooth, tmp, sizeof(smooth));
    }
    float best_score = -1.0f, best_anchor = 0.0f;
    int best_rule = HARMONIZER_COMPLEMENTARY;
    for (int r = 0; r < HARMONIZER_SQUARE + 1; r++)
        for (int a = 0; a < 360; a++) {
            const float anchor = (float)a / 360.0f;
            const float score = score_harmony(smooth, HARMONIZER_HUE_BINS, r, anchor);
            if (score > best_score) {
                best_score = score;
                best_rule = r;
                best_anchor = anchor;
            }
        }
    *PI(m, "rule") = best_rule;
    *P(m, "anchor_hue") = best_anchor;
    return 0;
}

// colorharmonizer.c _picked_color_to_hue and color_picker_apply (lines 1170-1236).
static int harmonizer_pick(OmToolContext *ctx, int node) {
    dt_iop_module_t *m = ctx->module;
    if (!m->get_f || !m->get_f("anchor_hue") || !m->get_f("custom_hue"))
        return 3;
    const dt_iop_order_iccprofile_info_t *const work_profile = ctx->capture->pipe.work_profile_info;
    if (!work_profile)
        return 4;
    dt_aligned_pixel_t px_rgb, px_xyz;
    for (int c = 0; c < 3; c++)
        px_rgb[c] = fmaxf(ctx->picked.in[DT_PICK_MEAN][c], 0.0f);
    px_rgb[3] = 0.0f;
    const float(*const matrix_in)[4] = (const float(*)[4])work_profile->matrix_in;
    for (int r = 0; r < 3; r++)
        px_xyz[r] = matrix_in[r][0] * px_rgb[0] + matrix_in[r][1] * px_rgb[1] + matrix_in[r][2] * px_rgb[2];
    px_xyz[3] = 0.0f;
    const float L_white = Y_to_dt_UCS_L_star(1.0f);
    dt_aligned_pixel_t px_xyz_d65, px_xyY, px_JCH;
    XYZ_D50_to_D65(px_xyz, px_xyz_d65);
    dt_D65_XYZ_to_xyY(px_xyz_d65, px_xyY);
    xyY_to_dt_UCS_JCH(px_xyY, L_white, px_JCH);
    const float hue = (px_JCH[2] + M_PI_F) / DT_2PI_F;
    if (node < 0)
        *P(m, "anchor_hue") = hue;
    else
        P(m, "custom_hue")[node] = hue;
    return 0;
}
static int harmonizer_anchor(OmToolContext *ctx) {
    return harmonizer_pick(ctx, -1);
}
static int harmonizer_custom_0(OmToolContext *ctx) {
    return harmonizer_pick(ctx, 0);
}
static int harmonizer_custom_1(OmToolContext *ctx) {
    return harmonizer_pick(ctx, 1);
}
static int harmonizer_custom_2(OmToolContext *ctx) {
    return harmonizer_pick(ctx, 2);
}
static int harmonizer_custom_3(OmToolContext *ctx) {
    return harmonizer_pick(ctx, 3);
}

const OmToolSpec om_tools_harmonizer[] = {
    // colorharmonizer.c:1415 the camera button (auto detect), :1444 anchor and :1499 custom
    // node point-or-area pickers
    {"colorharmonizer", "auto_detect", OM_TOOL_BUTTON, -1, OM_TOOL_CAPTURE, harmonizer_auto_detect},
    {"colorharmonizer", "anchor_hue", OM_TOOL_AREA, -1, 0, harmonizer_anchor},
    {"colorharmonizer", "custom_hue_0", OM_TOOL_AREA, -1, 0, harmonizer_custom_0},
    {"colorharmonizer", "custom_hue_1", OM_TOOL_AREA, -1, 0, harmonizer_custom_1},
    {"colorharmonizer", "custom_hue_2", OM_TOOL_AREA, -1, 0, harmonizer_custom_2},
    {"colorharmonizer", "custom_hue_3", OM_TOOL_AREA, -1, 0, harmonizer_custom_3},
    {NULL, NULL, 0, 0, 0, NULL},
};
