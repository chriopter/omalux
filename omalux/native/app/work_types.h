// SPDX-License-Identifier: GPL-3.0-or-later
#pragma once
#include "engine/controls.h"
#include <QImage>
#include <QString>
#include <QMetaType>
#include <array>
using ControlValues = std::array<float, OM_CONTROL_COUNT>;
using ControlRevisions = std::array<quint64, OM_CONTROL_COUNT>;
struct WorkTicket {
    quint64 revision = 1, epoch = 0;
};
enum class ActionKind {
    None,
    ApplyPreset,
    History,
    Halation,
    Open,
    ExportImage,
    SavePreset,
    DeletePreset,
    ExportPreset
};
struct EditorAction {
    ActionKind kind = ActionKind::None;
    QString value, destination;
    int quality = 90;
};
struct RenderResult {
    WorkTicket ticket;
    QImage image;
    QString status, gpuWarning;
};
Q_DECLARE_METATYPE(ControlValues)
Q_DECLARE_METATYPE(WorkTicket)
Q_DECLARE_METATYPE(RenderResult)
