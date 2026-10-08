// SPDX-License-Identifier: GPL-3.0-or-later
// Smoke step for the platform's native file dialog. With QT_QPA_PLATFORMTHEME=gtk3 (Omarchy's
// setting) QtQuick.Dialogs' FileDialog is Qt's QGtk3FileDialogHelper: a GtkFileChooserDialog
// in this process, on GTK's own display (headless: the broadway server of GDK_BACKEND). Keys
// are put into GDK's event queue for the dialog's window
// as a key event of the dialog's window with the keyboard device and the keymap's hardware
// code (gdk_event_put; GDK's own gdk_test_simulate_key does nothing on broadway) and
// dispatched by GTK like typed keys: the dialog's own key bindings run.
//   {"nativeDialog": "open"}       wait until a native GTK dialog is shown
//   {"nativeDialog": "closed"}     wait until none is shown
//   {"nativeDialog": "key", "key": "Escape"}   type a key into the shown dialog
#include "smoke_dialog.h"
#include <QDebug>
#include <QJsonObject>

// GTK and GDK are linked into the application (darktable and Qt's gtk3 theme use them); the
// few calls are declared here so this file needs no GTK headers next to Qt's.
extern "C" {
struct OmGList {
    void *data;
    OmGList *next, *prev;
};
OmGList *gtk_window_list_toplevels(void);
void g_list_free(OmGList *list);
int gtk_widget_get_visible(void *widget);
void *gtk_widget_get_window(void *widget);
const char *g_type_name_from_instance(void *instance);
const char *gtk_window_get_title(void *window);
unsigned gdk_keyval_from_name(const char *name);
// GdkEventKey of GTK 3's public ABI (gdk/gdkevents.h)
struct OmGdkEventKey {
    int type;
    void *window;
    signed char send_event;
    unsigned time;
    unsigned state;
    unsigned keyval;
    int length;
    char *string;
    unsigned short hardware_keycode;
    unsigned char group;
    unsigned is_modifier : 1;
};
struct OmGdkKeymapKey {
    unsigned keycode;
    int group, level;
};
void *gdk_event_new(int type);
void gdk_event_put(const void *event);
void gdk_event_free(void *event);
void gdk_event_set_device(void *event, void *device);
void *gdk_window_get_display(void *window);
void *gdk_display_get_default_seat(void *display);
void *gdk_seat_get_keyboard(void *seat);
void *gdk_keymap_get_for_display(void *display);
int gdk_keymap_get_entries_for_keyval(void *keymap, unsigned keyval, OmGdkKeymapKey **keys, int *n_keys);
void *g_object_ref(void *object);
void g_free(void *memory);
char *g_strdup(const char *text);
}

namespace {
constexpr int GDK_KEY_PRESS = 8, GDK_KEY_RELEASE = 9;

// The shown GtkDialog (file chooser or any other), or null.
void *shownDialog() {
    void *found = nullptr;
    OmGList *list = gtk_window_list_toplevels();
    for (auto *l = list; l; l = l->next)
        if (gtk_widget_get_visible(l->data) && QByteArray(g_type_name_from_instance(l->data)).contains("Dialog"))
            found = l->data;
    g_list_free(list);
    return found;
}
} // namespace

SmokeResult dialogSmokeStep(const QJsonObject &step) {
    if (!step.contains("nativeDialog"))
        return SmokeResult::NotHandled;
    const auto what = step["nativeDialog"].toString();
    void *dialog = shownDialog();
    if (what == "open" || what == "closed") {
        if ((dialog != nullptr) != (what == "open")) {
            qInfo() << "Native dialog pending" << what;
            return SmokeResult::Retry;
        }
        if (dialog)
            qInfo() << "Native dialog shown:" << g_type_name_from_instance(dialog) << gtk_window_get_title(dialog);
        else
            qInfo() << "No native dialog shown";
        return SmokeResult::Done;
    }
    if (what == "key") {
        if (!dialog || !gtk_widget_get_window(dialog)) {
            qCritical() << "No native dialog for a key";
            return SmokeResult::Fail;
        }
        const unsigned keyval = gdk_keyval_from_name(step["key"].toString().toUtf8().constData());
        void *window = gtk_widget_get_window(dialog);
        void *display = gdk_window_get_display(window);
        void *keyboard = gdk_seat_get_keyboard(gdk_display_get_default_seat(display));
        OmGdkKeymapKey *keys = nullptr;
        int count = 0;
        gdk_keymap_get_entries_for_keyval(gdk_keymap_get_for_display(display), keyval, &keys, &count);
        static unsigned now = 1000;
        for (const int type : {GDK_KEY_PRESS, GDK_KEY_RELEASE}) {
            auto *event = static_cast<OmGdkEventKey *>(gdk_event_new(type));
            event->window = g_object_ref(window);
            event->send_event = 0;
            event->time = now += 10;
            event->keyval = keyval;
            event->string = g_strdup("");
            event->hardware_keycode = count ? keys[0].keycode : 0;
            event->group = count ? keys[0].group : 0;
            if (keyboard)
                gdk_event_set_device(event, keyboard);
            gdk_event_put(event);
            gdk_event_free(event);
        }
        g_free(keys);
        qInfo() << "Native dialog key" << step["key"].toString() << keyval << (keyboard != nullptr) << count;
        return SmokeResult::Done;
    }
    qCritical() << "Unknown nativeDialog step" << what;
    return SmokeResult::Fail;
}
