// SPDX-License-Identifier: GPL-3.0-or-later
#include "development_tools.h"
#include "smoke.h"
#include "app/editor.h"
#include "app/frames.h"
#include <QGuiApplication>
#include <QQmlApplicationEngine>
#include <QQuickWindow>
#include <QTimer>
#include <QJsonDocument>
#include <QJsonArray>
#include <QFileInfo>
#include <QDir>
#include <QDebug>
#include <memory>
void installDevelopmentTools(QGuiApplication &app, Editor &editor, Frames *frames,
                             QQmlApplicationEngine &engine) {
    install_smoke(app, editor, frames, engine);
    // Batch thumbnails advance only after a completed engine render, without per-style startup.
    if (qEnvironmentVariableIsSet("OMALUX_PREVIEW_DIR")) {
        auto ids = std::make_shared<QStringList>();
        for (const auto &id : QJsonDocument::fromJson(qgetenv("OMALUX_PREVIEW_IDS")).array())
            ids->append(id.toString());
        auto index = std::make_shared<int>(0);
        auto waiting = std::make_shared<bool>(false);
        auto advance = [&, frames, ids, index, waiting] {
            if (!editor.presetsReady() || editor.preview().isEmpty() || editor.styleBusy())
                return;
            if (!editor.presetError().isEmpty() || !editor.status().startsWith("Ready")) {
                qCritical() << "Batch render failed:" << editor.presetError() << editor.status();
                app.exit(2);
                return;
            }
            if (*waiting) {
                const QString path =
                    QDir(qEnvironmentVariable("OMALUX_PREVIEW_DIR")).filePath(ids->at(*index) + ".png");
                if (!QDir().mkpath(QFileInfo(path).absolutePath()) || !frames->image().save(path)) {
                    qCritical() << "Could not save preview:" << path;
                    app.exit(2);
                    return;
                }
                qInfo().noquote() << "Rendered" << ids->at(*index);
                ++*index;
                *waiting = false;
            }
            if (*index == ids->size()) {
                app.quit();
                return;
            }
            *waiting = true;
            editor.applyPreset(ids->at(*index));
        };
        QObject::connect(&editor, &Editor::changed, &app,
                         [&app, advance] { QTimer::singleShot(0, &app, advance); });
        QTimer::singleShot(0, &app, advance);
        QTimer::singleShot(300000, &app, [&, frames] {
            qCritical() << "Batch preview timed out";
            app.exit(2);
        });
    }
    // Optional offscreen development capture, no effect during normal launches.
    if (qEnvironmentVariableIsSet("OMALUX_CAPTURE")) {
        QTimer::singleShot(
            qEnvironmentVariableIntValue("OMALUX_CAPTURE_DELAY") > 0
                ? qEnvironmentVariableIntValue("OMALUX_CAPTURE_DELAY") / 2
                : 2000,
            &app, [&, frames] {
                frames->image().save(qEnvironmentVariable("OMALUX_CAPTURE") + ".neutral.png");
                if (qEnvironmentVariableIsSet("OMALUX_CAPTURE_STYLE"))
                    engine.rootObjects().first()->setProperty("selectedPanel", 1);
                if (qEnvironmentVariableIsSet("OMALUX_CAPTURE_EXPAND"))
                    QMetaObject::invokeMethod(
                        engine.rootObjects().first(), "showPresetDetails",
                        Q_ARG(QVariant, QVariant(qEnvironmentVariable("OMALUX_CAPTURE_EXPAND"))));
                if (qEnvironmentVariableIsSet("OMALUX_CAPTURE_STYLE"))
                    editor.applyPreset(qEnvironmentVariable("OMALUX_CAPTURE_STYLE") == "1"
                                           ? "chromatic/preset.dtstyle"
                                           : qEnvironmentVariable("OMALUX_CAPTURE_STYLE"));
                else
                    editor.setControl(qEnvironmentVariable("OMALUX_CAPTURE_CONTROL", "brightness"),
                                      qEnvironmentVariableIsSet("OMALUX_CAPTURE_VALUE")
                                          ? qEnvironmentVariable("OMALUX_CAPTURE_VALUE").toDouble()
                                          : 0.30);
            });
        QTimer::singleShot(
            qEnvironmentVariableIntValue("OMALUX_CAPTURE_DELAY") > 0
                ? qEnvironmentVariableIntValue("OMALUX_CAPTURE_DELAY")
                : 4500,
            &app, [&, frames] {
                if (editor.preview().isEmpty() || editor.styleBusy() || !editor.presetError().isEmpty() ||
                    (qEnvironmentVariableIsSet("OMALUX_CAPTURE_STYLE") && editor.activeStyle().isEmpty())) {
                    qCritical() << "Capture did not finish applying the requested settings";
                    app.exit(2);
                    return;
                }
                auto window = qobject_cast<QQuickWindow *>(engine.rootObjects().first());
                if (window)
                    window->grabWindow().save(qEnvironmentVariable("OMALUX_CAPTURE"));
                frames->image().save(qEnvironmentVariable("OMALUX_CAPTURE") + ".preview.png");
                app.quit();
            });
    }
}
