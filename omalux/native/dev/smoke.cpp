// SPDX-License-Identifier: GPL-3.0-or-later
#include "smoke.h"
#include "smoke_canvas.h"
#include "smoke_pointer.h"
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
// Reads a value from the module catalog by operation, instance and parameter path: the row
// whose path is a prefix, then the remaining indices and members inside its "value".
static bool catalogValue(const QString &catalog, const QString &operation, int instance, const QString &path,
                         QJsonValue *value, bool *enabled) {
    for (const auto &entry : QJsonDocument::fromJson(catalog.toUtf8()).array()) {
        const auto module = entry.toObject();
        if (module["operation"].toString() != operation || module["instance"].toInt() != instance)
            continue;
        *enabled = module["enabled"].toBool();
        if (path == "@enabled") {
            *value = *enabled ? 1 : 0;
            return true;
        }
        // Displayed conversions ("@" paths) and the texts of runtime-list rows.
        if (path.startsWith('@') && module["derived"].toObject().contains(path)) {
            *value = module["derived"].toObject()[path];
            return true;
        }
        if (path.startsWith("labels:")) {
            *value = module["labels"].toObject()[path.mid(7)];
            return !value->isUndefined();
        }
        // The blend section ("blend.opacity", "blend.blendif_parameters[4]") and the instance
        // fields ("@multi_name") are members of the module entry, not parameter rows.
        QJsonArray rows = module["parameters"].toArray();
        rows.append(QJsonObject{{"path", "blend"}, {"value", module["blend"]}});
        if (path.startsWith("@") && module.contains(path.mid(1))) {
            const QJsonValue member = module[path.mid(1)];
            *value = member.isBool() ? QJsonValue(member.toBool() ? 1 : 0) : member;
            return true;
        }
        for (const auto &row : rows) {
            const auto rowPath = row.toObject()["path"].toString();
            if (path != rowPath && !path.startsWith(rowPath + "[") && !path.startsWith(rowPath + "."))
                continue;
            QJsonValue current = row.toObject()["value"];
            QString rest = path.mid(rowPath.size());
            while (!rest.isEmpty()) {
                if (rest.startsWith('[')) {
                    const int end = rest.indexOf(']');
                    current = current.toArray().at(rest.mid(1, end - 1).toInt());
                    rest = rest.mid(end + 1);
                } else if (rest.startsWith('.')) {
                    int end = 1;
                    while (end < rest.size() && rest[end] != '.' && rest[end] != '[')
                        ++end;
                    current = current.toObject()[rest.mid(1, end - 1)];
                    rest = rest.mid(end);
                } else
                    return false;
            }
            *value = current.isBool() ? QJsonValue(current.toBool() ? 1 : 0) : current;
            return true;
        }
    }
    return false;
}
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
    auto retries = std::make_shared<int>(0);
    auto updatedModules = std::make_shared<QStringList>();
    // Answers of requestChoices, keyed "operation/instance/list".
    auto choiceResults = std::make_shared<QVariantMap>();
    QObject::connect(&editor, &Editor::choicesReady, &app,
                     [choiceResults](QString operation, int instance, QString list, QString, QString result) {
                         (*choiceResults)[QString("%1/%2/%3").arg(operation).arg(instance).arg(list)] =
                             result;
                     });
    // "rejectStyle" expects a style error containing this text; once seen it is tolerated.
    auto expectedError = std::make_shared<QString>(), toleratedError = std::make_shared<QString>();
    // Module tool results by "operation/tool" (pickers and buttons, engine/module_tools.h).
    auto toolResults = std::make_shared<QVariantMap>();
    QObject::connect(&editor, &Editor::moduleToolResult, &app,
                     [toolResults](QString operation, int, QString tool, QString result, int error) {
                         auto map = QJsonDocument::fromJson(result.toUtf8()).object().toVariantMap();
                         map["error"] = error;
                         (*toolResults)[operation + "/" + tool] = map;
                     });
    QObject::connect(&editor, &Editor::moduleUpdated, &app,
                     [updatedModules](QString operation, int instance, QString json) {
                         const auto module = QJsonDocument::fromJson(json.toUtf8()).object();
                         if (module["operation"].toString() == operation &&
                             module["instance"].toInt() == instance)
                             updatedModules->append(operation);
                     });
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
        [&, frames, steps, index, previous, waiting, timer, historyMarks, dragging, retries, updatedModules,
         choiceResults, expectedError, toleratedError, toolResults] {
            if (*dragging)
                return;
            if (!expectedError->isEmpty() && editor.styleError().contains(*expectedError)) {
                qInfo() << "Rejected as expected:" << editor.styleError();
                *toleratedError = editor.styleError();
                expectedError->clear();
                *waiting = false;
            }
            if (!editor.styleError().isEmpty() && editor.styleError() != *toleratedError) {
                qCritical() << editor.styleError();
                app.exit(2);
                return;
            }
            // Without a readable photograph (Editor.photoMissing) the steps run on the empty editor.
            if (editor.styleBusy() ||
                (!editor.photoMissing() && (!editor.stylesReady() || editor.preview().isEmpty())))
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
            // Genuine mouse input on items (smoke_pointer.cpp).
            if (const auto pointer = pointerSmokeStep(step, engine, dragging); pointer != SmokeResult::NotHandled) {
                if (pointer == SmokeResult::Fail)
                    app.exit(2);
                return;
            }
            // Drawing on the image (smoke_canvas.cpp).
            if (const auto canvas = canvasSmokeStep(step, editor, engine); canvas != SmokeResult::NotHandled) {
                if (canvas == SmokeResult::Fail) {
                    app.exit(2);
                    return;
                }
                if (canvas == SmokeResult::Retry) {
                    if (++*retries < 100) {
                        --*index;
                        return;
                    }
                    qCritical() << "Canvas step failed" << step;
                    app.exit(2);
                    return;
                }
                *retries = 0;
                if (canvas == SmokeResult::Wait)
                    *waiting = true;
                return;
            }
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
                // "fails": text of the expected error; the file cannot be opened and the
                // photograph shown stays, so no new preview is waited for.
                if (step.contains("fails"))
                    *expectedError = step["fails"].toString();
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
            } else if (step.contains("emptyState")) {
                // The editor is (not) empty: no photograph could be read.
                if (editor.photoMissing() != step["emptyState"].toBool()) {
                    qCritical() << "Unexpected empty state" << editor.photoMissing() << editor.status();
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
            } else if (step.contains("dragItem")) {
                // Press, move and release the pointer over an item, in fractions of its size
                // (e.g. a box on the picker overlay): {"dragItem", "from": [x, y], "to": [x, y]}.
                const auto name = step["dragItem"].toString();
                std::function<QQuickItem *(QQuickItem *)> find = [&](QQuickItem *parent) -> QQuickItem * {
                    if (parent->objectName() == name && parent->isVisible())
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
                const Qt::KeyboardModifiers modifiers = step["modifiers"].toString() == "ctrl"    ? Qt::ControlModifier
                                                        : step["modifiers"].toString() == "shift" ? Qt::ShiftModifier
                                                                                                  : Qt::NoModifier;
                const auto from = step["from"].toArray(), to = step["to"].toArray();
                auto send = [&](QEvent::Type type, double fx, double fy, Qt::MouseButtons buttons) {
                    const QPointF point = item->mapToScene(QPointF(fx * item->width(), fy * item->height()));
                    QMouseEvent event(type, point, window->mapToGlobal(point.toPoint()), Qt::LeftButton, buttons,
                                      modifiers);
                    QGuiApplication::sendEvent(window, &event);
                };
                toolResults->clear(); // the next toolResult waits for what the drag applied
                send(QEvent::MouseButtonPress, from[0].toDouble(), from[1].toDouble(), Qt::LeftButton);
                for (int i = 1; i <= 8; ++i)
                    send(QEvent::MouseMove, from[0].toDouble() + i * (to[0].toDouble() - from[0].toDouble()) / 8,
                         from[1].toDouble() + i * (to[1].toDouble() - from[1].toDouble()) / 8, Qt::LeftButton);
                send(QEvent::MouseButtonRelease, to[0].toDouble(), to[1].toDouble(), Qt::NoButton);
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
            else if (step.contains("moduleTool")) {
                // A picker or module button: {"operation", "instance", "tool", "box", "gui"}.
                const auto call = step["moduleTool"].toObject();
                QVariantMap request{{"tool", call["tool"].toString()}};
                if (call.contains("box"))
                    request["box"] = call["box"].toVariant();
                if (call.contains("gui"))
                    request["gui"] = call["gui"].toVariant();
                toolResults->remove(call["operation"].toString() + "/" + call["tool"].toString());
                editor.runModuleTool(call["operation"].toString(), call["instance"].toInt(), request);
            } else if (step.contains("masksAddFirst")) {
                // area E: add the first shape the mask manager offers (the last "masks" result).
                const auto call = step["masksAddFirst"].toObject();
                const auto key = call["operation"].toString() + "/masks";
                const auto available = (*toolResults)[key].toMap()["masks"].toMap()["available"].toList();
                if (available.isEmpty()) {
                    qCritical() << "No shape to add" << key;
                    app.exit(2);
                    return;
                }
                QVariantMap gui{{"action", "add"}, {"id", available.first().toMap()["id"]}};
                toolResults->remove(key);
                editor.runModuleTool(call["operation"].toString(), call["instance"].toInt(),
                                     QVariantMap{{"tool", "masks"}, {"gui", gui}});
                *waiting = true;
            } else if (step.contains("toolResult")) {
                // Wait for a tool result; check its status, changed modules and reported values.
                const auto call = step["toolResult"].toObject();
                const auto key = call["operation"].toString() + "/" + call["tool"].toString();
                if (!toolResults->contains(key)) {
                    if (++*retries < 100) {
                        --*index;
                        return;
                    }
                    qCritical() << "No tool result" << key;
                    app.exit(2);
                    return;
                }
                *retries = 0;
                const auto result = (*toolResults)[key].toMap();
                bool ok = result["error"].toInt() == call["error"].toInt(0);
                if (call.contains("changed"))
                    ok = ok && result["changed"].toList() == call["changed"].toArray().toVariantList();
                const auto gui = call["gui"].toObject();
                for (auto it = gui.begin(); it != gui.end(); ++it) {
                    const auto range = it.value().toArray();
                    const double v = result["gui"].toMap()[it.key()].toDouble();
                    ok = ok && result["gui"].toMap().contains(it.key()) && v >= range[0].toDouble() &&
                         v <= range[1].toDouble();
                }
                if (call.contains("has"))
                    ok = ok && result.contains(call["has"].toString());
                qInfo().noquote() << "Tool" << key << QJsonDocument::fromVariant(result).toJson(QJsonDocument::Compact).left(400);
                if (!ok) {
                    qCritical() << "Unexpected tool result" << call << result;
                    app.exit(2);
                    return;
                }
                // area E: "consume": the next check of this tool waits for a new result (a tool
                // run from a clicked button rather than a moduleTool step).
                if (call["consume"].toBool())
                    toolResults->remove(key);
            } else if (step.contains("setParameters")) {
                const auto call = step["setParameters"].toObject();
                editor.setParameters(call["operation"].toString(), call["instance"].toInt(),
                                     call["values"].toObject().toVariantMap());
            } else if (step.contains("rejectStyle")) {
                // Saving must fail with an error that contains "error" (never a silent partial style).
                const auto call = step["rejectStyle"].toObject();
                *expectedError = call["error"].toString();
                editor.saveStyle(call["name"].toString());
                *waiting = true;
            } else if (step.contains("checkRejected")) {
                if (!expectedError->isEmpty()) {
                    if (++*retries < 100) {
                        --*index;
                        return;
                    }
                    qCritical() << "Style was not rejected";
                    app.exit(2);
                    return;
                }
                *retries = 0;
            } else if (step.contains("moduleInstance")) {
                // darktable's multi-instance menu: {operation, instance, action, name}.
                const auto call = step["moduleInstance"].toObject();
                editor.moduleInstance(call["operation"].toString(), call["instance"].toInt(),
                                      call["action"].toString(), call["name"].toString());
                *waiting = true;
            } else if (step.contains("checkInstances")) {
                // The instances of an operation in pipeline order, with their names when given.
                const auto call = step["checkInstances"].toObject();
                QJsonArray instances, names;
                for (const auto &entry : QJsonDocument::fromJson(editor.moduleCatalog().toUtf8()).array())
                    if (entry.toObject()["operation"].toString() == call["operation"].toString()) {
                        instances.append(entry.toObject()["instance"]);
                        names.append(entry.toObject()["multi_name"]);
                    }
                if (instances != call["instances"].toArray() ||
                    (call.contains("names") && names != call["names"].toArray())) {
                    if (++*retries < 100) {
                        --*index;
                        return;
                    }
                    qCritical() << "Unexpected instances" << call << instances << names;
                    app.exit(2);
                    return;
                }
                *retries = 0;
                qInfo() << "Instances" << call["operation"].toString() << instances << names;
            } else if (step.contains("resetControl")) {
                // What R and the row's reset menu do for a registered control.
                editor.resetControl(step["resetControl"].toString());
            } else if (step.contains("resetModule")) {
                const auto call = step["resetModule"].toObject();
                editor.resetModule(call["operation"].toString(), call["instance"].toInt());
            } else if (step.contains("parameterDrag")) {
                // Many edits of one path in quick succession, as a slider drag sends them.
                const auto call = step["parameterDrag"].toObject();
                *dragging = true;
                editor.setInteractive(true);
                auto *drag = new QTimer(&app);
                drag->setInterval(16);
                auto count = std::make_shared<int>(0);
                QObject::connect(
                    drag, &QTimer::timeout, &app, [&, drag, call, count, dragging, waiting, previous] {
                        const int samples = call["samples"].toInt(60);
                        const double fraction = double(++*count) / samples;
                        editor.setParameter(call["operation"].toString(), call["instance"].toInt(),
                                            call["path"].toString(),
                                            call["from"].toDouble() +
                                                fraction * (call["to"].toDouble() - call["from"].toDouble()));
                        if (*count < samples)
                            return;
                        *previous = editor.preview();
                        editor.setInteractive(false);
                        *waiting = true;
                        *dragging = false;
                        drag->stop();
                        drag->deleteLater();
                    });
                drag->start();
            } else if (step.contains("checkParameter")) {
                // The worker applies edits asynchronously: poll the catalog for a while.
                const auto call = step["checkParameter"].toObject();
                QJsonValue value;
                bool enabled = false;
                const bool found =
                    catalogValue(editor.moduleCatalog(), call["operation"].toString(),
                                 call["instance"].toInt(), call["path"].toString(), &value, &enabled);
                const bool textMatches =
                    call["value"].isString() && value.isString() &&
                    (call["contains"].toBool() ? value.toString().contains(call["value"].toString())
                                               : value.toString() == call["value"].toString());
                const bool matches =
                    found &&
                    (textMatches ||
                     (value.isDouble() &&
                      // "minimum"/"maximum" accept a range where the exact value depends on pixels.
                      (call.contains("value") ? std::abs(value.toDouble() - call["value"].toDouble()) <=
                                                    call["tolerance"].toDouble(1e-4)
                                              : value.toDouble() >= call["minimum"].toDouble(-1e30) &&
                                                    value.toDouble() <= call["maximum"].toDouble(1e30)))) &&
                    (!call.contains("enabled") || enabled == call["enabled"].toBool()) &&
                    (!call["updated"].toBool() || updatedModules->contains(call["operation"].toString()));
                if (!matches) {
                    if (++*retries < 100) {
                        --*index;
                        return;
                    }
                    qCritical() << "Unexpected parameter" << call << found << value << enabled
                                << *updatedModules;
                    app.exit(2);
                    return;
                }
                *retries = 0;
                qInfo() << "Parameter" << call["operation"].toString() << call["path"].toString()
                        << value.toVariant();
            } else if (step.contains("choices")) {
                // Request a runtime list, check it and optionally choose an item by label.
                const auto call = step["choices"].toObject();
                const auto key = QString("%1/%2/%3")
                                     .arg(call["operation"].toString())
                                     .arg(call["instance"].toInt())
                                     .arg(call["list"].toString());
                if (*retries == 0) {
                    choiceResults->remove(key);
                    editor.requestChoices(call["operation"].toString(), call["instance"].toInt(),
                                          call["list"].toString(), call["query"].toString());
                }
                if (!choiceResults->contains(key)) {
                    if (++*retries < 100) {
                        --*index;
                        return;
                    }
                    qCritical() << "No answer for list" << key;
                    app.exit(2);
                    return;
                }
                *retries = 0;
                const auto result =
                    QJsonDocument::fromJson((*choiceResults)[key].toString().toUtf8()).object();
                const auto items = result["items"].toArray();
                const int current = result["current"].toInt(-1);
                QStringList labels;
                for (const auto &item : items)
                    labels << item.toObject()["label"].toString();
                qInfo() << "Choices" << key << items.size() << "current" << current
                        << (current >= 0 ? labels.value(current) : QString()) << labels.mid(0, 8);
                const auto has = [&](const QString &text) {
                    for (const auto &label : labels)
                        if (label.contains(text))
                            return true;
                    return false;
                };
                static int listRetries = 0;
                if (items.size() < call["min"].toInt(1) ||
                    (call.contains("contains") && !has(call["contains"].toString())) ||
                    (call.contains("current") &&
                     (current < 0 || !labels.value(current).contains(call["current"].toString())))) {
                    // A list asked for right after the edit that fills it can be answered before the
                    // worker applied that edit: ask again a few times before failing.
                    if (++listRetries < 8) {
                        *retries = 1; // asked already: do not ask again while waiting
                        qInfo() << "List not ready, asking again" << key << labels.size();
                        choiceResults->remove(key);
                        editor.requestChoices(call["operation"].toString(), call["instance"].toInt(),
                                              call["list"].toString(), call["query"].toString());
                        --*index;
                        return;
                    }
                    qCritical() << "Unexpected list" << key << labels << current << result["error"];
                    app.exit(2);
                    return;
                }
                listRetries = 0;
                if (call.contains("choose")) {
                    for (const auto &item : items)
                        if (item.toObject()["label"].toString().contains(call["choose"].toString())) {
                            editor.setParameters(call["operation"].toString(), call["instance"].toInt(),
                                                 item.toObject()["set"].toObject().toVariantMap());
                            break;
                        }
                }
            } else if (step.contains("statusContains")) {
                if (!editor.status().contains(step["statusContains"].toString())) {
                    if (++*retries < 100) {
                        --*index;
                        return;
                    }
                    qCritical() << "Unexpected status" << editor.status();
                    app.exit(2);
                    return;
                }
                *retries = 0;
            } else if (step.contains("historyCount")) {
                const auto call = step["historyCount"].toObject();
                int count = 0;
                for (const auto &row : editor.history())
                    if (row.toMap()["operation"].toString() == call["operation"].toString() &&
                        row.toMap()["active"].toBool())
                        ++count;
                if (count != call["count"].toInt()) {
                    qCritical() << "Unexpected history count" << call << count;
                    app.exit(2);
                    return;
                }
            } else if (step.contains("setParameter")) {
                const auto call = step["setParameter"].toObject();
                editor.setParameter(call["operation"].toString(), call["instance"].toInt(),
                                    call["field"].toString(), call["value"].toDouble());
            } else if (step.contains("property"))
                engine.rootObjects().first()->setProperty(step["property"].toString().toUtf8().constData(),
                                                          step["value"].toVariant());
            else if (step.contains("filterSearch"))
                engine.rootObjects().first()->setProperty("filterSearch", step["filterSearch"].toString());
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
            } else if (step.contains("key") || step.contains("type")) {
                // "key" sends one combination ("Shift+R", "Down"); "type" types characters. Keys
                // go to the focused window, as typed keys do (a dialog in a window of its own).
                QWindow *window = QGuiApplication::focusWindow();
                if (!window)
                    window = qobject_cast<QQuickWindow *>(engine.rootObjects().first());
                QList<QPair<QKeyCombination, QString>> keys;
                if (step.contains("key")) {
                    const auto name = step["key"].toString();
                    keys.append({QKeySequence(name)[0], name.size() == 1 ? name : QString()});
                } else
                    for (const auto character : step["type"].toString())
                        keys.append({QKeySequence(QString(character))[0], QString(character)});
                for (int repeat = 0; repeat < std::max(1, step["repeat"].toInt(1)); ++repeat)
                    for (const auto &[key, text] : keys) {
                        QKeyEvent press(QEvent::KeyPress, key.key(), key.keyboardModifiers(), text);
                        QKeyEvent release(QEvent::KeyRelease, key.key(), key.keyboardModifiers(), text);
                        QGuiApplication::sendEvent(window, &press);
                        QGuiApplication::sendEvent(window, &release);
                    }
            } else if (step.contains("checkRoot")) {
                // Compare properties of the window, e.g. the selected control or key hints.
                const auto expected = step["checkRoot"].toObject();
                auto *root = engine.rootObjects().first();
                for (auto it = expected.begin(); it != expected.end(); ++it)
                    if (root->property(it.key().toUtf8()).toString() != it.value().toVariant().toString()) {
                        qCritical() << "Unexpected window property" << it.key() << root->property(it.key().toUtf8());
                        app.exit(2);
                        return;
                    }
            } else if (step.contains("logFocus")) {
                auto *window = qobject_cast<QQuickWindow *>(engine.rootObjects().first());
                auto *focus = window->activeFocusItem();
                qInfo() << "Focus" << step["logFocus"].toString() << focus
                        << (focus ? focus->objectName() : QString());
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
                // "label": what the newest step of that operation reads ("module • instance name").
                if (step.contains("label")) {
                    QString label;
                    for (const auto &row : editor.history())
                        if (row.toMap()["operation"].toString() == operation && row.toMap()["active"].toBool()) {
                            label = row.toMap()["label"].toString();
                            break;
                        }
                    if (label != step["label"].toString()) {
                        qCritical() << "Unexpected history label" << label << step;
                        app.exit(2);
                        return;
                    }
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
