// SPDX-License-Identifier: GPL-3.0-or-later
// Drawing on the image: the Qt-thread state of the shown on-canvas tool.
#include "editor.h"

QString Editor::canvasOverlay() const {
    return canvasJson;
}

void Editor::connectCanvas() {
    if (canvasConnected)
        return;
    canvasConnected = true;
    connect(worker.get(), &EngineWorker::canvasReady, this, [this](QString operation, int instance, QString overlay) {
        // An overlay of a tool that is no longer shown is stale.
        if (operation != canvasOperation || instance != canvasInstance || overlay == canvasJson)
            return;
        canvasJson = overlay;
        emit canvasChanged();
    });
}

void Editor::setCanvasModule(const QString &operation, int instance) {
    connectCanvas();
    const int shown = operation.isEmpty() ? -1 : instance;
    if (operation == canvasOperation && shown == canvasInstance)
        return;
    canvasOperation = operation;
    canvasInstance = shown;
    if (!canvasJson.isEmpty()) {
        canvasJson.clear();
        emit canvasChanged();
    }
    // Selecting does not touch the image; it only asks the worker to report this overlay.
    worker->moduleEdit({ActionKind::CanvasSelect, operation, shown, {}});
}

void Editor::editCanvas(const QString &operation, int instance, const QVariantMap &gesture) {
    connectCanvas();
    if (gesture.value("action").toString().isEmpty())
        return;
    queueModuleEdit({ActionKind::CanvasEdit, operation, instance, gesture});
}
