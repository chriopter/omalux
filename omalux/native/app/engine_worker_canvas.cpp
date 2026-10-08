// SPDX-License-Identifier: GPL-3.0-or-later
// The worker side of drawing on the image: gestures go through the module edit queue, the
// overlay of the shown tool is re-read after every render (the transforms need the pipe the
// render just synchronised) and sent only when it changed.
#include "engine_worker.h"
#include "engine/canvas.h"
#include <QJsonDocument>
#include <QJsonObject>

// Consecutive drag steps of the same gesture merge while the worker is busy, as slider
// values do: a move keeps its first start and takes the last end, absolute positions take
// the last one, scale factors multiply.
bool EngineWorker::mergeCanvasEdit(ModuleEdit &last, const ModuleEdit &edit) {
    if (last.kind != ActionKind::CanvasEdit || edit.kind != ActionKind::CanvasEdit ||
        last.operation != edit.operation || last.instance != edit.instance)
        return false;
    const QString action = edit.values.value("action").toString();
    if (last.values.value("action").toString() != action)
        return false;
    for (const char *key : {"id", "index", "part"})
        if (last.values.value(key) != edit.values.value(key))
            return false;
    if (action == "move" || action == "move-source") {
        if (last.values.contains("index")) {
            last.values["to"] = edit.values.value("to");
            return true;
        }
        if (last.values.value("to") != edit.values.value("from"))
            return false;
        last.values["to"] = edit.values.value("to");
        return true;
    }
    if (action == "rotate") {
        last.values["at"] = edit.values.value("at");
        return true;
    }
    if (action == "scale" || action == "feather") {
        last.values["factor"] = last.values.value("factor").toDouble() * edit.values.value("factor").toDouble();
        return true;
    }
    return false;
}

int EngineWorker::applyCanvasEdit(OmEngine *engine, const ModuleEdit &edit) {
    if (edit.kind == ActionKind::CanvasSelect) {
        canvasOperation = edit.operation;
        canvasInstance = edit.instance;
        publishedCanvas = QStringLiteral("\x01"); // differs from any overlay: publish once
        return 0;
    }
    const auto gesture = QJsonDocument(QJsonObject::fromVariantMap(edit.values)).toJson(QJsonDocument::Compact);
    return om_engine_canvas_edit(engine, edit.operation.toUtf8().constData(), edit.instance, gesture.constData());
}

void EngineWorker::publishCanvas(OmEngine *engine) {
    QString overlay;
    if (canvasInstance >= 0 && !canvasOperation.isEmpty()) {
        char *raw = om_engine_canvas(engine, canvasOperation.toUtf8().constData(), canvasInstance);
        overlay = QString::fromUtf8(raw ? raw : "");
        om_engine_free_json(raw);
    }
    if (overlay == publishedCanvas)
        return;
    publishedCanvas = overlay;
    emit canvasReady(canvasOperation, canvasInstance, overlay);
}
