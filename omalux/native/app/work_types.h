// SPDX-License-Identifier: GPL-3.0-or-later
#pragma once
#include "engine/controls.h"
#include <QImage>
#include <QVector4D>
#include <QString>
#include <QVariantMap>
#include <QMetaType>
#include <array>
using ControlValues = std::array<float, OM_CONTROL_COUNT>;
using ControlRevisions = std::array<quint64, OM_CONTROL_COUNT>;
struct WorkTicket {
    quint64 revision = 1, epoch = 0;
};
enum class ActionKind {
    None,
    ApplyStyle,
    History,
    Halation,
    SetParameters,
    ResetModule,
    Open,
    ExportImage,
    SaveStyle,
    DeleteStyle,
    ExportStyle,
    // darktable's multi-instance menu; ModuleEdit.values carries "action" and "name".
    ModuleInstance,
    // A darktable picker or module button (engine/module_tools.h); values hold the request.
    ModuleTool,
    // A camera preset put on the image by hand or taken off; values carry "name" and "on".
    CameraPreset,
    // Drawing on the image (engine/canvas.h): one gesture, or which module's tool is shown.
    CanvasEdit,
    CanvasSelect
};
struct EditorAction {
    ActionKind kind = ActionKind::None;
    QString value, destination;
    int quality = 90;
};
// A generic edit of one module instance, addressed through darktable's introspection.
// SetParameters carries {path: value}; ResetModule restores the module's defaults;
// CanvasEdit carries one gesture of the module's on-canvas tool.
struct ModuleEdit {
    ActionKind kind = ActionKind::SetParameters;
    QString operation;
    int instance = 0;
    QVariantMap values;
};
struct RenderResult {
    WorkTicket ticket;
    QImage image;
    QString status, gpuWarning;
    double aspectRatio = 1;
    QVector4D textureTransform{1, 1, 0, 0};
};
Q_DECLARE_METATYPE(ControlValues)
Q_DECLARE_METATYPE(WorkTicket)
Q_DECLARE_METATYPE(RenderResult)
