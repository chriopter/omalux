// SPDX-License-Identifier: GPL-3.0-or-later
#include "controls.h"
#include "common/darktable.h"
#include "common/film.h"
#include "common/image.h"
#include "common/opencl.h"
#include "develop/develop.h"
#include "develop/imageop.h"
#include "develop/pixelpipe.h"
#include "develop/pixelpipe_hb.h"

static dt_develop_t dev;
static dt_iop_module_t *modules[OM_CONTROL_COUNT];
static float *parameters[OM_CONTROL_COUNT];
static int loaded;

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
  if(loaded) { dt_dev_cleanup(&dev); loaded = 0; }
  gchar *directory = g_path_get_dirname(path);
  dt_film_t roll;
  dt_film_init(&roll);
  dt_filmid_t film = dt_film_new(&roll, directory);
  dt_film_cleanup(&roll);
  g_free(directory);
  dt_imgid_t image = dt_image_import(film, path, TRUE, FALSE);
  if(!dt_is_valid_imgid(image)) return 1;
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
  for(unsigned int i=0; i<OM_CONTROL_COUNT; ++i) {
    modules[i] = NULL; parameters[i] = NULL;
    for(GList *it=dev.iop; it; it=it->next) {
      dt_iop_module_t *module=it->data;
      if(!strcmp(module->op, om_controls[i].module)) {
        modules[i]=module;
        parameters[i]=module->get_p(module->params, om_controls[i].parameter);
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
int om_engine_render(const float *values, const unsigned char **pixels, int *width, int *height) {
  if(!loaded) return 1;
  for(unsigned int i=0; i<OM_CONTROL_COUNT; ++i) {
    if(!parameters[i]) return 2;
    *parameters[i] = om_parameter_value(i, CLAMP(values[i], om_controls[i].minimum, om_controls[i].maximum));
  }
  // Apply one complete state and one history update per module before rendering.
  for(unsigned int i=0; i<OM_CONTROL_COUNT; ++i) {
    gboolean seen=FALSE;
    for(unsigned int j=0; j<i; ++j) if(modules[j]==modules[i]) seen=TRUE;
    if(!seen) dt_dev_add_history_item_ext(&dev, modules[i], TRUE, FALSE);
  }
  dt_dev_process_image_job(&dev, &dev.full, dev.full.pipe, -1, DT_DEVICE_NONE);
  *pixels=dev.full.pipe->backbuf;
  *width=dev.full.pipe->backbuf_width;
  *height=dev.full.pipe->backbuf_height;
  return (*pixels && *width > 0 && *height > 0) ? 0 : 3;
}
void om_engine_cleanup(void) {
  if(loaded) dt_dev_cleanup(&dev);
  dt_cleanup();
}
