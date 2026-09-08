// SPDX-License-Identifier: GPL-3.0-or-later
#include "common/darktable.h"
#include "common/film.h"
#include "common/image.h"
#include "develop/develop.h"
#include "develop/imageop.h"
#include "develop/pixelpipe.h"
#include "develop/pixelpipe_hb.h"

static dt_develop_t dev;
static dt_iop_module_t *brightness_module;
static int loaded;

extern const char darktable_package_version[];
int om_engine_init(int argc, char **argv) {
  if(strcmp(darktable_package_version, OMALUX_DT_VERSION)) {
    fprintf(stderr, "darktable library changed; rebuild the native adapter.\n");
    return 1;
  }
  return dt_init(argc, argv, FALSE, TRUE, NULL);
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
  brightness_module = NULL;
  for(GList *it=dev.iop; it; it=it->next) {
    dt_iop_module_t *module=it->data;
    if(!strcmp(module->op,"colisa")) { brightness_module=module; break; }
  }
  return brightness_module ? 0 : 2;
}
int om_engine_render(float value, const unsigned char **pixels, int *width, int *height) {
  if(!loaded || !brightness_module) return 1;
  float *parameter=brightness_module->get_p(brightness_module->params,"brightness");
  if(!parameter) return 2;
  *parameter = CLAMP(value / 100.0f, -1.0f, 1.0f);
  dt_dev_add_history_item_ext(&dev, brightness_module, TRUE, FALSE);
  dt_dev_process_image_job(&dev, &dev.full, dev.full.pipe, -1, DT_DEVICE_CPU);
  *pixels=dev.full.pipe->backbuf;
  *width=dev.full.pipe->backbuf_width;
  *height=dev.full.pipe->backbuf_height;
  return (*pixels && *width > 0 && *height > 0) ? 0 : 3;
}
void om_engine_cleanup(void) {
  if(loaded) dt_dev_cleanup(&dev);
  dt_cleanup();
}
