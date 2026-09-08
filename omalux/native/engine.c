// SPDX-License-Identifier: GPL-3.0-or-later
#include "controls.h"
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

static dt_develop_t dev;
static dt_iop_module_t *modules[OM_CONTROL_COUNT];
static float *parameters[OM_CONTROL_COUNT];
static int loaded;
static float wb_temperature, wb_tint;
static float enabled_values[OM_CONTROL_COUNT], integer_values[OM_CONTROL_COUNT];
static int *integer_parameters[OM_CONTROL_COUNT];

static int bind_controls(void);
#include "preset_baseline.h"

extern const char darktable_package_version[];
int om_engine_init(int argc, char **argv) {
  if(strcmp(darktable_package_version, OMALUX_DT_VERSION)) {
    fprintf(stderr, "darktable library changed; rebuild the native adapter.\n");
    return 1;
  }
  return dt_init(argc, argv, FALSE, TRUE, NULL);
}
const char *om_engine_gpu_warning(void) {
  if(!dt_opencl_is_enabled())
    return "GPU acceleration unavailable. Check your OpenCL driver and darktable settings. Editing continues on the CPU.";
  if(loaded && dev.full.pipe->opencl_error)
    return "GPU processing failed. Editing continues on the CPU.";
  return "";
}
int om_engine_open(const char *path) {
  gchar *directory = g_path_get_dirname(path);
  dt_film_t roll;
  dt_film_init(&roll);
  dt_filmid_t film = dt_film_new(&roll, directory);
  dt_film_cleanup(&roll);
  g_free(directory);
  dt_imgid_t image = dt_image_import(film, path, TRUE, FALSE);
  if(!dt_is_valid_imgid(image)) return 1;
  if(loaded) { om_preset_baseline_clear(); dt_dev_cleanup(&dev); loaded=0; }
  dt_dev_init(&dev, TRUE);
  dev.gui_attached = FALSE;
  loaded = 1;
  dev.full.pipe->type |= DT_DEV_PIXELPIPE_IMAGE;
  dt_dev_load_image(&dev, image);
  dev.full.dev = &dev;
  dev.full.zoom = DT_ZOOM_FIT;
  dev.full.ppd = 1.0;
  dev.full.width = 1400;
  dev.full.height = 1000;
  dev.full.color_assessment = FALSE;
  om_preset_baseline_capture();
  return bind_controls();
}
static int bind_controls(void) {
  for(unsigned int i=0; i<OM_CONTROL_COUNT; ++i) {
    modules[i] = NULL; parameters[i] = NULL; integer_parameters[i]=NULL;
    for(GList *it=dev.iop; it; it=it->next) {
      dt_iop_module_t *module=it->data;
      if(!strcmp(module->op, om_controls[i].module) && module->multi_priority == 0) {
        modules[i]=module;
        if(g_str_has_prefix(om_controls[i].parameter,"@curve")) {
          const dt_introspection_field_t *field=module->get_f("y");
          const int index=atoi(om_controls[i].parameter+6);
          if(field && field->header.size==42*sizeof(float) && index>=0 && index<42)
            parameters[i]=(float *)module->get_p(module->params,"y")+index;
        } else if(g_str_has_prefix(om_controls[i].parameter,"@int:")) {
          const char *name=om_controls[i].parameter+5;
          const dt_introspection_field_t *field=module->get_f(name);
          if(field && (field->header.type==DT_INTROSPECTION_TYPE_INT || field->header.type==DT_INTROSPECTION_TYPE_ENUM) && field->header.size==sizeof(int)) {
            integer_parameters[i]=module->get_p(module->params,name);
            integer_values[i]=*integer_parameters[i];parameters[i]=&integer_values[i];
          }
        } else if(!strcmp(om_controls[i].parameter,"@enabled")) {
          enabled_values[i]=module->enabled; parameters[i]=&enabled_values[i];
        } else if(!strcmp(om_controls[i].parameter, "@temperature") || !strcmp(om_controls[i].parameter, "@tint")) {
          if(!om_wb_read(module,module->params,&wb_temperature,&wb_tint)) return 2;
          parameters[i]=!strcmp(om_controls[i].parameter,"@temperature") ? &wb_temperature : &wb_tint;
        } else if(!strcmp(om_controls[i].parameter, "@opacity"))
          parameters[i]=&module->blend_params->opacity;
        else {
          const dt_introspection_field_t *field=module->get_f(om_controls[i].parameter);
          if(field && field->header.type==DT_INTROSPECTION_TYPE_FLOAT && field->header.size==sizeof(float)) {
            const float a=om_parameter_value(i,om_controls[i].minimum),b=om_parameter_value(i,om_controls[i].maximum);
            if(fminf(a,b)<field->Float.Min-1e-4f || fmaxf(a,b)>field->Float.Max+1e-4f) return 2;
            parameters[i]=module->get_p(module->params, om_controls[i].parameter);
          }
        }
        break;
      }
    }
    if(!parameters[i]) {
      fprintf(stderr, "Missing darktable control: %s/%s\n", om_controls[i].module, om_controls[i].parameter);
      return 2;
    }
  }
  return 0;
}
int om_engine_apply_style(const char *path, const char *name, float *values) {
  if(!loaded) return 1;
  if(!dt_styles_exists(name)) dt_styles_import_from_file(path);
  GList *items=dt_styles_get_item_list(name, FALSE, -1, TRUE);
  if(!items) return 2;
  // Bundled styles target this exact adapter ABI. Reject unsupported items
  // before changing the image, rather than silently applying a partial look.
  for(GList *it=items; it; it=it->next) {
    dt_style_item_t *item=it->data;
    dt_iop_module_t *module=dt_iop_get_module_by_op_priority(dev.iop, item->operation, -1);
    if(!module || module->version()!=item->module_version || module->params_size!=item->params_size) {
      g_list_free_full(items, dt_style_item_free); return 3;
    }
  }
  om_preset_baseline_restore();
  GList *used=NULL;
  dt_ioppr_update_for_style_items(&dev, items, FALSE);
  for(GList *it=items; it; it=it->next)
    dt_styles_apply_style_item(&dev, it->data, &used, FALSE);
  g_list_free(used);
  g_list_free_full(items, dt_style_item_free);
  if(bind_controls()) return 4;
  dev.full.pipe->changed |= DT_DEV_PIPE_SYNCH;
  for(unsigned int i=0; i<OM_CONTROL_COUNT; ++i)
    values[i]=(*parameters[i]-om_controls[i].offset)/om_controls[i].scale;
  return 0;
}
char *om_engine_style_details(const char *path, const char *name) {
  return describe_style(&dev,path,name);
}
void om_engine_free_json(char *value) { g_free(value); }
void om_engine_read_controls(float *values) {
  for(unsigned int i=0;i<OM_CONTROL_COUNT;++i)
    values[i]=(*parameters[i]-om_controls[i].offset)/om_controls[i].scale;
}
int om_engine_update_controls(const float *values, const unsigned char *changed) {
  if(!loaded) return 1;
  for(unsigned int i=0; i<OM_CONTROL_COUNT; ++i) {
    if(!parameters[i]) return 2;
    if(changed[i]) {
      *parameters[i]=om_parameter_value(i,CLAMP(values[i],om_controls[i].minimum,om_controls[i].maximum));
      if(!strcmp(om_controls[i].parameter,"@opacity")) modules[i]->blend_params->mask_mode |= DEVELOP_MASK_ENABLED;
      if(integer_parameters[i]) *integer_parameters[i]=(int)lroundf(*parameters[i]);
    }
  }
  for(unsigned int i=0; i<OM_CONTROL_COUNT; ++i) {
    if(changed[i] && !strcmp(om_controls[i].module,"temperature")) {
      if(!om_wb_write(modules[i],wb_temperature,wb_tint)) return 3;
      break;
    }
  }
  for(unsigned int i=0; i<OM_CONTROL_COUNT; ++i) {
    if(!changed[i]) continue;
    gboolean seen=FALSE;
    for(unsigned int j=0;j<i;++j) if(changed[j] && modules[j]==modules[i]) seen=TRUE;
    if(!seen) {
      gboolean explicit_enable=FALSE;
      for(unsigned int j=0;j<OM_CONTROL_COUNT;++j) if(changed[j] && modules[j]==modules[i] && !strcmp(om_controls[j].parameter,"@enabled")) {
        modules[i]->enabled=enabled_values[j]>0.5; explicit_enable=TRUE;
      }
      dt_dev_add_history_item_ext(&dev,modules[i],!explicit_enable,FALSE);
    }
  }
  return bind_controls();
}
int om_engine_render(const unsigned char **pixels, int *width, int *height) {
  if(!loaded) return 1;
  dt_dev_process_image_job(&dev, &dev.full, dev.full.pipe, -1, DT_DEVICE_NONE);
  *pixels=dev.full.pipe->backbuf;
  *width=dev.full.pipe->backbuf_width;
  *height=dev.full.pipe->backbuf_height;
  return (dev.full.pipe->status==DT_DEV_PIXELPIPE_VALID && *pixels && *width > 0 && *height > 0) ? 0 : 3;
}
void om_engine_cleanup(void) {
  om_preset_baseline_clear();
  if(loaded) dt_dev_cleanup(&dev);
  dt_cleanup();
}

int om_engine_halation(void) {
  dt_iop_module_t *module=dt_iop_get_module_by_op_priority(dev.iop,"diffuse",0);
  if(!module) return 1;
  memcpy(module->params,module->default_params,module->params_size);
  const char *speeds[]={"first","second","third","fourth"};
  for(int i=0;i<4;++i) *(float *)module->get_p(module->params,speeds[i])=.5f;
  *(int *)module->get_p(module->params,"iterations")=1;
  *(int *)module->get_p(module->params,"radius")=32;
  *(float *)module->get_p(module->params,"threshold")=.8f;
  module->blend_params->blend_cst=DEVELOP_BLEND_CS_RGB_SCENE;
  module->blend_params->blend_mode=DEVELOP_BLEND_RGB_R;
  module->blend_params->mask_mode=DEVELOP_MASK_ENABLED;
  module->blend_params->opacity=35;
  dt_dev_add_history_item_ext(&dev,module,TRUE,FALSE);
  return bind_controls();
}
#include "session_actions.h"

#include "history.h"
