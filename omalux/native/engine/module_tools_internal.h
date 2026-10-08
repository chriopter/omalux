// SPDX-License-Identifier: GPL-3.0-or-later
// Shared between module_tools.c (capture, picking, dispatch) and the per-module ports in
// tools_*.c. Never included by Qt code.
#pragma once
#include "engine_internal.h"
#include "module_tools.h"
#include "common/color_picker.h"
#include "common/iop_profile.h"
#include "common/mipmap_cache.h"
#include <json-glib/json-glib.h>

typedef enum {
    OM_TOOL_AREA,      // colour picker over an area (DT_COLOR_PICKER_AREA)
    OM_TOOL_POINT,     // colour picker at a point (DT_COLOR_PICKER_POINT)
    OM_TOOL_BUTTON,    // a button; capture says whether it reads the module input
    OM_TOOL_HISTOGRAM  // the module input histogram only, nothing is written
} OmToolKind;

enum {
    OM_TOOL_DENOISE = 1 << 0,  // DT_COLOR_PICKER_DENOISE
    OM_TOOL_OUTPUT = 1 << 1,   // DT_COLOR_PICKER_IO: pick the module output as well
    OM_TOOL_CAPTURE = 1 << 2,  // a button that reads the module input (whole image)
    OM_TOOL_KEEP_OFF = 1 << 3  // do not switch the module on (darktable leaves it as it is)
};

// The module input as darktable's picker sees it: after the pipe converted it to the module's
// input colour space (or to the blend colour space while a mask is active).
typedef struct {
    dt_dev_pixelpipe_t pipe;
    dt_mipmap_buffer_t buf;
    gboolean pipe_ready, buf_ready;
    dt_dev_pixelpipe_iop_t *piece;
    float *input;                 // owned copy, channels × width × height floats
    dt_iop_buffer_dsc_t dsc;      // dsc.cst: colour space of input
    dt_iop_roi_t roi;             // where input lies in module-input coordinates (scaled)
    const dt_iop_order_iccprofile_info_t *work_profile;
    float *output;                // module output for OM_TOOL_OUTPUT, same roi as input
    dt_iop_buffer_dsc_t out_dsc;
    int box[4];                   // picked box in input pixels (x0, y0, x1, y1), x1/y1 exclusive
    gboolean box_valid;
} OmCapture;

typedef struct {
    lib_colorpicker_stats in, out;  // DT_PICK_MEAN/MIN/MAX
    gboolean valid, output_valid;
} OmPicked;

typedef struct OmToolContext OmToolContext;
typedef int (*OmToolRun)(OmToolContext *ctx);

typedef struct {
    const char *operation;
    const char *tool;
    OmToolKind kind;
    int cst;           // picker colour space, -1: the module's default_colorspace
    unsigned flags;
    OmToolRun run;
} OmToolSpec;

struct OmToolContext {
    OmEngine *engine;
    dt_develop_t *dev;
    dt_iop_module_t *module;
    const OmToolSpec *spec;
    float box[4];
    JsonObject *gui;          // request "gui", may be NULL
    OmCapture *capture;       // set for pickers and capturing buttons
    OmPicked picked;
    int picker_cst;           // effective picker colour space
    JsonObject *gui_out;      // values reported back for GUI-only rows
    JsonObject *extra;        // further result members (histogram, message ...)
    GPtrArray *touched;       // OmTouched*: modules whose params a tool may write
};

// Tables of the per-module ports (tools_*.c), each terminated by an entry with operation NULL.
extern const OmToolSpec om_tools_tone[];
extern const OmToolSpec om_tools_color[];
extern const OmToolSpec om_tools_geometry[];
extern const OmToolSpec om_tools_basicadj[];
extern const OmToolSpec om_tools_curves[];
extern const OmToolSpec om_tools_harmonizer[];
extern const OmToolSpec om_tools_blend[];
extern const OmToolSpec om_tools_effects[];   // area E: the remaining pickers (tools_effects.c)
extern const OmToolSpec om_tools_toneequal[]; // area E: tone equalizer auto-adjust (tools_toneequal.c)
extern const OmToolSpec om_tools_colormapping[]; // area E: color mapping acquire (tools_colormapping.c)
extern const OmToolSpec om_tools_vectorscope[];  // area E: RYB vectorscope and harmony guide (tools_vectorscope.c)
extern const OmToolSpec om_tools_blend_display[]; // area E: display mask / switch off mask (blend_display.c)
extern const OmToolSpec om_tools_masks[];         // area E: mask manager, rasterfile vectorize (mask_manager.c)
extern const OmToolSpec om_tools_ashift[];        // area E: rotate and perspective fit (ashift_fit.c)
extern const OmToolSpec om_tools_checker[];       // area E: color checker calibration (checker.c)

// Number from the request's "gui" member, or fallback.
double om_tool_gui(const OmToolContext *ctx, const char *name, double fallback);
// Report a GUI-only value back (e.g. a measured lightness shown in a target slider).
void om_tool_set_gui(OmToolContext *ctx, const char *name, double value);
// Another module the tool writes as well (flip also moves the crop): its parameters are
// compared afterwards and recorded in history with its own enabled state.
void om_tool_also(OmToolContext *ctx, dt_iop_module_t *module);
// Pick again with another colour space or box (a few tools sample twice).
gboolean om_tool_pick(OmToolContext *ctx, int cst, gboolean denoise, OmPicked *picked);
// Histogram of the captured module input in darktable's layout (bins × 4 counts).
uint32_t *om_tool_histogram(OmToolContext *ctx, int bins, uint32_t max[4]);
// The module of another operation (instance 0), for tools that read or write a neighbour.
dt_iop_module_t *om_tool_module(OmToolContext *ctx, const char *operation);
