// SPDX-License-Identifier: GPL-3.0-or-later
// The mask previews darktable draws from inside a module (darktable 5.6.1), without its GUI:
//   toneequal        "display exposure mask"                 (toneequal.c:3406, toneeq_process 1149)
//   filmicrgb        "display highlight reconstruction mask" (filmicrgb.c:4450, process 2119)
//   colorzones       "display selection"                     (colorzones.c:2681, process_display 425)
//   colorbalancergb  the mask quads of "shadows fall-off", "mask middle-gray fulcrum" and
//                    "highlights fall-off" over a checkerboard (colorbalancergb.c:1418 mask_callback,
//                    process 643-930, "mask preview settings" 2030-2068)
//   colorequal       the quads of "contrast threshold" and "parameter size"
//                    (colorequal.c:2565-2580, process 989/1101-1170)
// darktable draws them only on the darkroom pipe, reading its GUI data (gui_attached, gui_data,
// and for colorzones the focus). Headless the editor's full pipe runs with neither, so the
// displayed module's process is wrapped for that pipe only (module->process_plain, which
// dt_iop default_process and tiling call; OpenCL is switched off for the piece while shown):
// - toneequal, filmicrgb and colorzones run the module's own display code, ported below with
//   the parameters as commit_params prepares them; toneequal and colorzones set
//   pipe->mask_display as darktable does, so the pixelpipe skips the later modules and gamma
//   draws the result (PASSTHRU grey, colorzones' selection in yellow).
// - colorbalancergb runs unchanged and its result is then mixed with the checkerboard by the
//   module's own opacity masks, exactly the display branch of its pixel loop.
// - colorequal reads only g->mask_mode in process: it runs with a GUI data block that holds
//   just that value (the struct is the pinned release's, colorequal.c:239-284).
// Only one module shows a preview at a time (darktable: the focused module). The tool
// "display_mask" sets it: gui {"display": 0/1, "type": n (colorbalancergb mask, colorequal
// mode), "channel": n (colorzones: the curve shown), checker settings for colorbalancergb}.
#include "module_tools_internal.h"
#include "module_display.h"
#include "blend_display.h"
#include "common/chromatic_adaptation.h"
#include "common/colorspaces_inline_conversions.h"
#include "develop/blend.h"
#include "gui/gtk.h"

#define P(module, name) ((float *)(module)->get_p((module)->params, (name)))
#define PI(module, name) ((int *)(module)->get_p((module)->params, (name)))

static struct {
    char operation[64];
    int instance;
    int type, channel;
    int checker_size;
    dt_aligned_pixel_t checker_1, checker_2;
    dt_develop_blend_params_t saved;
    gboolean swapped;
} state;

static gboolean shown(void) {
    return state.operation[0] != 0;
}

static dt_iop_module_t *display_module(OmEngine *engine) {
    if (!shown() || !engine->loaded)
        return NULL;
    return dt_iop_get_module_by_op_priority(engine->dev.iop, state.operation, state.instance);
}

static gboolean is_target(const dt_iop_module_t *self, const dt_dev_pixelpipe_iop_t *piece) {
    return shown() && dt_pipe_is_full(piece->pipe) && piece->pipe == self->dev->full.pipe &&
           !strcmp(self->op, state.operation) && self->multi_priority == state.instance;
}

// ---- toneequal --------------------------------------------------------------------------
// toneequal.c display_luminance_mask (946-987).
static void toneequal_display(dt_iop_module_t *self, dt_dev_pixelpipe_iop_t *piece, const float *in, float *out,
                              const dt_iop_roi_t *roi_in, const dt_iop_roi_t *roi_out) {
    const size_t width = roi_in->width, height = roi_in->height;
    // toneeq_process sanity checks (1012-1015)
    if (width < 1 || height < 1 || roi_in->width < roi_out->width || roi_in->height < roi_out->height ||
        piece->colors != 4)
        return;
    float *luminance = dt_alloc_align_float(width * height);
    if (!luminance)
        return;
    if (om_toneequal_mask(self, in, luminance, width, height, MAX(piece->iwidth, piece->iheight), roi_in->scale)) {
        const size_t offset_x = (roi_in->x < roi_out->x) ? -roi_in->x + roi_out->x : 0;
        const size_t offset_y = (roi_in->y < roi_out->y) ? -roi_in->y + roi_out->y : 0;
        const size_t in_width = roi_in->width;
        const size_t out_width = (roi_in->width > roi_out->width) ? roi_out->width : roi_in->width;
        const size_t out_height = (roi_in->height > roi_out->height) ? roi_out->height : roi_in->height;
        DT_OMP_FOR(collapse(2))
        for (size_t i = 0; i < out_height; ++i)
            for (size_t j = 0; j < out_width; ++j) {
                // normalize the mask intensity between -8 EV and 0 EV, "gamma" 2.0
                const float intensity =
                    sqrtf(fminf(fmaxf(luminance[(i + offset_y) * in_width + (j + offset_x)] - 0.00390625f, 0.f) /
                                    0.99609375f,
                                1.f));
                const size_t index = (i * out_width + j) * 4;
                for (int c = 0; c < 3; c++)
                    out[index + c] = intensity;
                out[index + 3] = in[((i + offset_y) * in_width + (j + offset_x)) * 4 + 3];
            }
        piece->pipe->mask_display = DT_DEV_PIXELPIPE_DISPLAY_PASSTHRU;
    }
    dt_free_align(luminance);
}

// ---- filmicrgb --------------------------------------------------------------------------
// filmicrgb.c process (2089-2127): mask_clipped_pixels (1038-1068) and display_mask
// (2007-2021), with normalize and reconstruct_feather from commit_params (3036-3096).
// darktable fills the mask only while highlight reconstruction is enabled and otherwise shows
// an uninitialised buffer; the mask is computed here in both cases.
static void filmicrgb_display(dt_iop_module_t *self, dt_dev_pixelpipe_iop_t *piece, const float *in, float *out,
                              const dt_iop_roi_t *roi_in, const dt_iop_roi_t *roi_out) {
    if (!dt_iop_have_required_input_format(4, self, piece->colors, in, out, roi_in, roi_out))
        return;
    const float grey_source = *PI(self, "custom_grey") ? *P(self, "grey_point_source") / 100.0f : 0.1845f;
    const float threshold = powf(2.0f, *P(self, "white_point_source") + *P(self, "reconstruct_threshold")) * grey_source;
    const float feathering = exp2f(12.f / *P(self, "reconstruct_feather"));
    const float normalize = feathering / threshold;
    const size_t width = roi_out->width, height = roi_out->height;
    DT_OMP_FOR()
    for (size_t k = 0; k < 4 * height * width; k += 4) {
        const float pix_max = sqrtf(sqf(in[k]) + sqf(in[k + 1]) + sqf(in[k + 2]));
        const float argument = -pix_max * normalize + feathering;
        const float weight = fminf(fmaxf(1.0f / (1.0f + exp2f(argument)), 0.0f), 1.0f);
        for (int c = 0; c < 4; c++)
            out[k + c] = weight;
    }
}

// ---- colorzones -------------------------------------------------------------------------
// dt_iop_colorzones_data_t of the pinned release (colorzones.c:116-123).
typedef struct {
    void *curve[3];
    int curve_nodes[3];
    int curve_type[3];
    int channel;
    float lut[3][0x10000];
    int mode;
} OmColorzonesData;

// colorzones.c lookup (411-417) and process_display (425-474).
static float zones_lookup(const float *lut, const float i) {
    const int bin0 = MIN(0xffff, MAX(0, (int)(0x10000 * i)));
    const int bin1 = MIN(0xffff, MAX(0, (int)(0x10000 * i) + 1));
    const float f = 0x10000 * i - bin0;
    return lut[bin1] * f + lut[bin0] * (1.f - f);
}

static void colorzones_display(dt_iop_module_t *self, dt_dev_pixelpipe_iop_t *piece, const float *ivoid, float *ovoid,
                               const dt_iop_roi_t *roi_in, const dt_iop_roi_t *roi_out) {
    const OmColorzonesData *d = piece->data;
    const int ch = piece->colors;
    const float normalize_C = 1.f / (128.0f * M_SQRT2_F);
    const int display_channel = CLAMP(state.channel, 0, 2);
    dt_iop_image_copy_by_size(ovoid, ivoid, roi_out->width, roi_out->height, ch);
    DT_OMP_FOR()
    for (size_t k = 0; k < (size_t)roi_out->width * roi_out->height; k++) {
        const float *in = ivoid + ch * k;
        float *out = ovoid + ch * k;
        dt_aligned_pixel_t LCh;
        dt_Lab_2_LCH(in, LCh);
        float select = 0.0f;
        switch (d->channel) {
        case 0: select = LCh[0] * 0.01f; break;
        case 1: select = LCh[1] * normalize_C; break;
        default: select = LCh[2]; break;
        }
        select = CLAMP(select, 0.f, 1.f);
        out[3] = fabsf(zones_lookup(d->lut[display_channel], select) - .5f) * 4.f;
        out[3] = CLAMP(out[3], 0.f, 1.f);
    }
    piece->pipe->mask_display = DT_DEV_PIXELPIPE_DISPLAY_MASK;
    piece->pipe->bypass_blendif = TRUE;
}

// ---- colorbalancergb --------------------------------------------------------------------
// colorbalancergb.c opacity_masks (551-577).
static inline void cb_opacity_masks(const float x, const float shadows_weight, const float highlights_weight,
                                    const float midtones_weight, const float mask_grey_fulcrum,
                                    dt_aligned_pixel_t output) {
    const float x_offset = (x - mask_grey_fulcrum);
    const float x_offset_norm = x_offset / mask_grey_fulcrum;
    const float alpha = 1.f / (1.f + expf(x_offset_norm * shadows_weight));
    const float beta = 1.f / (1.f + expf(-x_offset_norm * highlights_weight));
    const float alpha_comp = 1.f - alpha;
    const float beta_comp = 1.f - beta;
    const float gamma = expf(-sqf(x_offset) * midtones_weight / 4.f) * sqf(alpha_comp) * sqf(beta_comp) * 8.f;
    output[0] = alpha;
    output[1] = gamma;
    output[2] = beta;
    output[3] = 0.f;
}

// The module's result, then its display branch (process 643-650, 662-704, 905-930): the pixel
// over the checkerboard by the opacity of the selected luminance mask, alpha opaque. The
// checker size is in pipe pixels (DT_PIXEL_APPLY_DPI with a factor of 1 headless).
static void colorbalancergb_display(dt_iop_module_t *self, dt_dev_pixelpipe_iop_t *piece, const float *in, float *out,
                                    const dt_iop_roi_t *roi_in, const dt_iop_roi_t *roi_out) {
    self->so->process_plain(self, piece, in, out, roi_in, roi_out);
    const dt_iop_order_iccprofile_info_t *const work_profile = dt_ioppr_get_pipe_current_profile_info(self, piece->pipe);
    if (!work_profile)
        return;
    dt_colormatrix_t temp = {{0.0f}}, input_matrix = {{0.0f}}, input_matrix_trans;
    dt_colormatrix_mul(temp, XYZ_D50_to_D65_CAT16, work_profile->matrix_in);
    dt_colormatrix_mul(input_matrix, XYZ_D65_to_LMS_2006_D65, temp);
    dt_colormatrix_transpose(input_matrix_trans, input_matrix);
    // commit_params (1147-1167)
    const float shadows_weight = 2.f + *P(self, "shadows_weight") * 2.f;
    const float highlights_weight = 2.f + *P(self, "highlights_weight") * 2.f;
    const float midtones_weight =
        sqf(shadows_weight) * sqf(highlights_weight) / (sqf(shadows_weight) + sqf(highlights_weight));
    const float mask_grey_fulcrum = powf(*P(self, "mask_grey_fulcrum"), 0.4101205819200422f);
    const size_t checker_1 = MAX(state.checker_size, 2);
    const size_t checker_2 = 2 * checker_1;
    const size_t npixels = (size_t)roi_out->height * roi_out->width;
    const size_t out_width = roi_out->width;
    const int type = CLAMP(state.type, 0, 2);
    dt_aligned_pixel_t checker_color_1, checker_color_2;
    copy_pixel(checker_color_1, state.checker_1);
    copy_pixel(checker_color_2, state.checker_2);
    DT_OMP_FOR()
    for (size_t k = 0; k < 4 * npixels; k += 4) {
        dt_aligned_pixel_t RGB, LMS, Yrg = {0.f}, Ych = {0.f}, opacities;
        copy_pixel(RGB, in + k);
        dt_vector_clipneg(RGB);
        dt_apply_transposed_color_matrix(RGB, input_matrix_trans, LMS);
        LMS_to_Yrg(LMS, Yrg);
        Yrg_to_Ych(Yrg, Ych);
        Ych[0] = MAX(Ych[0], 0.f);
        cb_opacity_masks(powf(Ych[0], 0.4101205819200422f), shadows_weight, highlights_weight, midtones_weight,
                         mask_grey_fulcrum, opacities);
        dt_aligned_pixel_t color;
        const size_t i = (k / 4) / out_width;
        const size_t j = (k / 4) % out_width;
        if (i % checker_1 < i % checker_2)
            copy_pixel(color, (j % checker_1 < j % checker_2) ? checker_color_2 : checker_color_1);
        else
            copy_pixel(color, (j % checker_1 < j % checker_2) ? checker_color_1 : checker_color_2);
        const float opacity = opacities[type];
        const float opacity_comp = 1.0f - opacity;
        for (int c = 0; c < 4; c++)
            out[k + c] = opacity_comp * color[c] + opacity * out[k + c];
        out[k + 3] = 1.0f;
    }
}

// ---- colorequal -------------------------------------------------------------------------
// dt_iop_colorequal_gui_data_t of the pinned release (colorequal.c:239-284, NODES 8,
// NUM_CHANNELS 3); process reads only mask_mode from it (989-991).
typedef struct {
    GtkWidget *white_level;
    GtkWidget *sat_red, *sat_orange, *sat_yellow, *sat_green;
    GtkWidget *sat_cyan, *sat_blue, *sat_lavender, *sat_magenta;
    GtkWidget *hue_red, *hue_orange, *hue_yellow, *hue_green;
    GtkWidget *hue_cyan, *hue_blue, *hue_lavender, *hue_magenta;
    GtkWidget *bright_red, *bright_orange, *bright_yellow, *bright_green;
    GtkWidget *bright_cyan, *bright_blue, *bright_lavender, *bright_magenta;
    GtkWidget *smoothing_hue, *threshold, *contrast;
    GtkWidget *chroma_size, *param_size, *use_filter;
    GtkWidget *hue_shift;
    GtkWidget *sat_sliders[8];
    GtkWidget *hue_sliders[8];
    GtkWidget *bright_sliders[8];
    int page_num;
    GtkNotebook *notebook;
    GtkDrawingArea *area;
    GtkStack *stack;
    dt_gui_collapsible_section_t cs;
    float *LUT;
    int channel;
    dt_iop_order_iccprofile_info_t *work_profile;
    dt_iop_order_iccprofile_info_t *white_adapted_profile;
    unsigned char *b_data[3];
    cairo_surface_t *b_surface[3];
    float graph_height;
    float max_saturation;
    gboolean gradients_cached;
    float *gamut_LUT;
    int mask_mode;
    gboolean dragging;
    gboolean on_node;
    int selected;
    float points[8 + 1][2];
} OmColorequalGui;

static void colorequal_display(dt_iop_module_t *self, dt_dev_pixelpipe_iop_t *piece, const float *in, float *out,
                               const dt_iop_roi_t *roi_in, const dt_iop_roi_t *roi_out) {
    OmColorequalGui g = {0};
    // _masking_callback_p / _t: channel + 1, or GRAD_SWITCH (4) + channel + 1
    g.mask_mode = state.type;
    void *saved = self->gui_data;
    self->gui_data = &g;
    self->so->process_plain(self, piece, in, out, roi_in, roi_out);
    self->gui_data = saved;
}

// ---- modules that only read a flag of their GUI data ----------------------------------------
// The module's own process with a stand-in GUI data block of the pinned release's layout that
// holds the flags its buttons set. `focus` also makes the module the focused one of an attached
// GUI for the call (dt_iop_has_focus, imageop.c:2378; demosaic's dev->gui_attached test); the
// worker renders on its own thread, nothing else reads the develop context meanwhile.
static void process_with_gui(dt_iop_module_t *self, dt_dev_pixelpipe_iop_t *piece, const float *in, float *out,
                             const dt_iop_roi_t *roi_in, const dt_iop_roi_t *roi_out, void *gui, gboolean focus) {
    dt_develop_t *dev = self->dev;
    void *saved = self->gui_data;
    const gboolean attached = dev->gui_attached;
    dt_iop_module_t *focused = dev->gui_module;
    self->gui_data = gui;
    if (focus) {
        dev->gui_attached = TRUE;
        dev->gui_module = self;
    }
    self->so->process_plain(self, piece, in, out, roi_in, roi_out);
    dev->gui_attached = attached;
    dev->gui_module = focused;
    self->gui_data = saved;
}

// dt_iop_highlights_gui_data_t (highlights.c:124-137); process reads hlr_mask_mode (857-875, 966):
// state.type is dt_highlights_mask_t (1 combine, 2 candidating, 3 strength, 4 clipped).
typedef struct {
    GtkWidget *widgets[10];
    int hlr_mask_mode;
} OmHighlightsGui;

// dt_iop_demosaic_gui_data_t (demosaic.c:159-184); process reads the three mask flags with
// dev->gui_attached (679-696). With GUI data the capture sharpening radius and threshold it
// computes are also written to the module's parameters (capture.c:787-822, for the sliders to
// show); the parameters are put back, the pipe keeps the computed values as without a GUI.
typedef struct {
    GtkWidget *widgets[15];
    dt_gui_collapsible_section_t capture;
    gboolean cs_mask, dual_mask, cs_boost_mask;
    gboolean autoradius, autothrs;
    float new_radius, new_thrs;
} OmDemosaicGui;

// dt_iop_lens_gui_data_t (lens.cc:173-194); process reads vig_masking (3009, 3088).
typedef struct {
    GtkWidget *widgets[25];
    dt_gui_collapsible_section_t fine_tune, vignette;
    GtkLabel *message;
    GtkBox *hbox1;
    int corrections_done;
    gboolean lensfun_trouble;
    gboolean vig_masking;
    const void *camera;
} OmLensGui;

// dt_iop_retouch_gui_data_t (retouch.c:119-172); process reads the three display flags of the
// focused module (3779-3823) and stores the first visible scale. state.type: 1 "display masks",
// 2 "display wavelet scale" (the scale is the module's curr_scale, the levels its
// preview_levels), 4 "temporarily switch off shapes".
typedef struct {
    int copied_scale;
    gboolean mask_display, suppress_mask, display_wavelet_scale;
    int displayed_wavelet_scale, preview_auto_levels;
    float preview_levels[3];
    int first_scale_visible;
    void *widgets[40];
} OmRetouchGui;

static void flag_display(dt_iop_module_t *self, dt_dev_pixelpipe_iop_t *piece, const float *in, float *out,
                         const dt_iop_roi_t *roi_in, const dt_iop_roi_t *roi_out) {
    if (dt_iop_module_is(self, "highlights")) {
        OmHighlightsGui g = {0};
        g.hlr_mask_mode = state.type;
        process_with_gui(self, piece, in, out, roi_in, roi_out, &g, FALSE);
    } else if (dt_iop_module_is(self, "demosaic")) {
        OmDemosaicGui g = {0};
        g.dual_mask = state.type == 1;
        g.cs_mask = state.type == 2;
        g.cs_boost_mask = state.type == 3;
        void *params = g_memdup2(self->params, self->params_size);
        process_with_gui(self, piece, in, out, roi_in, roi_out, &g, TRUE);
        memcpy(self->params, params, self->params_size);
        g_free(params);
    } else if (dt_iop_module_is(self, "lens")) {
        OmLensGui g = {0};
        g.vig_masking = TRUE;
        process_with_gui(self, piece, in, out, roi_in, roi_out, &g, FALSE);
    } else {
        OmRetouchGui g = {0};
        g.mask_display = (state.type & 1) != 0;
        g.display_wavelet_scale = (state.type & 2) != 0;
        g.suppress_mask = (state.type & 4) != 0;
        process_with_gui(self, piece, in, out, roi_in, roi_out, &g, TRUE);
    }
}

// ---- the wrapper ------------------------------------------------------------------------
static void display_process(dt_iop_module_t *self, dt_dev_pixelpipe_iop_t *piece, const void *const i, void *const o,
                            const dt_iop_roi_t *const roi_in, const dt_iop_roi_t *const roi_out) {
    if (!is_target(self, piece)) {
        self->so->process_plain(self, piece, i, o, roi_in, roi_out);
        return;
    }
    if (dt_iop_module_is(self, "highlights") || dt_iop_module_is(self, "demosaic") ||
        dt_iop_module_is(self, "lens") || dt_iop_module_is(self, "retouch")) {
        flag_display(self, piece, i, o, roi_in, roi_out);
        return;
    }
    if (dt_iop_module_is(self, "toneequal"))
        toneequal_display(self, piece, i, o, roi_in, roi_out);
    else if (dt_iop_module_is(self, "filmicrgb"))
        filmicrgb_display(self, piece, i, o, roi_in, roi_out);
    else if (dt_iop_module_is(self, "colorzones"))
        colorzones_display(self, piece, i, o, roi_in, roi_out);
    else if (dt_iop_module_is(self, "colorbalancergb"))
        colorbalancergb_display(self, piece, i, o, roi_in, roi_out);
    else if (dt_iop_module_is(self, "colorequal"))
        colorequal_display(self, piece, i, o, roi_in, roi_out);
    else
        self->so->process_plain(self, piece, i, o, roi_in, roi_out);
}

// Every module runs its own process again, except the displayed one.
static void unwrap_all(OmEngine *engine) {
    if (!engine->loaded)
        return;
    for (GList *l = engine->dev.iop; l; l = g_list_next(l)) {
        dt_iop_module_t *m = l->data;
        if (m->process_plain == display_process)
            m->process_plain = m->so->process_plain;
    }
}

// colorbalancergb gui_init (2052-2068): its keys with their defaults, unless already set.
// Headless no gui_init runs, and commit_params' dt_conf_get_float would store 0 for a missing key.
void om_module_display_init(void) {
    static const struct { const char *key; float value; } defaults[] = {
        {"plugins/darkroom/colorbalancergb/checker1/red", 1.0f},
        {"plugins/darkroom/colorbalancergb/checker1/green", 1.0f},
        {"plugins/darkroom/colorbalancergb/checker1/blue", 1.0f},
        {"plugins/darkroom/colorbalancergb/checker2/red", 0.18f},
        {"plugins/darkroom/colorbalancergb/checker2/green", 0.18f},
        {"plugins/darkroom/colorbalancergb/checker2/blue", 0.18f},
    };
    for (size_t i = 0; i < G_N_ELEMENTS(defaults); i++)
        if (!dt_conf_key_exists(defaults[i].key))
            dt_conf_set_float(defaults[i].key, defaults[i].value);
    if (!dt_conf_key_exists("plugins/darkroom/colorbalancergb/checker/size"))
        dt_conf_set_int("plugins/darkroom/colorbalancergb/checker/size", 8);
}

// The value the GUI keeps under darktable's key, else darktable's configuration.
static float conf_float(const OmToolContext *ctx, const char *key) {
    if (ctx->gui && json_object_has_member(ctx->gui, key))
        return om_tool_gui(ctx, key, 0);
    return dt_conf_get_float(key);
}

static int display_mask(OmToolContext *ctx) {
    dt_iop_module_t *m = ctx->module;
    const gboolean on = om_tool_gui(ctx, "display", 0) > 0.5;
    dt_dev_pixelpipe_t *pipe = ctx->dev->full.pipe;
    dt_iop_module_t *previous = display_module(ctx->engine);
    if (previous)
        dt_dev_pixelpipe_cache_invalidate_later(pipe, previous->iop_order, "omalux module display: ");
    unwrap_all(ctx->engine);
    memset(state.operation, 0, sizeof(state.operation));
    if (on) {
        g_strlcpy(state.operation, m->op, sizeof(state.operation));
        state.instance = m->multi_priority;
        state.type = (int)om_tool_gui(ctx, "type", 0);
        state.channel = (int)om_tool_gui(ctx, "channel", 0);
        // colorbalancergb commit_params (1093-1103), "mask preview settings"
        const char *const keys[2][3] = {{"plugins/darkroom/colorbalancergb/checker1/red",
                                         "plugins/darkroom/colorbalancergb/checker1/green",
                                         "plugins/darkroom/colorbalancergb/checker1/blue"},
                                        {"plugins/darkroom/colorbalancergb/checker2/red",
                                         "plugins/darkroom/colorbalancergb/checker2/green",
                                         "plugins/darkroom/colorbalancergb/checker2/blue"}};
        for (int c = 0; c < 3; c++) {
            state.checker_1[c] = CLAMP(conf_float(ctx, keys[0][c]), 0.f, 1.f);
            state.checker_2[c] = CLAMP(conf_float(ctx, keys[1][c]), 0.f, 1.f);
        }
        state.checker_1[3] = state.checker_2[3] = 1.f;
        state.checker_size = MAX((int)conf_float(ctx, "plugins/darkroom/colorbalancergb/checker/size"), 2);
        m->process_plain = display_process;
        // One preview at a time: the blend section's mask display of any module goes off
        // (darktable: another module takes the focus).
        om_blend_display_reset(ctx->engine);
    }
    dt_dev_pixelpipe_cache_invalidate_later(pipe, m->iop_order, "omalux module display: ");
    pipe->changed |= DT_DEV_PIPE_SYNCH;
    om_tool_set_gui(ctx, "display", on);
    return 0;
}

void om_module_display_reset(OmEngine *engine) {
    unwrap_all(engine);
    memset(state.operation, 0, sizeof(state.operation));
    state.swapped = FALSE;
}

gboolean om_module_display_active(void) {
    return shown();
}

void om_module_display_before_render(OmEngine *engine) {
    dt_iop_module_t *m = display_module(engine);
    state.swapped = FALSE;
    if (!m)
        return;
    dt_dev_pixelpipe_t *pipe = engine->dev.full.pipe;
    if (pipe->loading)
        return;
    if (m->process_plain != display_process)
        m->process_plain = display_process;
    // Synchronise first (commit_params sets process_cl_ready again), then keep the piece on
    // the CPU, where the wrapper runs.
    if (pipe->changed != DT_DEV_PIPE_UNCHANGED)
        dt_dev_pixelpipe_change(pipe, &engine->dev);
    dt_dev_pixelpipe_iop_t *piece = dt_dev_distort_get_iop_pipe(&engine->dev, pipe, m);
    if (!piece)
        return;
    piece->process_cl_ready = FALSE;
    // colorzones: the focused module's own blending is bypassed while its selection is shown
    // (pixelpipe_hb.c _piece_wants_blending 1337, bypass_blendif with dt_iop_has_focus).
    dt_develop_blend_params_t *bp = piece->blendop_data;
    if (dt_iop_module_is(m, "colorzones") && bp && (bp->mask_mode & DEVELOP_MASK_ENABLED)) {
        state.saved = *bp;
        bp->mask_mode = DEVELOP_MASK_DISABLED;
        state.swapped = TRUE;
    }
}

void om_module_display_after_render(OmEngine *engine) {
    if (!state.swapped)
        return;
    dt_iop_module_t *m = display_module(engine);
    dt_dev_pixelpipe_iop_t *piece = m ? dt_dev_distort_get_iop_pipe(&engine->dev, engine->dev.full.pipe, m) : NULL;
    if (piece && piece->blendop_data)
        *(dt_develop_blend_params_t *)piece->blendop_data = state.saved;
    state.swapped = FALSE;
}

const OmToolSpec om_tools_module_display[] = {
    {"toneequal", "display_mask", OM_TOOL_BUTTON, -1, 0, display_mask},
    // filmic only redraws (show_mask_callback 2716); the others switch the module on.
    {"filmicrgb", "display_mask", OM_TOOL_BUTTON, -1, OM_TOOL_KEEP_OFF, display_mask},
    {"colorzones", "display_mask", OM_TOOL_BUTTON, -1, 0, display_mask},
    {"colorbalancergb", "display_mask", OM_TOOL_BUTTON, -1, 0, display_mask},
    {"colorequal", "display_mask", OM_TOOL_BUTTON, -1, OM_TOOL_KEEP_OFF, display_mask},
    // the buttons only redraw (highlights.c _quad_callback 1104, demosaic.c 1627-1668, lens.cc
    // _visualize_callback 4358, retouch.c rt_showmask_callback 2040)
    {"highlights", "display_mask", OM_TOOL_BUTTON, -1, OM_TOOL_KEEP_OFF, display_mask},
    {"demosaic", "display_mask", OM_TOOL_BUTTON, -1, OM_TOOL_KEEP_OFF, display_mask},
    {"lens", "display_mask", OM_TOOL_BUTTON, -1, OM_TOOL_KEEP_OFF, display_mask},
    {"retouch", "display_mask", OM_TOOL_BUTTON, -1, OM_TOOL_KEEP_OFF, display_mask},
    {NULL, NULL, 0, 0, 0, NULL},
};
