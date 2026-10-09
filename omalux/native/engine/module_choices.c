// SPDX-License-Identifier: GPL-3.0-or-later
// Lists darktable fills at runtime and file choices: colour profiles, noise profiles, lensfun
// cameras and lenses with their focal length, aperture and distance values, LUT files,
// watermark markers, raster mask files, white balance settings, and the overlay image.
//
// A list is requested with om_engine_module_choices(operation, instance, list, query) and
// answers {"items": [{"label", "detail", "section", "set"}], "current", "more"}. Choosing an
// item sends its "set" object through om_engine_set_parameters, so the choice is one
// history item and reaches split mode like any other generic edit. Choices with side effects
// in darktable's callbacks use "@" paths handled by om_module_set_choice below.
//
// Each list follows the darktable 5.6.1 code that fills the GTK combobox or menu; the
// source lines are cited at each function.
#include "engine_internal.h"
#include "common/colormatrices.c"
#include "common/colorspaces.h"
#include "common/file_location.h"
#include "common/film.h"
#include "common/image.h"
#include "common/image_cache.h"
#include "common/noiseprofiles.h"
#include "common/overlay.h"
#include "control/conf.h"
#include <dirent.h>
#include <strings.h>
#include <lensfun.h>
#include <math.h>
#include "lut3d_gmz.h"

enum { OM_MAX_ITEMS = 400 };

typedef struct {
    JsonArray *items;
    const char *query;
    int count, more, current;
} OmList;

static void *param(dt_iop_module_t *module, const char *name) {
    return module->get_p ? module->get_p(module->params, name) : NULL;
}
static gboolean has_param(dt_iop_module_t *module, const char *name) {
    return module->get_f && module->get_f(name);
}
// Case-insensitive match of every word of the query against the label and detail.
static gboolean matches(const char *query, const char *label, const char *detail) {
    if (!query || !*query)
        return TRUE;
    gchar *hay = g_utf8_strdown(label ? label : "", -1);
    gchar *hay2 = g_utf8_strdown(detail ? detail : "", -1);
    gchar *needle = g_utf8_strdown(query, -1);
    gchar **words = g_strsplit_set(needle, " \t", -1);
    gboolean ok = TRUE;
    for (gchar **w = words; ok && *w; ++w)
        if (**w && !strstr(hay, *w) && !strstr(hay2, *w))
            ok = FALSE;
    g_strfreev(words);
    g_free(needle);
    g_free(hay);
    g_free(hay2);
    return ok;
}
// Adds an item and returns its "set" object to fill, or NULL when the query filters it out
// or the list is full.
static JsonObject *add(OmList *list, const char *label, const char *detail, const char *section,
                       gboolean current) {
    if (!matches(list->query, label, detail))
        return NULL;
    if (list->count >= OM_MAX_ITEMS) {
        ++list->more;
        return NULL;
    }
    JsonObject *item = json_object_new();
    json_object_set_string_member(item, "label", label ? label : "");
    if (detail && *detail)
        json_object_set_string_member(item, "detail", detail);
    if (section && *section)
        json_object_set_string_member(item, "section", section);
    JsonObject *set = json_object_new();
    json_object_set_object_member(item, "set", set);
    json_array_add_object_element(list->items, item);
    if (current)
        list->current = list->count;
    ++list->count;
    return set;
}

// ---- colour profiles -----------------------------------------------------------------------
// Input profiles: the image's own profiles first (update_profile_list, colorin.c:1919-2014),
// then darktable's profiles with an input position (:2023-2028). The current entry follows
// gui_update (:1650-1680). Choosing one stores type and filename (_profile_changed :493-524).
typedef struct {
    dt_colorspaces_color_profile_type_t type;
    const char *filename, *name;
} OmProfile;
static int image_profiles(dt_iop_module_t *module, OmProfile *out) {
    int n = 0;
    const dt_image_t *storage = &module->dev->image_storage;
    const dt_image_t *cimg = dt_image_cache_get(storage->id, 'r');
    if (cimg && cimg->profile)
        out[n++] = (OmProfile){DT_COLORSPACE_EMBEDDED_ICC, "", NULL};
    dt_image_cache_read_release(cimg);
    if (dt_is_valid_colormatrix(storage->d65_color_matrix[0]))
        out[n++] = (OmProfile){DT_COLORSPACE_EMBEDDED_MATRIX, "", NULL};
    if (dt_is_valid_colormatrix(storage->adobe_XYZ_to_CAM[0][0]) && !(storage->flags & DT_IMAGE_4BAYER))
        out[n++] = (OmProfile){DT_COLORSPACE_STANDARD_MATRIX, "", NULL};
    for (int k = 0; k < dt_profiled_colormatrix_cnt; k++)
        if (!strcasecmp(storage->camera_makermodel, dt_profiled_colormatrices[k].makermodel)) {
            out[n++] = (OmProfile){DT_COLORSPACE_ENHANCED_MATRIX, "", NULL};
            break;
        }
    for (int k = 0; k < dt_vendor_colormatrix_cnt; k++)
        if (!strcmp(storage->camera_makermodel, dt_vendor_colormatrices[k].makermodel)) {
            out[n++] = (OmProfile){DT_COLORSPACE_VENDOR_MATRIX, "", NULL};
            break;
        }
    for (int k = 0; k < dt_alternate_colormatrix_cnt; k++)
        if (!strcmp(storage->camera_makermodel, dt_alternate_colormatrices[k].makermodel)) {
            out[n++] = (OmProfile){DT_COLORSPACE_ALTERNATE_MATRIX, "", NULL};
            break;
        }
    for (int i = 0; i < n; ++i)
        out[i].name = dt_colorspaces_get_name(out[i].type, "");
    return n;
}
static gboolean same_profile(int type, const char *filename, int p_type, const char *p_filename) {
    return type == p_type &&
           (type != DT_COLORSPACE_FILE || dt_colorspaces_is_profile_equal(filename, p_filename));
}
// kind: 'i' input (colorin type), 'w' working (colorin type_work), 'o' output (colorout type).
static const char *profile_list(dt_iop_module_t *module, char kind, OmList *list) {
    const char *type_name = kind == 'w' ? "type_work" : "type";
    const char *file_name = kind == 'w' ? "filename_work" : "filename";
    const int *type = param(module, type_name);
    const char *filename = param(module, file_name);
    if (!type || !filename)
        return NULL;
    const char *label = NULL;
    gboolean found = FALSE;
    if (kind == 'i') {
        OmProfile own[8];
        const int n = image_profiles(module, own);
        for (int i = 0; i < n; ++i) {
            const gboolean current = !found && same_profile(own[i].type, own[i].filename, *type, filename);
            if (current) {
                label = own[i].name;
                found = TRUE;
            }
            JsonObject *set = list ? add(list, own[i].name, NULL, "image", current) : NULL;
            if (set) {
                json_object_set_int_member(set, type_name, own[i].type);
                json_object_set_string_member(set, file_name, own[i].filename);
            }
        }
    }
    for (GList *l = darktable.color_profiles->profiles; l; l = g_list_next(l)) {
        const dt_colorspaces_color_profile_t *pp = l->data;
        const int position = kind == 'i' ? pp->in_pos : kind == 'w' ? pp->work_pos : pp->out_pos;
        if (position < 0)
            continue;
        const gboolean current = !found && same_profile(pp->type, pp->filename, *type, filename);
        if (current) {
            label = pp->name;
            found = TRUE;
        }
        JsonObject *set =
            list ? add(list, pp->name, pp->type == DT_COLORSPACE_FILE ? pp->filename : NULL, NULL, current)
                 : NULL;
        if (set) {
            json_object_set_int_member(set, type_name, pp->type);
            json_object_set_string_member(set, file_name, pp->filename);
        }
    }
    return label ? label : dt_colorspaces_get_name(*type, filename);
}

// ---- noise profiles ------------------------------------------------------------------------
// The first entry is the profile darktable picks for the image's ISO (found, interpolated or
// generic poissonian; dt_iop_denoiseprofile_get_auto_profile, denoiseprofile.c:2803-2887, with
// _get_iso_highlight_preservation_shift :2655-2667); a[0] = -1 marks it as automatic
// (reload_defaults :2697-2702). The camera's measured profiles follow (:2726-2732).
// profile_callback (:2987-3000) copies a and b; gui_update (:3105-3116) selects the exact match.
static dt_noiseprofile_t auto_profile(dt_iop_module_t *module, GList *profiles, char *name, size_t len,
                                      gboolean *autodetected) {
    dt_noiseprofile_t interpolated = dt_noiseprofile_generic;
    *autodetected = FALSE;
    g_strlcpy(name, _(interpolated.name), len);
    const dt_image_t *img = &module->dev->image_storage;
    const int exif_iso = img->exif_iso;
    int iso = exif_iso, shift = 0;
    const gboolean *compensate = param(module, "compensate_hilite_pres");
    if (compensate && *compensate) {
        shift = (int)floorf(img->exif_highlight_preservation);
        if (shift < 0)
            shift = 0;
        iso >>= shift;
    }
    dt_noiseprofile_t *last = NULL;
    for (GList *iter = profiles; iter; iter = g_list_next(iter)) {
        dt_noiseprofile_t *current = iter->data;
        if (current->iso == iso) {
            interpolated = *current;
            *autodetected = TRUE;
            if (iso != exif_iso)
                snprintf(name, len, _("found ISO %d (ISO %d %+d EV)"), iso, exif_iso, -shift);
            else
                snprintf(name, len, _("found ISO %d"), iso);
            break;
        }
        if (last && last->iso < iso && current->iso > iso) {
            interpolated.iso = iso;
            dt_noiseprofile_interpolate(last, current, &interpolated);
            *autodetected = TRUE;
            if (iso != exif_iso)
                snprintf(name, len, _("interpolated ISO %d (ISO %d %+d EV)"), iso, exif_iso, -shift);
            else
                snprintf(name, len, _("interpolated ISO %d"), iso);
            break;
        }
        last = current;
    }
    return interpolated;
}
static void set_ab(JsonObject *set, const float *a, const float *b) {
    for (int k = 0; k < 3; ++k) {
        char path[8];
        snprintf(path, sizeof(path), "a[%d]", k);
        json_object_set_double_member(set, path, a[k]);
        snprintf(path, sizeof(path), "b[%d]", k);
        json_object_set_double_member(set, path, b[k]);
    }
}
static const char *noise_list(dt_iop_module_t *module, OmList *list, char *label, size_t len) {
    const float *a = param(module, "a"), *b = param(module, "b");
    if (!a || !b)
        return NULL;
    GList *profiles = dt_noiseprofile_get_matching(&module->dev->image_storage);
    char name[512];
    gboolean autodetected = FALSE;
    dt_noiseprofile_t interpolated = auto_profile(module, profiles, name, sizeof(name), &autodetected);
    if (autodetected)
        interpolated.a[0] = -1.0f;
    gboolean found = FALSE;
    const char *result = NULL;
    for (GList *iter = profiles; iter; iter = g_list_next(iter)) {
        const dt_noiseprofile_t *profile = iter->data;
        if (!memcmp(profile->a, a, sizeof(float) * 3) && !memcmp(profile->b, b, sizeof(float) * 3)) {
            g_strlcpy(label, profile->name, len);
            result = label;
            found = TRUE;
            break;
        }
    }
    // gui_changed (:3064-3073): an automatic profile (a[0] = -1) shows the first entry's name.
    const gboolean automatic = !found && a[0] == -1.0f;
    if (automatic) {
        g_strlcpy(label, name, len);
        result = label;
    }
    if (list) {
        JsonObject *set = add(list, name, NULL, NULL, automatic);
        if (set)
            set_ab(set, interpolated.a, interpolated.b);
        for (GList *iter = profiles; iter; iter = g_list_next(iter)) {
            const dt_noiseprofile_t *profile = iter->data;
            const gboolean current =
                !memcmp(profile->a, a, sizeof(float) * 3) && !memcmp(profile->b, b, sizeof(float) * 3);
            if ((set = add(list, profile->name, NULL, NULL, current)))
                set_ab(set, profile->a, profile->b);
        }
    }
    g_list_free_full(profiles, dt_noiseprofile_free);
    return result;
}

// ---- lensfun -------------------------------------------------------------------------------
// darktable's lens module keeps its database in the plugin's private global data. Omalux opens
// its own copy the same way (init_global, lens.cc:3343-3395, the branch for lensfun before
// 0.3.95), once, on the worker thread.
static lfDatabase *lens_db(void) {
    static lfDatabase *db = NULL;
    static gboolean tried = FALSE;
    if (tried)
        return db;
    tried = TRUE;
    db = lf_db_new();
    if (db && lf_db_load(db) != LF_NO_ERROR) {
        char datadir[PATH_MAX] = {0};
        dt_loc_get_datadir(datadir, sizeof(datadir));
        gchar *parent = g_path_get_dirname(datadir);
        g_free(db->HomeDataDir);
        db->HomeDataDir = g_build_filename(parent, "lensfun", NULL);
        g_free(parent);
        if (lf_db_load(db) != LF_NO_ERROR) {
            lf_db_destroy(db);
            db = NULL;
        }
    }
    return db;
}
// A lensfun multi-language string in the current locale, never NULL.
static const char *mlstr(const lfMLstr value) {
    const char *s = value ? lf_mlstr_get(value) : NULL;
    return s ? s : "";
}
static gchar *camera_label(const lfCamera *cam) {
    const char *maker = mlstr(cam->Maker), *model = mlstr(cam->Model);
    gchar *base = *maker ? g_strdup_printf("%s, %s", maker, model) : g_strdup(model);
    if (!cam->Variant)
        return base;
    gchar *out = g_strdup_printf("%s (%s)", base, cam->Variant);
    g_free(base);
    return out;
}
// The camera the module refers to: gui_update looks it up by model alone (lens.cc:4657-4667).
static const lfCamera *current_camera(dt_iop_module_t *module) {
    const char *camera = param(module, "camera");
    // The database is loaded on first use only (about 80 ms), not for images without a camera.
    lfDatabase *db = camera && camera[0] ? lens_db() : NULL;
    if (!db)
        return NULL;
    const lfCamera **cams = lf_db_find_cameras_ext(db, NULL, camera, 0);
    const lfCamera *cam = cams ? cams[0] : NULL;
    lf_free(cams);
    return cam;
}
// _parse_model (lens.cc:3829-3839): leading spaces dropped.
static const lfLens *current_lens(dt_iop_module_t *module, const lfCamera *cam) {
    const char *lens = param(module, "lens");
    lfDatabase *db = lens_db();
    if (!db || !cam || !lens || !lens[0])
        return NULL;
    while (*lens && g_ascii_isspace(*lens))
        ++lens;
    const lfLens **list = lf_db_find_lenses_hd(db, cam, NULL, *lens ? lens : NULL, 0);
    const lfLens *found = list ? list[0] : NULL;
    lf_free(list);
    return found;
}
static gint compare_collate(gconstpointer a, gconstpointer b) {
    return g_utf8_collate(*(const char *const *)a, *(const char *const *)b);
}
static int compare_cameras(const void *a, const void *b) {
    const lfCamera *x = *(const lfCamera *const *)a, *y = *(const lfCamera *const *)b;
    const int m = g_utf8_collate(mlstr(x->Maker), mlstr(y->Maker));
    return m ? m : g_utf8_collate(mlstr(x->Model), mlstr(y->Model));
}
// The camera menu (camera_menu_fill, lens.cc:3772-3824): every camera, grouped by maker in
// collation order. Choosing one is _camera_menu_select (:3764-3771): model, crop factor and
// has_been_set.
static void camera_list(dt_iop_module_t *module, OmList *list) {
    lfDatabase *db = lens_db();
    const lfCamera *const *all = db ? lf_db_get_cameras(db) : NULL;
    if (!all)
        return;
    const lfCamera *cur = current_camera(module);
    int n = 0;
    while (all[n])
        ++n;
    const lfCamera **sorted = g_new(const lfCamera *, n ? n : 1);
    memcpy(sorted, all, sizeof(*sorted) * n);
    qsort(sorted, n, sizeof(*sorted), compare_cameras);
    for (int i = 0; i < n; ++i) {
        gchar *label = camera_label(sorted[i]);
        gchar *detail = g_strdup_printf("%s · crop factor %.1f", sorted[i]->Mount ? sorted[i]->Mount : "",
                                        sorted[i]->CropFactor);
        JsonObject *set = add(list, label, detail, mlstr(sorted[i]->Maker), sorted[i] == cur);
        if (set) {
            json_object_set_string_member(set, "camera", sorted[i]->Model);
            json_object_set_double_member(set, "crop", sorted[i]->CropFactor);
            json_object_set_int_member(set, "has_been_set", 1);
        }
        g_free(label);
        g_free(detail);
    }
    g_free(sorted);
}
// The lens menu (_lens_menusearch_clicked, lens.cc:4161-4180, _lens_menu_fill :4116-4158):
// the lenses lensfun finds for the camera, sorted and uniquified, grouped by maker.
static void lens_list(dt_iop_module_t *module, OmList *list) {
    lfDatabase *db = lens_db();
    const lfCamera *cam = current_camera(module);
    if (!db)
        return;
    const lfLens **lenses = lf_db_find_lenses_hd(db, cam, NULL, NULL, LF_SEARCH_SORT_AND_UNIQUIFY);
    if (!lenses)
        return;
    const lfLens *cur = current_lens(module, cam);
    int n = 0;
    while (lenses[n])
        ++n;
    // Stable maker grouping in collation order, as the menu shows them.
    GPtrArray *makers = g_ptr_array_new();
    for (int i = 0; i < n; ++i) {
        const char *m = mlstr(lenses[i]->Maker);
        gboolean seen = FALSE;
        for (guint k = 0; k < makers->len; ++k)
            if (!g_strcmp0(g_ptr_array_index(makers, k), m))
                seen = TRUE;
        if (!seen)
            g_ptr_array_add(makers, (gpointer)m);
    }
    g_ptr_array_sort(makers, compare_collate);
    for (guint k = 0; k < makers->len; ++k) {
        const char *maker = g_ptr_array_index(makers, k);
        for (int i = 0; i < n; ++i) {
            if (g_strcmp0(mlstr(lenses[i]->Maker), maker))
                continue;
            const lfLens *lens = lenses[i];
            gchar *detail = lens->MinFocal < lens->MaxFocal
                                ? g_strdup_printf("%g-%gmm", lens->MinFocal, lens->MaxFocal)
                                : g_strdup_printf("%gmm", lens->MinFocal);
            const gboolean current = cur && !g_strcmp0(mlstr(cur->Model), mlstr(lens->Model)) &&
                                     !g_strcmp0(mlstr(cur->Maker), maker);
            JsonObject *set = add(list, mlstr(lens->Model), detail, maker, current);
            if (set) {
                json_object_set_string_member(set, "@lens", lens->Model);
                json_object_set_string_member(set, "@lens_maker", lens->Maker ? lens->Maker : "");
            }
            g_free(detail);
        }
    }
    g_ptr_array_free(makers, TRUE);
    lf_free(lenses);
}
// ---- "find camera" and "find lens" (area "Pipetten & Knöpfe") ----
// _camera_autosearch_clicked (lens.cc:3857-3888) and _lens_autosearch_clicked (:4184-4207): the
// cameras and lenses lensfun finds for the names darktable detected from the image's EXIF
// (the module's default parameters, _parse_model drops leading spaces). darktable leaves the
// maker argument of FindCamerasExt uninitialised; it is passed as unknown here.
static const char *default_name(dt_iop_module_t *module, const char *name) {
    const char *txt = module->get_f && module->get_f(name) ? module->get_p(module->default_params, name) : NULL;
    while (txt && *txt && g_ascii_isspace(*txt))
        ++txt;
    return txt;
}
static void camera_autosearch(dt_iop_module_t *module, OmList *list) {
    lfDatabase *db = lens_db();
    const char *model = default_name(module, "camera");
    if (!db)
        return;
    if (!model || !*model) {
        camera_list(module, list);
        return;
    }
    const lfCamera **cams = lf_db_find_cameras_ext(db, NULL, model, 0);
    if (!cams)
        return;
    const lfCamera *cur = current_camera(module);
    for (int i = 0; cams[i]; ++i) {
        gchar *label = camera_label(cams[i]);
        gchar *detail = g_strdup_printf("%s · crop factor %.1f", cams[i]->Mount ? cams[i]->Mount : "", cams[i]->CropFactor);
        JsonObject *set = add(list, label, detail, mlstr(cams[i]->Maker), cams[i] == cur);
        if (set) {
            json_object_set_string_member(set, "camera", cams[i]->Model);
            json_object_set_double_member(set, "crop", cams[i]->CropFactor);
            json_object_set_int_member(set, "has_been_set", 1);
        }
        g_free(label);
        g_free(detail);
    }
    lf_free(cams);
}
static void lens_autosearch(dt_iop_module_t *module, OmList *list) {
    lfDatabase *db = lens_db();
    const lfCamera *cam = current_camera(module);
    const char *model = default_name(module, "lens");
    if (!db)
        return;
    const lfLens **lenses =
        lf_db_find_lenses_hd(db, cam, NULL, model && *model ? model : NULL, LF_SEARCH_SORT_AND_UNIQUIFY);
    if (!lenses)
        return;
    const lfLens *cur = current_lens(module, cam);
    for (int i = 0; lenses[i]; ++i) {
        const lfLens *lens = lenses[i];
        gchar *detail = lens->MinFocal < lens->MaxFocal ? g_strdup_printf("%g-%gmm", lens->MinFocal, lens->MaxFocal)
                                                        : g_strdup_printf("%gmm", lens->MinFocal);
        JsonObject *set = add(list, mlstr(lens->Model), detail, mlstr(lens->Maker), lens == cur);
        if (set) {
            json_object_set_string_member(set, "@lens", lens->Model);
            json_object_set_string_member(set, "@lens_maker", lens->Maker ? lens->Maker : "");
        }
        g_free(detail);
    }
    lf_free(lenses);
}
// ---- end find camera / lens ----
// _precision, lens.cc:3602-3623: the digits darktable prints for focal, aperture and distance.
static int precision(double x, double adj) {
    x *= adj;
    if (x == 0)
        return 1;
    if (x < 1.0)
        return x < 0.1 ? (x < 0.01 ? 5 : 4) : 3;
    if (x < 100.0)
        return x < 10.0 ? 2 : 1;
    return 0;
}
static void value_item(OmList *list, const char *field, double value, double stored) {
    char text[32];
    snprintf(text, sizeof(text), "%.*f", precision(value, 10.0), value);
    JsonObject *set = add(list, text, NULL, NULL, fabs(value - stored) < 1e-6);
    if (set) {
        json_object_set_double_member(set, field, value);
        json_object_set_int_member(set, "has_been_set", 1);
    }
}
// The editable focal length, f-stop and distance comboboxes (_lens_set, lens.cc:3922-4099):
// the current value first, then darktable's fixed steps inside the lens's range. Typed values
// are accepted like darktable's entry (sscanf of the text, _lens_comboentry_*_update).
static void lens_values(dt_iop_module_t *module, const char *field, OmList *list) {
    double focal_values[] = {-INFINITY, 4.5, 8,   10,  12,  14,  15,  16,   17,   18,   20,   24,   28,
                             30,        31,  35,  38,  40,  43,  45,  50,   55,   60,   70,   75,   77,
                             80,        85,  90,  100, 105, 110, 120, 135,  150,  200,  210,  240,  250,
                             300,       400, 500, 600, 700, 800, 840, 1000, 1120, 1200, 1600, 2000, INFINITY};
    double aperture_values[] = {-INFINITY, 0.7, 0.8, 0.9, 1,   1.1, 1.2, 1.4, 1.8, 2,  2.2, 2.5, 2.8,     3.2,
                                3.4,       4,   4.5, 5.0, 5.6, 6.3, 7.1, 8,   9,   10, 11,  13,  14,      16,
                                18,        20,  22,  25,  29,  32,  38,  45,  50,  54, 64,  90,  INFINITY};
    const float *stored = param(module, field);
    if (!stored)
        return;
    const lfLens *lens = current_lens(module, current_camera(module));
    if (list->query && *list->query) {
        // A typed number becomes the first entry, as darktable's editable combobox accepts it.
        float typed = 0.f;
        if (sscanf(list->query, "%f", &typed) == 1 && typed > 0.f) {
            const char *keep = list->query;
            list->query = NULL;
            value_item(list, field, typed, *stored);
            list->query = keep;
        }
    }
    value_item(list, field, *stored, *stored);
    if (!strcmp(field, "distance")) {
        float val = 0.25f;
        for (int k = 0; k < 25; k++) {
            if (val > 1000.0f)
                val = 1000.0f;
            value_item(list, field, val, *stored);
            if (val >= 1000.0f)
                break;
            val *= M_SQRT2_F;
        }
        return;
    }
    if (!lens)
        return; // darktable fills these lists only once a lens is known
    const int nf = sizeof(focal_values) / sizeof(double), na = sizeof(aperture_values) / sizeof(double);
    if (!strcmp(field, "focal")) {
        int ffi = 1, fli = -1;
        for (int i = 1; i < nf - 1; i++) {
            if (focal_values[i] < lens->MinFocal)
                ffi = i + 1;
            if (focal_values[i] > lens->MaxFocal && fli == -1)
                fli = i;
        }
        if (focal_values[ffi] > lens->MinFocal) {
            focal_values[ffi - 1] = lens->MinFocal;
            ffi--;
        }
        if (lens->MaxFocal == 0 || fli < 0)
            fli = nf - 2;
        if (focal_values[fli + 1] < lens->MaxFocal) {
            focal_values[fli + 1] = lens->MaxFocal;
            ffi++;
        }
        if (fli < ffi)
            fli = ffi + 1;
        for (int k = 0; k < fli - ffi; k++)
            value_item(list, field, focal_values[ffi + k], *stored);
    } else if (!strcmp(field, "aperture")) {
        int ffi = 1, fli = na - 1;
        for (int i = 1; i < na - 1; i++)
            if (aperture_values[i] < lens->MinAperture)
                ffi = i + 1;
        if (aperture_values[ffi] > lens->MinAperture) {
            aperture_values[ffi - 1] = lens->MinAperture;
            ffi--;
        }
        for (int k = 0; k < fli - ffi; k++)
            value_item(list, field, aperture_values[ffi + k], *stored);
    }
}
// _lenstype_to_lensfun_lenstype and _modflags_to_lensfun_mods (lens.cc:299-333).
static lfLensType lensfun_type(int lt) {
    switch (lt) {
    case 1:
        return LF_RECTILINEAR;
    case 2:
        return LF_FISHEYE;
    case 3:
        return LF_PANORAMIC;
    case 4:
        return LF_EQUIRECTANGULAR;
    case 5:
        return LF_FISHEYE_ORTHOGRAPHIC;
    case 6:
        return LF_FISHEYE_STEREOGRAPHIC;
    case 7:
        return LF_FISHEYE_EQUISOLID;
    case 8:
        return LF_FISHEYE_THOBY;
    default:
        return LF_UNKNOWN;
    }
}
static int lensfun_mods(int flags) {
    int mods = LF_MODIFY_GEOMETRY | LF_MODIFY_SCALE;
    mods |= flags & 4 ? LF_MODIFY_DISTORTION : 0;
    mods |= flags & 2 ? LF_MODIFY_VIGNETTING : 0;
    mods |= flags & 1 ? LF_MODIFY_TCA : 0;
    return mods;
}
// Choosing a lens: _lens_menu_select (lens.cc:4101-4114) runs _lens_set (model, and the focal
// length of a prime lens, :3922-4048), marks the parameters as set and stores the automatic
// scale of _get_autoscale_lf (:1000-1043, _get_modifier :953-998 for lensfun < 0.3.95).
static int set_lens(dt_iop_module_t *module, const char *model, const char *maker) {
    lfDatabase *db = lens_db();
    const lfCamera *cam = current_camera(module);
    char *lens_p = param(module, "lens");
    float *focal = param(module, "focal"), *scale = param(module, "scale");
    gboolean *has_been_set = param(module, "has_been_set");
    const dt_introspection_field_t *lens_f = module->get_f ? module->get_f("lens") : NULL;
    if (!db || !lens_p || !focal || !scale || !has_been_set || !lens_f || !model)
        return 5;
    const lfLens **lenses = lf_db_find_lenses_hd(db, cam, maker && *maker ? maker : NULL, model, 0);
    const lfLens *lens = lenses ? lenses[0] : NULL;
    if (!lens) {
        lf_free(lenses);
        return 5;
    }
    dt_strlcpy_to_fixed(lens_p, lens->Model, lens_f->header.size);
    if (lens->MinFocal >= lens->MaxFocal)
        *focal = lens->MinFocal;
    *has_been_set = TRUE;
    // _get_autoscale_lf looks the lens up again by the stored model.
    float result = 1.0f;
    const lfLens **again = lf_db_find_lenses_hd(db, cam, NULL, lens_p, 0);
    if (again) {
        const dt_image_t *img = &module->dev->image_storage;
        const int *flags = param(module, "modify_flags"), *inverse = param(module, "inverse"),
                  *target = param(module, "target_geom");
        const float *crop = param(module, "crop"), *aperture = param(module, "aperture"),
                    *distance = param(module, "distance");
        lfModifier *modifier = lf_modifier_new(again[0], crop ? *crop : 0.f, img->p_width, img->p_height);
        lf_modifier_initialize(modifier, again[0], LF_PF_F32, *focal, aperture ? *aperture : 0.f,
                               distance ? *distance : 0.f, 1.0f, lensfun_type(target ? *target : 1),
                               lensfun_mods(flags ? *flags : 7), inverse ? *inverse : 0);
        result = lf_modifier_get_auto_scale(modifier, inverse ? *inverse : 0);
        lf_modifier_destroy(modifier);
    }
    lf_free(again);
    lf_free(lenses);
    *scale = result;
    return 0;
}

// ---- files ---------------------------------------------------------------------------------
static int compare_strings(const void *a, const void *b) {
    return strcmp(*(char *const *)a, *(char *const *)b);
}
static gboolean has_extension(const char *name, const char *const *extensions) {
    const char *dot = strrchr(name, '.');
    if (!dot)
        return FALSE;
    for (const char *const *e = extensions; *e; ++e)
        if (!g_ascii_strcasecmp(dot, *e))
            return TRUE;
    return FALSE;
}
// Files under a folder (recursive up to a depth), relative paths with '/' separators.
static void collect_files(const char *root, const char *relative, int depth, const char *const *extensions,
                          GPtrArray *out) {
    gchar *folder = relative ? g_build_filename(root, relative, NULL) : g_strdup(root);
    GDir *dir = g_dir_open(folder, 0, NULL);
    if (dir) {
        const gchar *name;
        while ((name = g_dir_read_name(dir)) && out->len < 5000) {
            if (name[0] == '.')
                continue;
            gchar *rel = relative ? g_strdup_printf("%s/%s", relative, name) : g_strdup(name);
            gchar *full = g_build_filename(folder, name, NULL);
            if (g_file_test(full, G_FILE_TEST_IS_DIR)) {
                if (depth > 0)
                    collect_files(root, rel, depth - 1, extensions, out);
                g_free(rel);
            } else if (has_extension(name, extensions))
                g_ptr_array_add(out, rel);
            else
                g_free(rel);
            g_free(full);
        }
        g_dir_close(dir);
    }
    g_free(folder);
}
// LUT files below the LUT root folder (plugins/darkroom/lut3d/def_path). darktable picks one with
// a file chooser limited to that folder and lists the files of the chosen file's folder
// (_button_clicked, lut3d.c:1607-1675; _update_filepath_combobox :1570-1605; extensions
// _check_extension :1550-1568). Omalux lists the whole root, sub-folders included. G'MIC
// compressed LUTs (.gmz) are read by lut3d_gmz.cpp (area E) once the file is chosen.
static const char *const lut_extensions[] = {".png", ".cube", ".3dl", ".gmz", NULL};
static void lut_list(dt_iop_module_t *module, OmList *list) {
    gchar *root = dt_conf_get_string("plugins/darkroom/lut3d/def_path");
    const char *filepath = param(module, "filepath");
    if (root && root[0]) {
        GPtrArray *files = g_ptr_array_new_with_free_func(g_free);
        collect_files(root, NULL, 4, lut_extensions, files);
        g_ptr_array_sort(files, compare_strings);
        for (guint i = 0; i < files->len; ++i) {
            const char *rel = g_ptr_array_index(files, i);
            gchar *folder = g_path_get_dirname(rel);
            gchar *base = g_path_get_basename(rel);
            JsonObject *set = add(list, base, strcmp(folder, ".") ? folder : NULL, NULL,
                                  filepath && !strcmp(filepath, rel));
            if (set) {
                json_object_set_string_member(set, "filepath", rel);
                // _filepath_callback (lut3d.c:1460-1480): a new file drops a compressed LUT;
                // a new .gmz keeps the LUT name, the file is then read (lut3d_gmz_params.c).
                const gboolean gmz = g_str_has_suffix(rel, ".gmz") || g_str_has_suffix(rel, ".GMZ");
                if (has_param(module, "nb_keypoints") && !gmz)
                    json_object_set_int_member(set, "nb_keypoints", 0);
                if (has_param(module, "lutname") && !gmz)
                    json_object_set_string_member(set, "lutname", "");
            }
            g_free(folder);
            g_free(base);
        }
        g_ptr_array_free(files, TRUE);
    }
    g_free(root);
}
// Watermark markers from darktable's data folder and the user's config folder
// (_refresh_watermarks, watermark.c:1184-1207; _load_watermarks :1142-1182): SVG and PNG files,
// shown as "name (extension)".
static gchar *marker_label(const char *file) {
    gchar *copy = g_strdup(file);
    gchar *dot = strrchr(copy, '.');
    gchar *label = dot ? (*dot = 0, g_strdup_printf("%s (%s)", copy, dot + 1)) : g_strdup(copy);
    g_free(copy);
    return label;
}
static void marker_list(dt_iop_module_t *module, OmList *list) {
    const char *current = param(module, "filename");
    char dirs[2][PATH_MAX] = {{0}, {0}};
    dt_loc_get_datadir(dirs[0], sizeof(dirs[0]));
    dt_loc_get_user_config_dir(dirs[1], sizeof(dirs[1]));
    static const char *const extensions[] = {".svg", ".png", NULL};
    for (int d = 0; d < 2; ++d) {
        gchar *folder = g_build_filename(dirs[d], "watermarks", NULL);
        GPtrArray *files = g_ptr_array_new_with_free_func(g_free);
        collect_files(folder, NULL, 0, extensions, files);
        g_ptr_array_sort(files, compare_strings);
        for (guint i = 0; i < files->len; ++i) {
            const char *file = g_ptr_array_index(files, i);
            gchar *label = marker_label(file);
            JsonObject *set = add(list, label, NULL, d ? "user" : NULL, current && !strcmp(current, file));
            if (set)
                json_object_set_string_member(set, "filename", file);
            g_free(label);
        }
        g_ptr_array_free(files, TRUE);
        g_free(folder);
    }
}
// Raster mask files of the chosen folder (_update_filepath, rasterfile.c:322-361; PFM and PNG).
static void raster_list(dt_iop_module_t *module, OmList *list) {
    const char *path = param(module, "path"), *file = param(module, "file");
    if (!path || !path[0])
        return;
    static const char *const extensions[] = {".pfm", ".png", NULL};
    GPtrArray *files = g_ptr_array_new_with_free_func(g_free);
    collect_files(path, NULL, 0, extensions, files);
    g_ptr_array_sort(files, compare_strings);
    for (guint i = 0; i < files->len; ++i) {
        const char *name = g_ptr_array_index(files, i);
        JsonObject *set = add(list, name, NULL, NULL, file && !strcmp(file, name));
        if (set)
            json_object_set_string_member(set, "file", name);
    }
    g_ptr_array_free(files, TRUE);
}
// A path below a root folder, without the root and with '/' separators; NULL outside it.
static gchar *below(const char *root, const char *path) {
    if (!root || !root[0] || !path)
        return NULL;
    gchar *r = g_canonicalize_filename(root, NULL), *p = g_canonicalize_filename(path, NULL);
    gchar *out = NULL;
    const size_t n = strlen(r);
    if (strlen(p) > n + 1 && !strncmp(p, r, n) && p[n] == G_DIR_SEPARATOR)
        out = g_strdup(p + n + 1);
    g_free(r);
    g_free(p);
    if (out)
        for (char *c = out; *c; ++c)
            if (*c == '\\')
                *c = '/';
    return out;
}
// The LUT root may hold links to the folders with the files (Omalux's own root links the style
// and camera catalogues, omalux/style_assets.py): a file chosen in such a folder by its real
// path is below the root through that link.
static gchar *below_links(const char *root, const char *path) {
    gchar *out = below(root, path);
    GDir *dir = !out && root && root[0] ? g_dir_open(root, 0, NULL) : NULL;
    const gchar *name;
    while (dir && !out && (name = g_dir_read_name(dir))) {
        gchar *entry = g_build_filename(root, name, NULL);
        gchar *target = g_file_test(entry, G_FILE_TEST_IS_SYMLINK) ? realpath(entry, NULL) : NULL;
        gchar *inside = target ? below(target, path) : NULL;
        if (inside)
            out = g_strdup_printf("%s/%s", name, inside);
        g_free(inside);
        free(target);
        g_free(entry);
    }
    if (dir)
        g_dir_close(dir);
    return out;
}
static int write_text(dt_iop_module_t *module, const char *name, const char *value) {
    char *target = param(module, name);
    const dt_introspection_field_t *f = module->get_f ? module->get_f(name) : NULL;
    if (!target || !f || f->header.type != DT_INTROSPECTION_TYPE_ARRAY || strlen(value) >= f->header.size)
        return 5;
    memset(target, 0, f->header.size);
    g_strlcpy(target, value, f->header.size);
    return 0;
}
// A LUT file chosen in the file dialog: accepted only below the LUT root folder, stored
// relative to it (_button_clicked, lut3d.c:1655-1667, then _filepath_callback).
static int set_lut_file(dt_iop_module_t *module, const char *path) {
    gchar *root = dt_conf_get_string("plugins/darkroom/lut3d/def_path");
    gchar *rel = below_links(root, path);
    g_free(root);
    int error = rel && has_extension(rel, lut_extensions) ? write_text(module, "filepath", rel) : 5;
    // a .gmz keeps the LUT name and is read after the edit (lut3d_gmz_params.c)
    if (!error && !g_str_has_suffix(rel, ".gmz") && !g_str_has_suffix(rel, ".GMZ")) {
        int *keypoints = param(module, "nb_keypoints");
        if (keypoints)
            *keypoints = 0;
        if (has_param(module, "lutname"))
            write_text(module, "lutname", "");
    }
    g_free(rel);
    return error;
}
// A raster mask file chosen in the file dialog: accepted only below the raster mask root
// folder; the folder and the file name are stored (_fbutton_clicked, rasterfile.c:363-424).
static int set_raster_file(dt_iop_module_t *module, const char *chosen) {
    static const char *const extensions[] = {".pfm", ".png", NULL};
    g_autofree gchar *path = g_canonicalize_filename(chosen, NULL);
    gchar *root = dt_conf_get_string("plugins/darkroom/segments/def_path");
    gchar *rel = below(root, path);
    g_free(root);
    int error = 5;
    if (rel && has_extension(path, extensions)) {
        gchar *dir = g_path_get_dirname(path), *base = g_path_get_basename(path);
        error = write_text(module, "path", dir);
        if (!error)
            error = write_text(module, "file", base);
        g_free(dir);
        g_free(base);
    }
    g_free(rel);
    return error;
}
// An overlay image chosen in the file dialog. darktable takes an image dropped from its
// library (_drag_and_drop_received, overlay.c:989-1047); Omalux imports the file into the
// session library first, as opening a photo does, then records it the same way.
static int set_overlay_file(dt_iop_module_t *module, const char *chosen) {
    dt_imgid_t *imgid = param(module, "imgid");
    char *filename = param(module, "filename");
    const dt_introspection_field_t *f = module->get_f ? module->get_f("filename") : NULL;
    gchar *path = chosen ? g_canonicalize_filename(chosen, NULL) : NULL;
    if (!imgid || !filename || !f || !path || !g_file_test(path, G_FILE_TEST_IS_REGULAR)) {
        g_free(path);
        return 5;
    }
    gchar *directory = g_path_get_dirname(path);
    dt_film_t roll;
    dt_film_init(&roll);
    const dt_filmid_t film = dt_film_new(&roll, directory);
    dt_film_cleanup(&roll);
    g_free(directory);
    const dt_imgid_t overlay = dt_is_valid_filmid(film) ? dt_image_import(film, path, TRUE, FALSE) : NO_IMGID;
    g_free(path);
    const dt_imgid_t target = module->dev->image_storage.id;
    if (!dt_is_valid_imgid(overlay) || overlay == target || dt_overlay_used_by(overlay, target))
        return 5;
    if (dt_is_valid_imgid(*imgid))
        dt_overlay_remove(target, *imgid);
    *imgid = overlay;
    dt_overlay_record(target, overlay);
    memset(filename, 0, f->header.size);
    dt_image_full_path(overlay, filename, f->header.size, NULL);
    return 0;
}

// ---- dispatch ------------------------------------------------------------------------------
int om_module_set_choice(OmEngine *engine, dt_iop_module_t *module, size_t count, const char *const *paths,
                         const double *values, const char *const *texts) {
    (void)engine;
    (void)values;
    const char *lens = NULL, *lens_maker = NULL;
    for (size_t i = 0; i < count; ++i) {
        if (!strcmp(paths[i], "@lens"))
            lens = texts[i];
        else if (!strcmp(paths[i], "@lens_maker"))
            lens_maker = texts[i];
    }
    if (!strcmp(module->op, "lens") && lens)
        return count <= 2 ? set_lens(module, lens, lens_maker) : 3;
    if (count == 1 && texts[0]) {
        if (!strcmp(module->op, "lut3d") && !strcmp(paths[0], "@lut_file"))
            return set_lut_file(module, texts[0]);
        if (!strcmp(module->op, "rasterfile") && !strcmp(paths[0], "@raster_file"))
            return set_raster_file(module, texts[0]);
        if (!strcmp(module->op, "overlay") && !strcmp(paths[0], "@overlay_file"))
            return set_overlay_file(module, texts[0]);
    }
    return -1;
}

// The values the dynamic rows show, keyed by layout field: the current profile, lens,
// file, ... (the GTK comboboxes' visible text).
void om_module_describe_choices(dt_iop_module_t *module, JsonObject *entry) {
    JsonObject *labels = json_object_new();
    char buffer[512];
    const char *op = module->op;
    if (!strcmp(op, "colorin")) {
        const char *in = profile_list(module, 'i', NULL), *work = profile_list(module, 'w', NULL);
        if (in)
            json_object_set_string_member(labels, "type", in);
        if (work)
            json_object_set_string_member(labels, "type_work", work);
    } else if (!strcmp(op, "colorout")) {
        const char *out = profile_list(module, 'o', NULL);
        if (out)
            json_object_set_string_member(labels, "type", out);
    } else if (!strcmp(op, "denoiseprofile")) {
        const char *name = noise_list(module, NULL, buffer, sizeof(buffer));
        if (name)
            json_object_set_string_member(labels, "@profile", name);
    } else if (!strcmp(op, "lens")) {
        const lfCamera *cam = current_camera(module);
        const char *camera = param(module, "camera"), *lens_p = param(module, "lens");
        if (cam) {
            gchar *label = camera_label(cam);
            json_object_set_string_member(labels, "camera", label);
            g_free(label);
        } else if (camera)
            json_object_set_string_member(labels, "camera", camera);
        const lfLens *lens = current_lens(module, cam);
        if (lens) {
            const char *maker = mlstr(lens->Maker), *model = mlstr(lens->Model);
            gchar *label = *maker ? g_strdup_printf("%s, %s", maker, model) : g_strdup(model);
            json_object_set_string_member(labels, "lens", label);
            g_free(label);
        } else if (lens_p)
            json_object_set_string_member(labels, "lens", lens_p);
        const char *fields[3] = {"focal", "aperture", "distance"};
        for (int i = 0; i < 3; ++i) {
            const float *v = param(module, fields[i]);
            if (!v)
                continue;
            snprintf(buffer, sizeof(buffer), "%.*f", precision(*v, 10.0), *v);
            json_object_set_string_member(labels, fields[i], buffer);
        }
    } else if (!strcmp(op, "lut3d")) {
        const char *filepath = param(module, "filepath");
        if (filepath)
            json_object_set_string_member(labels, "filepath", filepath);
        const char *lutname = param(module, "lutname");
        if (lutname)
            json_object_set_string_member(labels, "lutname", lutname);
    } else if (!strcmp(op, "watermark")) {
        const char *filename = param(module, "filename");
        if (filename) {
            gchar *label = marker_label(filename);
            json_object_set_string_member(labels, "filename", label);
            g_free(label);
        }
    } else if (!strcmp(op, "rasterfile")) {
        const char *file = param(module, "file");
        if (file)
            json_object_set_string_member(labels, "file", file);
    } else if (!strcmp(op, "overlay")) {
        const char *filename = param(module, "filename");
        if (filename && filename[0]) {
            gchar *base = g_path_get_basename(filename);
            json_object_set_string_member(labels, "imgid", base);
            g_free(base);
        }
    } else if (!strcmp(op, "temperature")) {
        JsonArray *items = json_array_new();
        OmList list = {items, NULL, 0, 0, -1};
        om_wb_list(module, &list.current, items);
        if (list.current >= 0 && list.current < (int)json_array_get_length(items))
            json_object_set_string_member(
                labels, "preset",
                json_object_get_string_member(json_array_get_object_element(items, list.current), "label"));
        json_array_unref(items);
    }
    if (json_object_get_size(labels))
        json_object_set_object_member(entry, "labels", labels);
    else
        json_object_unref(labels);
}

char *om_engine_module_choices(OmEngine *engine, const char *operation, int instance, const char *name,
                               const char *query) {
    if (!engine->loaded || !operation || !name)
        return NULL;
    dt_iop_module_t *module = dt_iop_get_module_by_op_priority(engine->dev.iop, operation, instance);
    if (!module)
        return NULL;
    JsonArray *items = json_array_new();
    OmList list = {items, query, 0, 0, -1};
    const char *error = NULL;
    char buffer[512];
    if (!strcmp(operation, "colorin") && !strcmp(name, "type"))
        profile_list(module, 'i', &list);
    else if (!strcmp(operation, "colorin") && !strcmp(name, "type_work"))
        profile_list(module, 'w', &list);
    else if (!strcmp(operation, "colorout") && !strcmp(name, "type"))
        profile_list(module, 'o', &list);
    else if (!strcmp(operation, "denoiseprofile") && !strcmp(name, "@profile"))
        noise_list(module, &list, buffer, sizeof(buffer));
    else if (!strcmp(operation, "lens") && !strcmp(name, "camera")) {
        if (!lens_db())
            error = "the lensfun database could not be loaded";
        camera_list(module, &list);
    } else if (!strcmp(operation, "lens") && !strcmp(name, "lens")) {
        if (!lens_db())
            error = "the lensfun database could not be loaded";
        lens_list(module, &list);
    } else if (!strcmp(operation, "lens") && (!strcmp(name, "find_camera") || !strcmp(name, "find_lens"))) {
        if (!lens_db())
            error = "the lensfun database could not be loaded";
        if (!strcmp(name, "find_camera"))
            camera_autosearch(module, &list);
        else
            lens_autosearch(module, &list);
    } else if (!strcmp(operation, "lens") &&
               (!strcmp(name, "focal") || !strcmp(name, "aperture") || !strcmp(name, "distance")))
        lens_values(module, name, &list);
    else if (!strcmp(operation, "lut3d") && !strcmp(name, "filepath"))
        lut_list(module, &list);
    else if (!strcmp(operation, "lut3d") && !strcmp(name, "lutname")) {
        // area E: the LUTs of a .gmz file (darktable's lutname list, lut3d.c:1714-1745)
        char *names = om_lut3d_gmz_names(module);
        JsonNode *parsed = names ? json_from_string(names, NULL) : NULL;
        const char *current = param(module, "lutname");
        if (parsed && JSON_NODE_HOLDS_ARRAY(parsed)) {
            JsonArray *array = json_node_get_array(parsed);
            for (guint i = 0; i < json_array_get_length(array); ++i) {
                const char *lut = json_array_get_string_element(array, i);
                JsonObject *set = add(&list, lut, NULL, NULL, current && !strcmp(current, lut));
                if (set)
                    json_object_set_string_member(set, "lutname", lut);
            }
        } else
            error = "the chosen file is not a compressed LUT (.gmz)";
        if (parsed)
            json_node_unref(parsed);
        g_free(names);
    }
    else if (!strcmp(operation, "watermark") && !strcmp(name, "filename"))
        marker_list(module, &list);
    else if (!strcmp(operation, "rasterfile") && !strcmp(name, "file"))
        raster_list(module, &list);
    else if (!strcmp(operation, "temperature") && !strcmp(name, "preset")) {
        JsonArray *all = json_array_new();
        int current = -1;
        om_wb_list(module, &current, all);
        for (guint i = 0; i < json_array_get_length(all); ++i) {
            JsonObject *item = json_array_get_object_element(all, i);
            JsonObject *set =
                add(&list, json_object_get_string_member(item, "label"), NULL,
                    json_object_get_string_member_with_default(item, "section", NULL), (int)i == current);
            if (set)
                json_object_set_int_member(
                    set, "@preset",
                    json_object_get_int_member(json_object_get_object_member(item, "set"), "@preset"));
        }
        json_array_unref(all);
    } else
        error = "unknown list";
    JsonObject *result = json_object_new();
    json_object_set_array_member(result, "items", items);
    json_object_set_int_member(result, "current", list.current);
    json_object_set_int_member(result, "more", list.more);
    if (error)
        json_object_set_string_member(result, "error", error);
    JsonNode *node = json_node_new(JSON_NODE_OBJECT);
    json_node_take_object(node, result);
    char *out = json_to_string(node, FALSE);
    json_node_free(node);
    return out;
}
