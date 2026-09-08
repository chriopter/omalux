// SPDX-License-Identifier: GPL-3.0-or-later
#include "engine_internal.h"
#include "common/darktable.h"
#include "common/film.h"
#include "common/image.h"
#include "common/opencl.h"
#include "common/styles.h"
#include "common/iop_order.h"
#include "develop/develop.h"
#include "develop/imageop.h"
#include "develop/pixelpipe.h"
#include "develop/pixelpipe_hb.h"
#include "style_details.h"
#include "develop/blend.h"
#include "white_balance.h"

extern const char darktable_package_version[];
OmEngine *om_engine_create(int argc, char **argv) {
    if (strcmp(darktable_package_version, OMALUX_DT_VERSION)) {
        fprintf(stderr, "darktable library changed; rebuild the native adapter.\n");
        return NULL;
    }
    if (dt_init(argc, argv, FALSE, TRUE, NULL))
        return NULL;
    return g_new0(OmEngine, 1);
}
const char *om_engine_gpu_warning(OmEngine *engine) {
    if (!dt_opencl_is_enabled())
        return "GPU acceleration unavailable. Check your OpenCL driver and darktable settings. Editing "
               "continues on the CPU.";
    if (engine->loaded && engine->dev.full.pipe->opencl_error)
        return "GPU processing failed. Editing continues on the CPU.";
    return "";
}
int om_engine_open(OmEngine *engine, const char *path) {
    gchar *directory = g_path_get_dirname(path);
    dt_film_t roll;
    dt_film_init(&roll);
    dt_filmid_t film = dt_film_new(&roll, directory);
    dt_film_cleanup(&roll);
    g_free(directory);
    dt_imgid_t image = dt_image_import(film, path, TRUE, FALSE);
    if (!dt_is_valid_imgid(image))
        return 1;
    if (engine->loaded) {
        om_preset_baseline_clear(engine);
        dt_dev_cleanup(&engine->dev);
        engine->loaded = 0;
    }
    dt_dev_init(&engine->dev, TRUE);
    engine->dev.gui_attached = FALSE;
    engine->loaded = 1;
    // Keep the interactive FULL pipe created by dt_dev_init. IMAGE is a
    // one-shot helper flag which disables intermediate cache reuse.
    dt_dev_load_image(&engine->dev, image);
    engine->dev.full.dev = &engine->dev;
    engine->dev.full.zoom = DT_ZOOM_FIT;
    engine->dev.full.ppd = 1.0;
    engine->dev.full.width = OM_PREVIEW_WIDTH;
    engine->dev.full.height = OM_PREVIEW_HEIGHT;
    engine->dev.full.color_assessment = FALSE;
    om_preset_baseline_capture(engine);
    return om_engine_bind_controls(engine);
}
int om_engine_bind_controls(OmEngine *engine) {
    for (unsigned int i = 0; i < OM_CONTROL_COUNT; ++i) {
        engine->modules[i] = NULL;
        engine->parameters[i] = NULL;
        engine->integer_parameters[i] = NULL;
        for (GList *it = engine->dev.iop; it; it = it->next) {
            dt_iop_module_t *module = it->data;
            if (!strcmp(module->op, om_controls[i].module) && module->multi_priority == 0) {
                engine->modules[i] = module;
                if (g_str_has_prefix(om_controls[i].parameter, "@curve")) {
                    const dt_introspection_field_t *field = module->get_f("y");
                    const int index = atoi(om_controls[i].parameter + 6);
                    if (field && field->header.size == 42 * sizeof(float) && index >= 0 && index < 42)
                        engine->parameters[i] = (float *)module->get_p(module->params, "y") + index;
                } else if (g_str_has_prefix(om_controls[i].parameter, "@int:")) {
                    const char *name = om_controls[i].parameter + 5;
                    const dt_introspection_field_t *field = module->get_f(name);
                    if (field &&
                        (field->header.type == DT_INTROSPECTION_TYPE_INT ||
                         field->header.type == DT_INTROSPECTION_TYPE_ENUM) &&
                        field->header.size == sizeof(int)) {
                        engine->integer_parameters[i] = module->get_p(module->params, name);
                        engine->integer_values[i] = *engine->integer_parameters[i];
                        engine->parameters[i] = &engine->integer_values[i];
                    }
                } else if (!strcmp(om_controls[i].parameter, "@enabled")) {
                    engine->enabled_values[i] = module->enabled;
                    engine->parameters[i] = &engine->enabled_values[i];
                } else if (!strcmp(om_controls[i].parameter, "@temperature") ||
                           !strcmp(om_controls[i].parameter, "@tint")) {
                    if (!om_wb_read(module, module->params, &engine->wb_temperature, &engine->wb_tint))
                        return 2;
                    engine->parameters[i] = !strcmp(om_controls[i].parameter, "@temperature")
                                                ? &engine->wb_temperature
                                                : &engine->wb_tint;
                } else if (!strcmp(om_controls[i].parameter, "@opacity"))
                    engine->parameters[i] = &module->blend_params->opacity;
                else {
                    const dt_introspection_field_t *field = module->get_f(om_controls[i].parameter);
                    if (field && field->header.type == DT_INTROSPECTION_TYPE_FLOAT &&
                        field->header.size == sizeof(float)) {
                        const float a = om_parameter_value(i, om_controls[i].minimum),
                                    b = om_parameter_value(i, om_controls[i].maximum);
                        if (fminf(a, b) < field->Float.Min - 1e-4f || fmaxf(a, b) > field->Float.Max + 1e-4f)
                            return 2;
                        engine->parameters[i] = module->get_p(module->params, om_controls[i].parameter);
                    }
                }
                break;
            }
        }
        if (!engine->parameters[i]) {
            fprintf(stderr, "Missing darktable control: %s/%s\n", om_controls[i].module,
                    om_controls[i].parameter);
            return 2;
        }
    }
    return 0;
}
int om_engine_apply_style(OmEngine *engine, const char *path, const char *name, float *values) {
    if (!engine->loaded)
        return 1;
    if (!dt_styles_exists(name))
        dt_styles_import_from_file(path);
    GList *items = dt_styles_get_item_list(name, FALSE, -1, TRUE);
    if (!items)
        return 2;
    // Bundled styles target this exact adapter ABI. Reject unsupported items
    // before changing the image, rather than silently applying a partial look.
    for (GList *it = items; it; it = it->next) {
        dt_style_item_t *item = it->data;
        dt_iop_module_t *module = dt_iop_get_module_by_op_priority(engine->dev.iop, item->operation, -1);
        if (!module || module->version() != item->module_version ||
            module->params_size != item->params_size) {
            g_list_free_full(items, dt_style_item_free);
            return 3;
        }
    }
    om_preset_baseline_restore(engine);
    GList *used = NULL;
    dt_ioppr_update_for_style_items(&engine->dev, items, FALSE);
    for (GList *it = items; it; it = it->next)
        dt_styles_apply_style_item(&engine->dev, it->data, &used, FALSE);
    g_list_free(used);
    g_list_free_full(items, dt_style_item_free);
    if (om_engine_bind_controls(engine))
        return 4;
    engine->dev.full.pipe->changed |= DT_DEV_PIPE_SYNCH;
    for (unsigned int i = 0; i < OM_CONTROL_COUNT; ++i)
        values[i] = (*engine->parameters[i] - om_controls[i].offset) / om_controls[i].scale;
    return 0;
}
char *om_engine_style_details(OmEngine *engine, const char *path, const char *name) {
    return describe_style(&engine->dev, path, name);
}
void om_engine_free_json(char *value) {
    g_free(value);
}
void om_engine_read_controls(OmEngine *engine, float *values) {
    for (unsigned int i = 0; i < OM_CONTROL_COUNT; ++i)
        values[i] = (*engine->parameters[i] - om_controls[i].offset) / om_controls[i].scale;
}
int om_engine_update_controls(OmEngine *engine, const float *values, const unsigned char *changed) {
    if (!engine->loaded)
        return 1;
    for (unsigned int i = 0; i < OM_CONTROL_COUNT; ++i) {
        if (!engine->parameters[i])
            return 2;
        if (changed[i]) {
            *engine->parameters[i] =
                om_parameter_value(i, CLAMP(values[i], om_controls[i].minimum, om_controls[i].maximum));
            if (!strcmp(om_controls[i].parameter, "@opacity"))
                engine->modules[i]->blend_params->mask_mode |= DEVELOP_MASK_ENABLED;
            if (engine->integer_parameters[i])
                *engine->integer_parameters[i] = (int)lroundf(*engine->parameters[i]);
        }
    }
    for (unsigned int i = 0; i < OM_CONTROL_COUNT; ++i) {
        if (changed[i] && !strcmp(om_controls[i].module, "temperature")) {
            if (!om_wb_write(engine->modules[i], engine->wb_temperature, engine->wb_tint))
                return 3;
            break;
        }
    }
    for (unsigned int i = 0; i < OM_CONTROL_COUNT; ++i) {
        if (!changed[i])
            continue;
        gboolean seen = FALSE;
        for (unsigned int j = 0; j < i; ++j)
            if (changed[j] && engine->modules[j] == engine->modules[i])
                seen = TRUE;
        if (!seen) {
            gboolean explicit_enable = FALSE;
            for (unsigned int j = 0; j < OM_CONTROL_COUNT; ++j)
                if (changed[j] && engine->modules[j] == engine->modules[i] &&
                    !strcmp(om_controls[j].parameter, "@enabled")) {
                    engine->modules[i]->enabled = engine->enabled_values[j] > 0.5;
                    explicit_enable = TRUE;
                }
            dt_dev_add_history_item_ext(&engine->dev, engine->modules[i], !explicit_enable, FALSE);
        }
    }
    return om_engine_bind_controls(engine);
}
int om_engine_render(OmEngine *engine, const unsigned char **pixels, int *width, int *height,
                     int interactive) {
    if (!engine->loaded)
        return 1;
    const int target_width = interactive ? OM_FAST_PREVIEW_WIDTH : OM_PREVIEW_WIDTH;
    const int target_height = interactive ? OM_FAST_PREVIEW_HEIGHT : OM_PREVIEW_HEIGHT;
    if (engine->dev.full.width != target_width || engine->dev.full.height != target_height) {
        engine->dev.full.width = target_width;
        engine->dev.full.height = target_height;
        engine->dev.full.pipe->changed |= DT_DEV_PIPE_ZOOMED;
    }
    // Diagnostic reference: identical pipe/ROI, but recompute every stage.
    if (g_getenv("OMALUX_FLUSH_PREVIEW_CACHE"))
        dt_dev_pixelpipe_cache_flush(engine->dev.full.pipe);
    dt_dev_process_image_job(&engine->dev, &engine->dev.full, engine->dev.full.pipe, -1, DT_DEVICE_NONE);
    *pixels = engine->dev.full.pipe->backbuf;
    *width = engine->dev.full.pipe->backbuf_width;
    *height = engine->dev.full.pipe->backbuf_height;
    return (engine->dev.full.pipe->status == DT_DEV_PIXELPIPE_VALID && *pixels && *width > 0 && *height > 0)
               ? 0
               : 3;
}
void om_engine_cleanup(OmEngine *engine) {
    om_preset_baseline_clear(engine);
    if (engine->loaded)
        dt_dev_cleanup(&engine->dev);
    dt_cleanup();
    g_free(engine);
}

int om_engine_halation(OmEngine *engine) {
    dt_iop_module_t *module = dt_iop_get_module_by_op_priority(engine->dev.iop, "diffuse", 0);
    if (!module)
        return 1;
    memcpy(module->params, module->default_params, module->params_size);
    const char *speeds[] = {"first", "second", "third", "fourth"};
    for (int i = 0; i < 4; ++i)
        *(float *)module->get_p(module->params, speeds[i]) = .5f;
    *(int *)module->get_p(module->params, "iterations") = 1;
    *(int *)module->get_p(module->params, "radius") = 32;
    *(float *)module->get_p(module->params, "threshold") = .8f;
    module->blend_params->blend_cst = DEVELOP_BLEND_CS_RGB_SCENE;
    module->blend_params->blend_mode = DEVELOP_BLEND_RGB_R;
    module->blend_params->mask_mode = DEVELOP_MASK_ENABLED;
    module->blend_params->opacity = 35;
    dt_dev_add_history_item_ext(&engine->dev, module, TRUE, FALSE);
    return om_engine_bind_controls(engine);
}

const char *om_engine_version(void) {
    return darktable_package_version;
}
