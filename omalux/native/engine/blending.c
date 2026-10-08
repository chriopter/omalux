// SPDX-License-Identifier: GPL-3.0-or-later
// darktable's per-module blend section (develop/blend_gui.c, release-5.6.1) without GTK.
// Every rule here is a port of a GUI callback, cited by function name; the parameters are
// darktable's own dt_develop_blend_params_t (develop/blend.h), committed with
// dt_iop_commit_blend_params so raster-mask links and the default colour space are handled as
// darktable handles them.
#include "blending.h"
#include "masks_api.h"
#include "common/image.h"
#include "develop/masks.h"
#include <math.h>

// One tab of the parametric mask (blend_gui.c Lab_channels, rgb_channels, rgbj_channels).
typedef struct {
    const char *label, *name;
    int in;             // input channel; the output channel is in + 4
    gboolean boost;     // boost_factor_enabled
    float boost_offset; // shown boost factor = stored - offset
    const char *scale;  // how darktable prints the markers: default, ab, hue
    float increment;
} OmBlendChannel;

static const OmBlendChannel lab_channels[] = {
    {"L", "lightness", DEVELOP_BLENDIF_L_in, TRUE, 0.0f, "default", 1.0f / 100.0f},
    {"a", "green/red", DEVELOP_BLENDIF_A_in, TRUE, 0.0f, "ab", 1.0f / 256.0f},
    {"b", "blue/yellow", DEVELOP_BLENDIF_B_in, TRUE, 0.0f, "ab", 1.0f / 256.0f},
    {"C", "saturation", DEVELOP_BLENDIF_C_in, TRUE, 0.0f, "default", 1.0f / 100.0f},
    {"h", "hue", DEVELOP_BLENDIF_h_in, FALSE, 0.0f, "hue", 1.0f / 360.0f},
    {NULL}};
static const OmBlendChannel rgb_channels[] = {
    {"g", "gray", DEVELOP_BLENDIF_GRAY_in, TRUE, 0.0f, "default", 1.0f / 255.0f},
    {"R", "red", DEVELOP_BLENDIF_RED_in, TRUE, 0.0f, "default", 1.0f / 255.0f},
    {"G", "green", DEVELOP_BLENDIF_GREEN_in, TRUE, 0.0f, "default", 1.0f / 255.0f},
    {"B", "blue", DEVELOP_BLENDIF_BLUE_in, TRUE, 0.0f, "default", 1.0f / 255.0f},
    {"H", "hue", DEVELOP_BLENDIF_H_in, FALSE, 0.0f, "hue", 1.0f / 360.0f},
    {"S", "chroma", DEVELOP_BLENDIF_S_in, FALSE, 0.0f, "default", 1.0f / 100.0f},
    {"L", "luminance", DEVELOP_BLENDIF_l_in, FALSE, 0.0f, "default", 1.0f / 100.0f},
    {NULL}};
// Jz and Cz carry an offset so their initial boost reads 0 (_blend_init_blendif_boost_parameters).
static const OmBlendChannel rgbj_channels[] = {
    {"g", "gray", DEVELOP_BLENDIF_GRAY_in, TRUE, 0.0f, "default", 1.0f / 255.0f},
    {"R", "red", DEVELOP_BLENDIF_RED_in, TRUE, 0.0f, "default", 1.0f / 255.0f},
    {"G", "green", DEVELOP_BLENDIF_GREEN_in, TRUE, 0.0f, "default", 1.0f / 255.0f},
    {"B", "blue", DEVELOP_BLENDIF_BLUE_in, TRUE, 0.0f, "default", 1.0f / 255.0f},
    {"Jz", "luminance", DEVELOP_BLENDIF_Jz_in, TRUE, -6.64385619f, "default", 1.0f / 100.0f},
    {"Cz", "chroma", DEVELOP_BLENDIF_Cz_in, TRUE, -6.64385619f, "default", 1.0f / 100.0f},
    {"hz", "hue", DEVELOP_BLENDIF_hz_in, FALSE, 0.0f, "hue", 1.0f / 360.0f},
    {NULL}};

gboolean om_blend_path(const char *path) {
    return path && g_str_has_prefix(path, "blend.");
}

static gboolean supports_blending(dt_iop_module_t *module) {
    return (module->flags() & IOP_FLAGS_SUPPORTS_BLENDING) && !module->hide_enable_button;
}
// dt_iop_gui_init_blending: bd->masks_support and bd->blendif_support.
static gboolean masks_support(dt_iop_module_t *module) {
    return !(module->flags() & IOP_FLAGS_NO_MASKS);
}
static gboolean blendif_support(dt_iop_module_t *module) {
    const dt_iop_colorspace_type_t cst = module->blend_colorspace(module, NULL, NULL);
    return cst == IOP_CS_LAB || cst == IOP_CS_RGB;
}

// The colour space the blend section works in (dt_iop_gui_update_blending, bd->csp).
static dt_develop_blend_colorspace_t blend_csp(dt_iop_module_t *module, const dt_develop_blend_params_t *bp) {
    const dt_develop_blend_colorspace_t def = dt_develop_blend_default_module_blend_colorspace(module);
    switch (def) {
    case DEVELOP_BLEND_CS_RAW:
        return DEVELOP_BLEND_CS_RAW;
    case DEVELOP_BLEND_CS_LAB:
    case DEVELOP_BLEND_CS_RGB_DISPLAY:
    case DEVELOP_BLEND_CS_RGB_SCENE:
        switch (bp->blend_cst) {
        case DEVELOP_BLEND_CS_LAB:
        case DEVELOP_BLEND_CS_RGB_DISPLAY:
        case DEVELOP_BLEND_CS_RGB_SCENE:
            return bp->blend_cst;
        default:
            return def;
        }
    default:
        return DEVELOP_BLEND_CS_NONE;
    }
}

static const OmBlendChannel *channels_of(dt_develop_blend_colorspace_t csp) {
    switch (csp) {
    case DEVELOP_BLEND_CS_LAB:
        return lab_channels;
    case DEVELOP_BLEND_CS_RGB_DISPLAY:
        return rgb_channels;
    case DEVELOP_BLEND_CS_RGB_SCENE:
        return rgbj_channels;
    default:
        return NULL;
    }
}

static const OmBlendChannel *channel_of(dt_develop_blend_colorspace_t csp, int in) {
    for (const OmBlendChannel *c = channels_of(csp); c && c->label; ++c)
        if (c->in == in)
            return c;
    return NULL;
}

// _blendif_blend_parameter_enabled: the fulcrum exists for some modes in RGB (scene).
static gboolean fulcrum_enabled(dt_develop_blend_colorspace_t csp, uint32_t mode) {
    if (csp != DEVELOP_BLEND_CS_RGB_SCENE)
        return FALSE;
    switch (mode & ~DEVELOP_BLEND_REVERSE) {
    case DEVELOP_BLEND_ADD:
    case DEVELOP_BLEND_MULTIPLY:
    case DEVELOP_BLEND_SUBTRACT:
    case DEVELOP_BLEND_SUBTRACT_INVERSE:
    case DEVELOP_BLEND_DIVIDE:
    case DEVELOP_BLEND_DIVIDE_INVERSE:
    case DEVELOP_BLEND_RGB_R:
    case DEVELOP_BLEND_RGB_G:
    case DEVELOP_BLEND_RGB_B:
        return TRUE;
    default:
        return FALSE;
    }
}

// The blend modes the combobox offers in this colour space, in darktable's order and
// sections (dt_iop_gui_update_blending, _add_blendmode_combo over dt_develop_blend_mode_names).
typedef struct {
    const char *section;
    int start, end;
} OmModeRange;
static const OmModeRange display_modes[] = {
    {"normal & difference", DEVELOP_BLEND_NORMAL2, DEVELOP_BLEND_DIFFERENCE2},
    {NULL, DEVELOP_BLEND_BOUNDED, DEVELOP_BLEND_BOUNDED},
    {"lighten", DEVELOP_BLEND_LIGHTEN, DEVELOP_BLEND_LIGHTEN},
    {NULL, DEVELOP_BLEND_ADD, DEVELOP_BLEND_ADD},
    {NULL, DEVELOP_BLEND_SCREEN, DEVELOP_BLEND_SCREEN},
    {"darken", DEVELOP_BLEND_DARKEN, DEVELOP_BLEND_DARKEN},
    {NULL, DEVELOP_BLEND_SUBTRACT, DEVELOP_BLEND_SUBTRACT},
    {NULL, DEVELOP_BLEND_MULTIPLY, DEVELOP_BLEND_MULTIPLY},
    {"contrast enhancing", DEVELOP_BLEND_OVERLAY, DEVELOP_BLEND_PINLIGHT},
    {NULL, 0, 0}};
static const OmModeRange lab_colour_modes[] = {
    {"color channel", DEVELOP_BLEND_LAB_LIGHTNESS, DEVELOP_BLEND_LAB_COLOR},
    {NULL, DEVELOP_BLEND_HUE, DEVELOP_BLEND_COLORADJUST},
    {"chromaticity & lightness", DEVELOP_BLEND_LIGHTNESS, DEVELOP_BLEND_CHROMATICITY},
    {NULL, 0, 0}};
static const OmModeRange rgb_colour_modes[] = {
    {"color channel", DEVELOP_BLEND_RGB_R, DEVELOP_BLEND_HSV_COLOR},
    {NULL, DEVELOP_BLEND_HUE, DEVELOP_BLEND_COLORADJUST},
    {"chromaticity & lightness", DEVELOP_BLEND_LIGHTNESS, DEVELOP_BLEND_CHROMATICITY},
    {NULL, 0, 0}};
static const OmModeRange scene_modes[] = {
    {"normal & arithmetic", DEVELOP_BLEND_NORMAL2, DEVELOP_BLEND_DIFFERENCE2},
    {NULL, DEVELOP_BLEND_MULTIPLY, DEVELOP_BLEND_HARMONIC_MEAN},
    {"color channel", DEVELOP_BLEND_RGB_R, DEVELOP_BLEND_RGB_B},
    {"chromaticity & lightness", DEVELOP_BLEND_LIGHTNESS, DEVELOP_BLEND_CHROMATICITY},
    {NULL, 0, 0}};

static const char *mode_label(const dt_introspection_type_enum_tuple_t *item) {
    const char *text = item->description ? item->description : item->name;
    const char *bar = text ? strchr(text, '|') : NULL; // NC_("blendmode", …) is "blendmode|…"
    return bar ? bar + 1 : text;
}

typedef void (*OmModeVisit)(int value, const char *label, const char *section, gpointer data);
static void visit_ranges(const OmModeRange *ranges, OmModeVisit visit, gpointer data) {
    for (const OmModeRange *r = ranges; r->start || r->end; ++r) {
        const dt_introspection_type_enum_tuple_t *item = dt_develop_blend_mode_names;
        while (item->name && item->value != r->start)
            ++item;
        const char *section = r->section;
        for (; item->name; ++item) {
            visit(item->value, mode_label(item), section, data);
            section = NULL;
            if (item->value == r->end)
                break;
        }
    }
}
static void visit_modes(dt_develop_blend_colorspace_t csp, OmModeVisit visit, gpointer data) {
    if (csp == DEVELOP_BLEND_CS_LAB || csp == DEVELOP_BLEND_CS_RGB_DISPLAY || csp == DEVELOP_BLEND_CS_RAW) {
        visit_ranges(display_modes, visit, data);
        if (csp == DEVELOP_BLEND_CS_LAB)
            visit_ranges(lab_colour_modes, visit, data);
        else if (csp == DEVELOP_BLEND_CS_RGB_DISPLAY)
            visit_ranges(rgb_colour_modes, visit, data);
    } else if (csp == DEVELOP_BLEND_CS_RGB_SCENE)
        visit_ranges(scene_modes, visit, data);
}
typedef struct {
    int value;
    gboolean found;
} OmModeSearch;
static void find_mode(int value, const char *label, const char *section, gpointer data) {
    OmModeSearch *search = data;
    if (value == search->value)
        search->found = TRUE;
}
static gboolean mode_offered(dt_develop_blend_colorspace_t csp, int value) {
    OmModeSearch search = {value, FALSE};
    visit_modes(csp, find_mode, &search);
    return search.found;
}
static void add_mode(int value, const char *label, const char *section, gpointer data) {
    JsonObject *entry = json_object_new();
    json_object_set_int_member(entry, "value", value);
    json_object_set_string_member(entry, "label", label);
    if (section)
        json_object_set_string_member(entry, "section", section);
    json_array_add_object_element(data, entry);
}

// _blendif_are_output_channels_used
static gboolean outputs_used(const dt_develop_blend_params_t *bp, dt_develop_blend_colorspace_t csp) {
    const gboolean inclusive = bp->mask_combine & DEVELOP_COMBINE_INCL;
    const uint32_t mask = csp == DEVELOP_BLEND_CS_LAB
                              ? DEVELOP_BLENDIF_Lab_MASK & DEVELOP_BLENDIF_OUTPUT_MASK
                              : DEVELOP_BLENDIF_RGB_MASK & DEVELOP_BLENDIF_OUTPUT_MASK;
    const uint32_t active = bp->blendif & mask;
    const uint32_t inverted = (bp->blendif >> 16) ^ (inclusive ? mask : 0);
    return active || (inverted & ~bp->blendif & mask);
}

// _blendif_clean_output_channels, applied while the output channels are hidden.
static void clean_outputs(dt_develop_blend_params_t *d, dt_develop_blend_colorspace_t csp) {
    const uint32_t mask = csp == DEVELOP_BLEND_CS_LAB
                              ? DEVELOP_BLENDIF_Lab_MASK & DEVELOP_BLENDIF_OUTPUT_MASK
                              : DEVELOP_BLENDIF_RGB_MASK & DEVELOP_BLENDIF_OUTPUT_MASK;
    const uint32_t inversion = d->mask_combine & DEVELOP_COMBINE_INCL ? (mask << 16) : 0;
    d->blendif = (d->blendif & ~(mask | (mask << 16))) | inversion;
    for (int ch = 0; ch < DEVELOP_BLENDIF_SIZE; ++ch)
        if (DEVELOP_BLENDIF_OUTPUT_MASK & (1 << ch)) {
            d->blendif_parameters[ch * 4 + 0] = 0.0f;
            d->blendif_parameters[ch * 4 + 1] = 0.0f;
            d->blendif_parameters[ch * 4 + 2] = 1.0f;
            d->blendif_parameters[ch * 4 + 3] = 1.0f;
        }
}

// _blendop_blendif_sliders_callback: a channel is processed unless its range spans everything.
static void update_channel_bit(dt_develop_blend_params_t *bp, int ch) {
    const float *p = &bp->blendif_parameters[4 * ch];
    if (p[1] == 0.0f && p[2] == 1.0f)
        bp->blendif &= ~(1u << ch);
    else
        bp->blendif |= (1u << ch);
}

static gboolean indexed(const char *path, const char *name, int *index) {
    const size_t n = strlen(name);
    if (strncmp(path, name, n) || path[n] != '[' || !g_ascii_isdigit(path[n + 1]))
        return FALSE;
    char *end = NULL;
    const gint64 i = g_ascii_strtoll(path + n + 1, &end, 10);
    if (!end || strcmp(end, "]"))
        return FALSE;
    *index = (int)i;
    return TRUE;
}

static gboolean is_bool(double value) {
    return value == 0.0 || value == 1.0;
}

void om_blend_begin(OmBlendEdit *edit, dt_iop_module_t *module) {
    memset(edit, 0, sizeof(*edit));
    edit->params = *module->blend_params;
    // dt_iop_gui_update_blending: output channels are shown once they carry a setting.
    edit->outputs_shown = outputs_used(&edit->params, blend_csp(module, &edit->params));
}

// The raster masks a module offers to later modules (dt_iop_advertise_rastermask).
static dt_iop_module_t *raster_source(dt_iop_module_t *module, const char *operation, int instance, int id) {
    for (GList *it = module->dev->iop; it; it = it->next) {
        dt_iop_module_t *candidate = it->data;
        if (candidate == module)
            return NULL; // darktable lists only modules before this one (_raster_combo_populate)
        if (!strcmp(candidate->op, operation) && candidate->multi_priority == instance &&
            g_hash_table_contains(candidate->raster_mask.source.masks, GINT_TO_POINTER(id)))
            return candidate;
    }
    return NULL;
}

static int set_mask_mode(dt_iop_module_t *module, dt_develop_blend_params_t *bp, double value) {
    const int mode = (int)value;
    if (mode != value)
        return 5;
    const gboolean masks = masks_support(module), blendif = blendif_support(module);
    switch (mode) {
    case DEVELOP_MASK_DISABLED:
    case DEVELOP_MASK_ENABLED:
        break;
    case DEVELOP_MASK_ENABLED | DEVELOP_MASK_MASK:
    case DEVELOP_MASK_ENABLED | DEVELOP_MASK_RASTER:
        if (!masks)
            return 4;
        break;
    case DEVELOP_MASK_ENABLED | DEVELOP_MASK_CONDITIONAL:
        if (!blendif)
            return 4;
        break;
    case DEVELOP_MASK_ENABLED | DEVELOP_MASK_MASK_CONDITIONAL:
        if (!masks || !blendif)
            return 4;
        break;
    default:
        return 5;
    }
    // _blendop_masks_mode_callback
    bp->mask_mode = mode;
    return 0;
}

// _blendif_change_blend_colorspace
static int set_colorspace(dt_iop_module_t *module, dt_develop_blend_params_t *bp, double value) {
    const dt_develop_blend_colorspace_t module_cst = dt_develop_blend_default_module_blend_colorspace(module);
    if (!blendif_support(module) ||
        !(module_cst == DEVELOP_BLEND_CS_LAB || module_cst == DEVELOP_BLEND_CS_RGB_DISPLAY ||
          module_cst == DEVELOP_BLEND_CS_RGB_SCENE))
        return 4;
    dt_develop_blend_colorspace_t cst = (int)value;
    if (cst != value)
        return 5;
    // _blendif_options_callback offers Lab only to modules that work in Lab.
    if (cst == DEVELOP_BLEND_CS_LAB && module_cst != DEVELOP_BLEND_CS_LAB)
        return 4;
    switch (cst) {
    case DEVELOP_BLEND_CS_LAB:
    case DEVELOP_BLEND_CS_RGB_DISPLAY:
    case DEVELOP_BLEND_CS_RGB_SCENE:
        break;
    case DEVELOP_BLEND_CS_NONE:
        cst = module_cst; // "reset to default blend colorspace"
        break;
    default:
        return 5;
    }
    if (cst == bp->blend_cst)
        return 0;
    dt_develop_blend_init_blendif_parameters(bp, cst);
    // The last history item of this module with that colour space supplies its mask settings.
    for (const GList *history = g_list_last(module->dev->history); history;
         history = g_list_previous(history)) {
        const dt_dev_history_item_t *item = history->data;
        if (item->module == module && item->blend_params && item->blend_params->blend_cst == cst) {
            const dt_develop_blend_params_t *hp = item->blend_params;
            bp->blend_mode = hp->blend_mode;
            bp->blend_parameter = hp->blend_parameter;
            bp->blendif = hp->blendif;
            memcpy(bp->blendif_parameters, hp->blendif_parameters, sizeof(hp->blendif_parameters));
            memcpy(bp->blendif_boost_factors, hp->blendif_boost_factors, sizeof(hp->blendif_boost_factors));
            break;
        }
    }
    return 0;
}

// _blendop_blendif_boost_factor_callback, for the tab whose input channel is `in`.
static int set_boost(dt_develop_blend_params_t *bp, dt_develop_blend_colorspace_t csp, int in, double value) {
    const OmBlendChannel *channel = channel_of(csp, in);
    if (!channel || !channel->boost)
        return 4;
    const float shown = CLAMP((float)value, 0.0f, 18.0f);
    for (int in_out = 1; in_out >= 0; --in_out) {
        const int ch = channel->in + 4 * in_out;
        float off = 0.0f;
        if (csp == DEVELOP_BLEND_CS_LAB && (ch == DEVELOP_BLENDIF_A_in || ch == DEVELOP_BLENDIF_A_out ||
                                            ch == DEVELOP_BLENDIF_B_in || ch == DEVELOP_BLENDIF_B_out))
            off = 0.5f;
        const float new_value = shown + channel->boost_offset;
        const float factor = exp2f(bp->blendif_boost_factors[ch]) / exp2f(new_value);
        float *p = &bp->blendif_parameters[4 * ch];
        if (p[0] > 0.0f)
            p[0] = CLIP((p[0] - off) * factor + off);
        if (p[1] > 0.0f)
            p[1] = CLIP((p[1] - off) * factor + off);
        if (p[2] < 1.0f)
            p[2] = CLIP((p[2] - off) * factor + off);
        if (p[3] < 1.0f)
            p[3] = CLIP((p[3] - off) * factor + off);
        if (p[1] == 0.0f && p[2] == 1.0f)
            bp->blendif &= ~(1u << ch);
        bp->blendif_boost_factors[ch] = new_value;
    }
    return 0;
}

// "blend.raster_mask@<operation>/<instance>" = mask id, or "blend.raster_mask" = -1 for none
// (_raster_value_changed_callback).
static int set_raster(OmBlendEdit *edit, dt_iop_module_t *module, const char *path, double value) {
    dt_develop_blend_params_t *bp = &edit->params;
    if (!masks_support(module))
        return 4;
    if (!strcmp(path, "raster_mask")) {
        if (value != -1)
            return 5;
        memset(bp->raster_mask_source, 0, sizeof(bp->raster_mask_source));
        bp->raster_mask_instance = 0;
        bp->raster_mask_id = INVALID_MASKID;
        return 0;
    }
    if (!g_str_has_prefix(path, "raster_mask@"))
        return 3;
    const char *operation = path + strlen("raster_mask@");
    const char *slash = strrchr(operation, '/');
    if (!slash || slash == operation || !g_ascii_isdigit(slash[1]))
        return 3;
    char *op = g_strndup(operation, slash - operation);
    char *end = NULL;
    const int instance = (int)g_ascii_strtoll(slash + 1, &end, 10);
    const int id = (int)value;
    dt_iop_module_t *source = (end && !*end && id == value) ? raster_source(module, op, instance, id) : NULL;
    g_free(op);
    if (!source)
        return 5;
    memset(bp->raster_mask_source, 0, sizeof(bp->raster_mask_source));
    g_strlcpy(bp->raster_mask_source, source->op, sizeof(bp->raster_mask_source));
    bp->raster_mask_instance = source->multi_priority;
    bp->raster_mask_id = id;
    // A source that gets its first user has to keep its mask: process the pipe again.
    edit->reprocess = TRUE;
    return 0;
}

int om_blend_assign(OmBlendEdit *edit, dt_iop_module_t *module, const char *path, double value) {
    if (!om_blend_path(path))
        return 3;
    if (!supports_blending(module))
        return 4;
    if (!isfinite(value))
        return 5;
    path += strlen("blend.");
    dt_develop_blend_params_t *bp = &edit->params;
    const dt_develop_blend_colorspace_t csp = blend_csp(module, bp);
    int index = 0;
    int error = 0;
    if (!strcmp(path, "mask_mode"))
        error = set_mask_mode(module, bp, value);
    else if (!strcmp(path, "blend_cst"))
        error = set_colorspace(module, bp, value);
    else if (!strcmp(path, "blend_mode")) {
        // _blendop_blend_mode_callback: keep the blend order, drop a fulcrum the mode does not use.
        const int mode = (int)value;
        if (mode != value || mode < 0 ||
            (!mode_offered(csp, mode) && mode != (int)(bp->blend_mode & DEVELOP_BLEND_MODE_MASK)))
            return 5;
        if (mode != (int)(bp->blend_mode & DEVELOP_BLEND_MODE_MASK)) {
            bp->blend_mode = (uint32_t)mode | (bp->blend_mode & DEVELOP_BLEND_REVERSE);
            if (!fulcrum_enabled(csp, bp->blend_mode))
                bp->blend_parameter = 0.0f;
        }
    } else if (!strcmp(path, "reverse")) {
        // _blendop_blend_order_clicked
        if (!is_bool(value))
            return 5;
        bp->blend_mode =
            value ? (bp->blend_mode | DEVELOP_BLEND_REVERSE) : (bp->blend_mode & ~DEVELOP_BLEND_REVERSE);
    } else if (!strcmp(path, "blend_parameter"))
        bp->blend_parameter = CLAMP((float)value, -18.0f, 18.0f);
    else if (!strcmp(path, "opacity"))
        bp->opacity = CLAMP((float)value, 0.0f, 100.0f);
    else if (!strcmp(path, "mask_combine")) {
        // _blendop_masks_combine_callback
        const int combine = (int)value;
        if (combine != value || combine < 0 || combine > (DEVELOP_COMBINE_INV | DEVELOP_COMBINE_INCL))
            return 5;
        bp->mask_combine &= ~(DEVELOP_COMBINE_INV | DEVELOP_COMBINE_INCL);
        bp->mask_combine |= combine;
        if (blendif_support(module)) {
            const uint32_t mask =
                csp == DEVELOP_BLEND_CS_LAB ? DEVELOP_BLENDIF_Lab_MASK : DEVELOP_BLENDIF_RGB_MASK;
            const uint32_t unused = mask & ~bp->blendif;
            bp->blendif &= ~(unused << 16);
            if (bp->mask_combine & DEVELOP_COMBINE_INCL)
                bp->blendif |= unused << 16;
            if (!edit->outputs_shown)
                clean_outputs(bp, csp);
        }
    } else if (!strcmp(path, "drawn_polarity")) {
        // _blendop_masks_polarity_callback
        if (!is_bool(value))
            return 5;
        bp->mask_combine = value ? (bp->mask_combine | DEVELOP_COMBINE_MASKS_POS)
                                 : (bp->mask_combine & ~DEVELOP_COMBINE_MASKS_POS);
    } else if (indexed(path, "blendif_parameters", &index)) {
        if (!blendif_support(module))
            return 4;
        if (index < 0 || index >= 4 * DEVELOP_BLENDIF_SIZE)
            return 3;
        bp->blendif_parameters[index] = CLAMP((float)value, 0.0f, 1.0f);
        update_channel_bit(bp, index / 4);
    } else if (indexed(path, "polarity", &index)) {
        // _blendop_blendif_polarity_callback: a set bit is the negative polarity.
        if (!blendif_support(module))
            return 4;
        if (index < 0 || index >= DEVELOP_BLENDIF_SIZE || !is_bool(value))
            return index < 0 || index >= DEVELOP_BLENDIF_SIZE ? 3 : 5;
        bp->blendif = value ? (bp->blendif | (1u << (index + 16))) : (bp->blendif & ~(1u << (index + 16)));
    } else if (indexed(path, "reset_channel", &index)) {
        // A double-click on darktable's gradient slider: markers back to their defaults
        // (value-changed) and the polarity that follows the combine mode (value-reset).
        if (!blendif_support(module))
            return 4;
        if (index < 0 || index >= DEVELOP_BLENDIF_SIZE)
            return 3;
        memcpy(&bp->blendif_parameters[4 * index],
               &module->default_blendop_params->blendif_parameters[4 * index], 4 * sizeof(float));
        update_channel_bit(bp, index);
        if (bp->mask_combine & DEVELOP_COMBINE_INCL)
            bp->blendif |= 1u << (16 + index);
        else
            bp->blendif &= ~(1u << (16 + index));
    } else if (indexed(path, "boost_factor", &index)) {
        if (!blendif_support(module))
            return 4;
        error = set_boost(bp, csp, index, value);
    } else if (!strcmp(path, "invert_all")) {
        // _blendop_blendif_invert
        if (!blendif_support(module))
            return 4;
        const uint32_t toggle = csp == DEVELOP_BLEND_CS_LAB ? DEVELOP_BLENDIF_Lab_MASK << 16
                                : (csp == DEVELOP_BLEND_CS_RGB_DISPLAY || csp == DEVELOP_BLEND_CS_RGB_SCENE)
                                    ? DEVELOP_BLENDIF_RGB_MASK << 16
                                    : 0;
        bp->blendif ^= toggle;
        bp->mask_combine ^= DEVELOP_COMBINE_MASKS_POS;
        bp->mask_combine ^= DEVELOP_COMBINE_INCL;
    } else if (!strcmp(path, "reset_parametric")) {
        // _blendop_blendif_reset
        if (!blendif_support(module))
            return 4;
        bp->blendif = module->default_blendop_params->blendif;
        memcpy(bp->blendif_parameters, module->default_blendop_params->blendif_parameters,
               sizeof(bp->blendif_parameters));
        bp->details = module->default_blendop_params->details;
    } else if (!strcmp(path, "output_channels_shown")) {
        // GUI state only: "show output channels" from the blending options menu.
        if (!is_bool(value))
            return 5;
        edit->outputs_shown = value != 0;
        return 0;
    } else if (!strcmp(path, "clean_output_channels")) {
        // _blendif_hide_output_channels: "reset and hide output channels".
        if (!blendif_support(module))
            return 4;
        clean_outputs(bp, csp);
        edit->outputs_shown = FALSE;
    } else if (!strcmp(path, "details")) {
        // _blendop_blendif_details_callback reprocesses when the details mask is first needed.
        const float old = bp->details;
        bp->details = CLAMP((float)value, -1.0f, 1.0f);
        if (old == 0.0f && bp->details != 0.0f)
            edit->reprocess = TRUE;
    } else if (!strcmp(path, "feathering_guide")) {
        const int guide = (int)value;
        if (guide != value ||
            !(guide == DEVELOP_MASK_GUIDE_IN_BEFORE_BLUR || guide == DEVELOP_MASK_GUIDE_OUT_BEFORE_BLUR ||
              guide == DEVELOP_MASK_GUIDE_IN_AFTER_BLUR || guide == DEVELOP_MASK_GUIDE_OUT_AFTER_BLUR))
            return 5;
        bp->feathering_guide = guide;
    } else if (!strcmp(path, "feathering_radius") || !strcmp(path, "blur_radius")) {
        // _blendop_blendif_feathering_callback moves old edits to the current feathering.
        if (!strcmp(path, "feathering_radius"))
            bp->feathering_radius = CLAMP((float)value, 0.0f, 250.0f);
        else
            bp->blur_radius = CLAMP((float)value, 0.0f, 100.0f);
        if (bp->feather_version == 0)
            bp->feather_version = 1;
    } else if (!strcmp(path, "brightness"))
        bp->brightness = CLAMP((float)value, -1.0f, 1.0f);
    else if (!strcmp(path, "contrast"))
        bp->contrast = CLAMP((float)value, -1.0f, 1.0f);
    else if (!strcmp(path, "raster_mask_invert")) {
        // _raster_polarity_callback
        if (!masks_support(module) || !is_bool(value))
            return masks_support(module) ? 5 : 4;
        bp->raster_mask_invert = value != 0;
    } else if (g_str_has_prefix(path, "raster_mask"))
        error = set_raster(edit, module, path, value);
    else
        return 3;
    if (!error)
        edit->touched = TRUE;
    return error;
}

void om_blend_commit(dt_iop_module_t *module, OmBlendEdit *edit) {
    if (!edit->touched)
        return;
    dt_iop_module_t *previous = module->raster_mask.sink.source;
    dt_iop_commit_blend_params(module, &edit->params);
    // _raster_value_changed_callback: the old source no longer serves this module.
    if (previous && previous != module->raster_mask.sink.source)
        g_hash_table_remove(previous->raster_mask.source.users, module);
    if (edit->reprocess && module->dev && module->dev->full.pipe)
        module->dev->full.pipe->cache_obsolete = TRUE;
}

static JsonArray *float_array(const float *values, int count) {
    JsonArray *array = json_array_new();
    for (int i = 0; i < count; ++i)
        json_array_add_double_element(array, values[i]);
    return array;
}

typedef struct {
    JsonArray *array;
    dt_iop_module_t *module;
} OmRasterList;
static void add_raster(gpointer key, gpointer value, gpointer data) {
    JsonObject *entry = json_object_new();
    json_object_set_int_member(entry, "id", GPOINTER_TO_INT(key));
    json_object_set_string_member(entry, "label", value ? (const char *)value : "");
    json_array_add_object_element(((OmRasterList *)data)->array, entry);
}

void om_blend_describe(JsonObject *entry, dt_iop_module_t *module) {
    if (!supports_blending(module))
        return;
    const dt_develop_blend_params_t *bp = module->blend_params;
    const dt_develop_blend_colorspace_t csp = blend_csp(module, bp);
    const dt_develop_blend_colorspace_t module_cst = dt_develop_blend_default_module_blend_colorspace(module);
    JsonObject *blend = json_object_new();
    json_object_set_boolean_member(blend, "masks", masks_support(module));
    json_object_set_boolean_member(blend, "parametric", blendif_support(module));
    json_object_set_int_member(blend, "csp", csp);
    json_object_set_int_member(blend, "default_cst", module_cst);
    // The details threshold works on raw data (dt_iop_gui_update_blending).
    const dt_image_t image = module->dev->image_storage;
    json_object_set_boolean_member(blend, "raw", dt_image_is_rawprepare_supported(&image));
    json_object_set_int_member(blend, "mask_mode", bp->mask_mode);
    json_object_set_int_member(blend, "blend_cst", bp->blend_cst);
    json_object_set_int_member(blend, "blend_mode", bp->blend_mode & DEVELOP_BLEND_MODE_MASK);
    json_object_set_boolean_member(blend, "reverse", (bp->blend_mode & DEVELOP_BLEND_REVERSE) != 0);
    json_object_set_double_member(blend, "blend_parameter", bp->blend_parameter);
    json_object_set_boolean_member(blend, "fulcrum", fulcrum_enabled(csp, bp->blend_mode));
    json_object_set_double_member(blend, "opacity", bp->opacity);
    json_object_set_int_member(blend, "mask_combine",
                               bp->mask_combine & (DEVELOP_COMBINE_INV | DEVELOP_COMBINE_INCL));
    json_object_set_boolean_member(blend, "drawn_polarity",
                                   (bp->mask_combine & DEVELOP_COMBINE_MASKS_POS) != 0);
    json_object_set_int_member(blend, "blendif", bp->blendif);
    json_object_set_boolean_member(blend, "outputs_used", outputs_used(bp, csp));
    json_object_set_double_member(blend, "details", bp->details);
    json_object_set_int_member(blend, "feathering_guide", bp->feathering_guide);
    json_object_set_double_member(blend, "feathering_radius", bp->feathering_radius);
    json_object_set_double_member(blend, "blur_radius", bp->blur_radius);
    json_object_set_double_member(blend, "brightness", bp->brightness);
    json_object_set_double_member(blend, "contrast", bp->contrast);
    // Drawn mask: how many shapes its group holds (dt_iop_gui_update_masks).
    json_object_set_int_member(blend, "mask_id", bp->mask_id);
    dt_masks_form_t *group = dt_masks_get_from_id(module->dev, bp->mask_id);
    json_object_set_int_member(blend, "drawn_shapes",
                               group && (group->type & DT_MASKS_GROUP) ? (int)g_list_length(group->points)
                                                                       : 0);
    json_object_set_boolean_member(blend, "drawn_available", om_engine_masks_add_shape != NULL);
    json_object_set_string_member(blend, "raster_mask_source", bp->raster_mask_source);
    json_object_set_int_member(blend, "raster_mask_instance", bp->raster_mask_instance);
    json_object_set_int_member(blend, "raster_mask_id", bp->raster_mask_id);
    json_object_set_boolean_member(blend, "raster_mask_invert", bp->raster_mask_invert != 0);
    json_object_set_boolean_member(blend, "raster_linked", module->raster_mask.sink.source != NULL);
    // What this module offers as raster mask to later modules.
    OmRasterList rasters = {json_array_new(), module};
    g_hash_table_foreach(module->raster_mask.source.masks, add_raster, &rasters);
    json_object_set_array_member(blend, "raster_masks", rasters.array);
    // The rest is needed only while a mask is on; every blend edit describes the module again,
    // so the catalog of an image whose modules do not blend stays small.
    if (bp->mask_mode == DEVELOP_MASK_DISABLED) {
        json_object_set_object_member(entry, "blend", blend);
        return;
    }
    json_object_set_array_member(blend, "blendif_parameters",
                                 float_array(bp->blendif_parameters, 4 * DEVELOP_BLENDIF_SIZE));
    json_object_set_array_member(blend, "boost_factors",
                                 float_array(bp->blendif_boost_factors, DEVELOP_BLENDIF_SIZE));

    JsonArray *modes = json_array_new();
    visit_modes(csp, add_mode, modes);
    const int mode = bp->blend_mode & DEVELOP_BLEND_MODE_MASK;
    if (!mode_offered(csp, mode))
        for (const dt_introspection_type_enum_tuple_t *item = dt_develop_blend_mode_names; item->name; ++item)
            if (item->value == mode) {
                add_mode(mode, mode_label(item), "deprecated", modes);
                break;
            }
    json_object_set_array_member(blend, "blend_modes", modes);

    JsonArray *channels = json_array_new();
    for (const OmBlendChannel *c = channels_of(csp); blendif_support(module) && c && c->label; ++c) {
        JsonObject *channel = json_object_new();
        json_object_set_string_member(channel, "label", c->label);
        json_object_set_string_member(channel, "name", c->name);
        json_object_set_int_member(channel, "in", c->in);
        json_object_set_int_member(channel, "out", c->in + 4);
        json_object_set_boolean_member(channel, "boost", c->boost);
        json_object_set_double_member(channel, "boost_offset", c->boost_offset);
        json_object_set_string_member(channel, "scale", c->scale);
        json_object_set_double_member(channel, "increment", c->increment);
        json_array_add_object_element(channels, channel);
    }
    json_object_set_array_member(blend, "channels", channels);
    json_object_set_object_member(entry, "blend", blend);
}
