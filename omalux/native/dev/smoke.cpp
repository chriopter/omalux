// SPDX-License-Identifier: GPL-3.0-or-later
#include "smoke.h"
#include "app/editor.h"
#include "app/frames.h"
#include <QGuiApplication>
#include <QQmlApplicationEngine>
#include <QQuickWindow>
#include <QJsonDocument>
#include <QJsonArray>
#include <QJsonObject>
#include <QFile>
#include <QTimer>
#include <QDebug>
#include <functional>
#include <memory>
#include <cmath>
#include <QKeyEvent>
#include <QMouseEvent>
#include <QWheelEvent>
#include <QQuickItem>
#include <QKeySequence>
// Optional deterministic integration driver; inactive during normal use.
void install_smoke(QGuiApplication &app, Editor &editor, Frames *frames, QQmlApplicationEngine &engine) {
    const auto path = qEnvironmentVariable("OMALUX_SMOKE_SCRIPT");
    if (path.isEmpty())
        return;
    QFile file(path);
    if (!file.open(QIODevice::ReadOnly)) {
        app.exit(2);
        return;
    }
    const auto steps = QJsonDocument::fromJson(file.readAll()).array();
    auto index = std::make_shared<int>(0);
    auto previous = std::make_shared<QString>();
    auto waiting = std::make_shared<bool>(false);
    auto dragging = std::make_shared<bool>(false);
    auto historyMarks = std::make_shared<QVariantMap>();
    QObject::connect(&editor, &Editor::changed, &app, [&, historyMarks] {
        if (editor.preview().startsWith("image://hover/")) {
            const auto image = static_cast<Frames *>(engine.imageProvider("hover"))->image();
            (*historyMarks)[QString("hover-width-%1").arg(image.width())] = true;
        }
    });
    auto *timer = new QTimer(&app);
    timer->setInterval(150);
    QObject::connect(
        timer, &QTimer::timeout, &app,
        [&, frames, steps, index, previous, waiting, timer, historyMarks, dragging] {
            if (*dragging)
                return;
            if (!editor.styleError().isEmpty()) {
                qCritical() << editor.styleError();
                app.exit(2);
                return;
            }
            if (!editor.stylesReady() || editor.preview().isEmpty() || editor.styleBusy())
                return;
            if (*waiting && editor.preview() == *previous)
                return;
            *waiting = false;
            if (*index >= steps.size()) {
                qInfo() << "Smoke complete";
                timer->stop();
                QTimer::singleShot(qEnvironmentVariableIntValue("OMALUX_SMOKE_SETTLE"), &app,
                                   [&app] { app.quit(); });
                return;
            }
            const auto step = steps[(*index)++].toObject();
            qInfo() << "Smoke step" << *index << step;
            *previous = editor.preview();
            if (step.contains("drag")) {
                *dragging = true;
                auto *window = qobject_cast<QQuickWindow *>(engine.rootObjects().first());
                std::function<QQuickItem *(QQuickItem *)> find = [&](QQuickItem *item) -> QQuickItem * {
                    if (item->objectName() == "control-slider-" + step["drag"].toString())
                        return item;
                    for (auto *child : item->childItems())
                        if (auto *found = find(child))
                            return found;
                    return nullptr;
                };
                auto *slider = step["pointer"].toBool() ? find(window->contentItem()) : nullptr;
                if (step["pointer"].toBool() && !slider) {
                    app.exit(2);
                    return;
                }
                auto pointer = [window, slider](QEvent::Type type, double value) {
                    const double from = slider->property("from").toDouble(),
                                 to = slider->property("to").toDouble();
                    const QPointF point = slider->mapToScene(QPointF(
                        5 + (slider->width() - 10) * (value - from) / (to - from), slider->height() / 2));
                    QMouseEvent event(type, point, window->mapToGlobal(point.toPoint()),
                                      type == QEvent::MouseMove ? Qt::NoButton : Qt::LeftButton,
                                      type == QEvent::MouseButtonRelease ? Qt::NoButton : Qt::LeftButton,
                                      Qt::NoModifier);
                    QGuiApplication::sendEvent(window, &event);
                };
                if (slider)
                    pointer(QEvent::MouseButtonPress, step["from"].toDouble());
                else
                    editor.setInteractive(true);
                auto *drag = new QTimer(&app);
                drag->setInterval(16);
                auto count = std::make_shared<int>(0), updates = std::make_shared<int>(0);
                auto last = std::make_shared<QString>(editor.preview());
                auto drafts = std::make_shared<int>(0);
                QObject::connect(drag, &QTimer::timeout, &app,
                                 [&, drag, step, count, updates, last, drafts, dragging, previous, waiting,
                                  slider, pointer, frames] {
                                     if (editor.preview() != *last) {
                                         ++*updates;
                                         *last = editor.preview();
                                         if (frames->image().width() <= 700)
                                             ++*drafts;
                                     }
                                     const int samples = step["samples"].toInt(120);
                                     if (*count >= samples) {
                                         *previous = editor.preview();
                                         *waiting = true;
                                         if (slider)
                                             pointer(QEvent::MouseButtonRelease, step["to"].toDouble());
                                         else
                                             editor.setInteractive(false);
                                         qInfo() << "Drag draft frames" << *drafts;
                                         if (*drafts < 2) {
                                             qCritical() << "No reduced previews during drag";
                                             app.exit(2);
                                         }
                                         drag->stop();
                                         drag->deleteLater();
                                         *dragging = false;
                                         qInfo() << "Drag intermediate frames" << *updates;
                                         if (*updates < 2) {
                                             qCritical() << "Preview stalled during drag";
                                             app.exit(2);
                                         }
                                         return;
                                     }
                                     *previous = editor.preview();
                                     *waiting = true;
                                     const double fraction = double(++*count) / samples;
                                     const double value =
                                         step["from"].toDouble() +
                                         fraction * (step["to"].toDouble() - step["from"].toDouble());
                                     if (slider)
                                         pointer(QEvent::MouseMove, value);
                                     else
                                         editor.setControl(step["drag"].toString(), value);
                                 });
                drag->start();
            } else if (step.contains("control")) {
                const auto id = step["control"].toString();
                const auto value = step["value"].toDouble();
                if (editor.controlValues()[id].toDouble() != value) {
                    editor.setControl(id, value);
                    *waiting = true;
                }
            } else if (step.contains("controls")) {
                editor.setControls(step["controls"].toObject().toVariantMap());
                *waiting = true;
            } else if (step.contains("halation")) {
                editor.applyHalation();
                *waiting = true;
            } else if (step.contains("style")) {
                editor.applyStyle(step["style"].toString());
                *waiting = true;
            } else if (step.contains("open")) {
                editor.openPhoto(QUrl::fromLocalFile(step["open"].toString()));
                *waiting = true;
            } else if (step.contains("saveStyle")) {
                editor.saveStyle(step["saveStyle"].toString());
                *waiting = true;
            } else if (step.contains("applyNamed")) {
                for (const auto &p : editor.styles())
                    if (p.toMap()["name"] == step["applyNamed"].toVariant()) {
                        editor.applyStyle(p.toMap()["id"].toString());
                        *waiting = true;
                        break;
                    }
                if (!*waiting) {
                    qCritical() << "Saved style missing";
                    app.exit(2);
                }
            } else if (step.contains("geometry")) {
                auto *panel = engine.rootObjects().first()->findChild<QObject *>("geometryPanel");
                if (!panel) {
                    app.exit(2);
                    return;
                }
                if (step.contains("ratio"))
                    panel->setProperty("aspectRatio", step["ratio"].toDouble());
                if (!QMetaObject::invokeMethod(panel, step["geometry"].toString().toUtf8().constData())) {
                    app.exit(2);
                    return;
                }
            } else if (step.contains("exportNamed") || step.contains("deleteNamed")) {
                const auto name = step.contains("exportNamed") ? step["exportNamed"] : step["deleteNamed"];
                bool found = false;
                for (const auto &p : editor.styles())
                    if (p.toMap()["name"] == name.toVariant()) {
                        const auto id = p.toMap()["id"].toString();
                        found = true;
                        if (step.contains("exportNamed"))
                            editor.exportStyle(id, QUrl::fromLocalFile(step["destination"].toString()));
                        else {
                            editor.deleteStyle(id);
                            *waiting = true;
                        }
                        break;
                    }
                if (!found) {
                    qCritical() << "Style missing";
                    app.exit(2);
                    return;
                }
            } else if (step.contains("export")) {
                editor.exportPhoto(QUrl::fromLocalFile(step["export"].toString()), 90);
                *waiting = true;
            } else if (step.contains("checkHoverFull")) {
                const auto image = static_cast<Frames *>(engine.imageProvider("hover"))->image();
                if (!editor.preview().startsWith("image://hover/") || image.width() != 1400 ||
                    (*historyMarks)["hover-width-700"].toBool()) {
                    qCritical() << "Expected full-resolution hover without a draft frame";
                    app.exit(2);
                    return;
                }
            } else if (step.contains("checkPreviewWidth")) {
                auto *displayFrames = editor.preview().startsWith("image://hover/")
                                          ? static_cast<Frames *>(engine.imageProvider("hover"))
                                          : frames;
                if (displayFrames->image().width() != step["checkPreviewWidth"].toInt()) {
                    qCritical() << "Final preview resolution wrong";
                    app.exit(2);
                    return;
                }
            } else if (step.contains("refresh")) {
                editor.setInteractive(true);
                editor.setInteractive(false);
                *waiting = true;
            } else if (step.contains("leaveHoverItem")) {
                auto *window = qobject_cast<QQuickWindow *>(engine.rootObjects().first());
                const QPointF point(5, 5);
                QMouseEvent event(QEvent::MouseMove, point, window->mapToGlobal(point.toPoint()),
                                  Qt::NoButton, Qt::NoButton, Qt::NoModifier);
                QGuiApplication::sendEvent(window, &event);
            } else if (step.contains("hoverStyle")) {
                editor.hoverStyle(step["hoverStyle"].toString(), true);
                *waiting = true;
            } else if (step.contains("leaveStyle")) {
                editor.hoverStyle("", false);
            } else if (step.contains("cancelHover")) {
                editor.hoverStyle(step["cancelHover"].toString(), true);
                editor.hoverStyle("", false);
            } else if (step.contains("checkHover")) {
                if (editor.preview().startsWith("image://hover/") != step["checkHover"].toBool()) {
                    qCritical() << "Unexpected hover state";
                    app.exit(2);
                    return;
                }
            } else if (step.contains("rememberStack")) {
                (*historyMarks)[step["rememberStack"].toString()] = editor.history();
            } else if (step.contains("checkStack")) {
                if ((*historyMarks)[step["checkStack"].toString()].toList() != editor.history()) {
                    qCritical() << "Hover modified history";
                    app.exit(2);
                    return;
                }
            } else if (step.contains("rememberPreview")) {
                (*historyMarks)[step["rememberPreview"].toString()] = editor.preview();
            } else if (step.contains("checkPreview")) {
                if ((*historyMarks)[step["checkPreview"].toString()].toString() != editor.preview()) {
                    qCritical() << "Original preview not restored";
                    app.exit(2);
                    return;
                }
            } else if (step.contains("capture")) {
                auto *window = qobject_cast<QQuickWindow *>(engine.rootObjects().first());
                window->grabWindow().save(step["capture"].toString());
                auto *displayFrames = editor.preview().startsWith("image://hover/")
                                          ? static_cast<Frames *>(engine.imageProvider("hover"))
                                          : frames;
                displayFrames->image().save(step["capture"].toString() + ".preview.png");
            } else if (step.contains("reveal"))
                QMetaObject::invokeMethod(engine.rootObjects().first(), "revealControl",
                                          Q_ARG(QVariant, step["reveal"].toVariant()));
            else if (step.contains("clickItem") || step.contains("visibleItem") ||
                     step.contains("hoverItem")) {
                const auto name = step[step.contains("hoverItem")   ? "hoverItem"
                                       : step.contains("clickItem") ? "clickItem"
                                                                    : "visibleItem"]
                                      .toString();
                std::function<QQuickItem *(QQuickItem *)> find = [&](QQuickItem *parent) -> QQuickItem * {
                    if (parent->objectName() == name)
                        return parent;
                    for (auto *child : parent->childItems())
                        if (auto *found = find(child))
                            return found;
                    return nullptr;
                };
                auto *window = qobject_cast<QQuickWindow *>(engine.rootObjects().first());
                auto *item = find(window->contentItem());
                if (!item) {
                    qCritical() << "Missing item" << name;
                    app.exit(2);
                    return;
                }
                if (step.contains("hoverItem")) {
                    const QPointF point = item->mapToScene(QPointF(item->width() / 2, item->height() / 2));
                    QMouseEvent event(QEvent::MouseMove, point, window->mapToGlobal(point.toPoint()),
                                      Qt::NoButton, Qt::NoButton, Qt::NoModifier);
                    QGuiApplication::sendEvent(window, &event);
                    *waiting = true;
                } else if (step.contains("clickItem")) {
                    if (!QMetaObject::invokeMethod(item, "clicked")) {
                        app.exit(2);
                        return;
                    }
                } else if (item->isVisible() != step["visible"].toBool()) {
                    qCritical() << "Unexpected visibility" << name << item->isVisible();
                    app.exit(2);
                    return;
                }
            } else if (step.contains("dumpControls")) {
                QFile file(step["dumpControls"].toString());
                if (file.open(QIODevice::WriteOnly)) {
                    file.write(QJsonDocument(QJsonArray::fromVariantList(editor.controls()))
                                   .toJson(QJsonDocument::Indented));
                    file.close();
                }
            } else if (step.contains("dumpModules")) {
                QFile file(step["dumpModules"].toString());
                if (file.open(QIODevice::WriteOnly)) {
                    file.write(editor.moduleCatalog().toUtf8());
                    file.close();
                }
            } else if (step.contains("panel"))
                engine.rootObjects().first()->setProperty("selectedPanel", step["panel"].toInt());
            else if (step.contains("filterView"))
                engine.rootObjects().first()->setProperty("filterView", step["filterView"].toInt());
            else if (step.contains("rememberControls")) {
                (*historyMarks)[step["rememberControls"].toString()] = editor.controlValues();
            } else if (step.contains("checkControls")) {
                const auto expected = (*historyMarks)[step["checkControls"].toString()].toMap();
                if (expected.isEmpty()) {
                    app.exit(2);
                    return;
                }
                const auto actual = editor.controlValues();
                for (auto it = expected.begin(); it != expected.end(); ++it)
                    if (std::abs(actual[it.key()].toDouble() - it.value().toDouble()) > .01) {
                        qCritical() << "Style retained an earlier edit" << it.key() << actual[it.key()]
                                    << it.value();
                        app.exit(2);
                        return;
                    }
            } else if (step.contains("rememberHistory")) {
                for (const auto &row : editor.history())
                    if (row.toMap()["current"].toBool())
                        (*historyMarks)[step["rememberHistory"].toString()] = row.toMap()["step"];
            } else if (step.contains("selectHistory") || step.contains("clickHistory")) {
                const auto name =
                    step[step.contains("clickHistory") ? "clickHistory" : "selectHistory"].toString();
                if (!historyMarks->contains(name)) {
                    app.exit(2);
                    return;
                }
                const int position = (*historyMarks)[name].toInt();
                if (step.contains("clickHistory")) {
                    const auto target = "history-step-" + QString::number(position);
                    const auto findItem = [&](auto &&self, QQuickItem *item) -> QQuickItem * {
                        if (item->objectName() == target)
                            return item;
                        for (auto *child : item->childItems())
                            if (auto *found = self(self, child))
                                return found;
                        return nullptr;
                    };
                    auto *entry = findItem(
                        findItem, qobject_cast<QQuickWindow *>(engine.rootObjects().first())->contentItem());
                    if (!entry || !QMetaObject::invokeMethod(entry, "clicked")) {
                        qCritical() << "History row unavailable";
                        app.exit(2);
                        return;
                    }
                } else
                    editor.selectHistory(position);
                *waiting = true;
            } else if (step.contains("wheelSidebar") || step.contains("pixelWheelSidebar")) {
                auto *window = qobject_cast<QQuickWindow *>(engine.rootObjects().first());
                const QPointF point(window->width() - 100, step["y"].toDouble(250));
                QWheelEvent event(point, window->mapToGlobal(point.toPoint()),
                                  QPoint(0, step["pixelWheelSidebar"].toInt()),
                                  QPoint(0, step["wheelSidebar"].toInt()), Qt::NoButton, Qt::NoModifier,
                                  static_cast<Qt::ScrollPhase>(step["phase"].toInt()), false);
                auto *panel = engine.rootObjects().first()->findChild<QObject *>("filtersScroll");
                auto *flick = panel ? panel->property("contentItem").value<QObject *>() : nullptr;
                if (flick)
                    qInfo() << "Scroll before" << flick->property("contentY");
                QGuiApplication::sendEvent(window, &event);
                if (flick)
                    qInfo() << "Scroll immediate" << flick->property("contentY");
            } else if (step.contains("checkScrolled")) {
                auto *panel = engine.rootObjects().first()->findChild<QObject *>("filtersScroll");
                auto *flick = panel ? panel->property("contentItem").value<QObject *>() : nullptr;
                if (flick)
                    qInfo() << "Scroll settled" << flick->property("contentY");
                if (!flick || flick->property("contentY").toDouble() < step["checkScrolled"].toDouble(1) ||
                    (step.contains("maximum") &&
                     flick->property("contentY").toDouble() > step["maximum"].toDouble())) {
                    qCritical() << "Sidebar did not scroll";
                    app.exit(2);
                    return;
                }
            } else if (step.contains("key")) {
                auto *window = qobject_cast<QQuickWindow *>(engine.rootObjects().first());
                const auto key = QKeySequence(step["key"].toString())[0];
                QKeyEvent press(QEvent::KeyPress, key.key(), key.keyboardModifiers());
                QKeyEvent release(QEvent::KeyRelease, key.key(), key.keyboardModifiers());
                QGuiApplication::sendEvent(window, &press);
                QGuiApplication::sendEvent(window, &release);
            } else if (step.contains("checkPanel")) {
                if (engine.rootObjects().first()->property("selectedPanel").toInt() !=
                    step["checkPanel"].toInt()) {
                    qCritical() << "Unexpected sidebar panel";
                    app.exit(2);
                    return;
                }
            } else if (step.contains("historyContains") || step.contains("historyAbsent")) {
                const bool expected = step.contains("historyContains");
                const auto operation = step[expected ? "historyContains" : "historyAbsent"].toString();
                bool found = false;
                for (const auto &row : editor.history())
                    if (row.toMap()["operation"].toString() == operation)
                        found = true;
                if (found != expected) {
                    qCritical() << "Unexpected history contents" << step << editor.history();
                    app.exit(2);
                    return;
                }
                int current = 0;
                for (const auto &row : editor.history())
                    if (row.toMap()["current"].toBool())
                        ++current;
                if (current != 1) {
                    qCritical() << "Expected one current history row";
                    app.exit(2);
                    return;
                }
            } else if (step.contains("check")) {
                const auto expected = step["check"].toObject();
                for (auto it = expected.begin(); it != expected.end(); ++it)
                    if (std::abs(editor.controlValues()[it.key()].toDouble() - it.value().toDouble()) > .01) {
                        qCritical() << "Unexpected control" << it.key() << editor.controlValues()[it.key()];
                        app.exit(2);
                        return;
                    }
            }
        });
    timer->start();
    QTimer::singleShot(180000, &app, [&app] {
        qCritical() << "Smoke timeout";
        app.exit(2);
    });
}
