// SPDX-License-Identifier: GPL-3.0-or-later
#include "comparison_bridge.h"
#include "engine/canvas.h"
#include <QDir>
#include <QFileInfo>
// darktable styles carry no drawn forms (common/styles.c), so a module that uses them, or a
// history step with them, reaches the comparison process as a complete XMP sidecar which its
// Lua bridge applies with image:apply_sidecar (lua/image.c:96). Like a history snapshot it
// starts a new epoch: the comparison replaces its whole history and replays later controls.
bool ComparisonBridge::sidecar(OmEngine *engine, quint64 revision) {
    if (mailbox.isEmpty())
        return true;
    const QString name = "omalux-sidecar-" + QString::number(revision);
    const QString path = QDir(QFileInfo(mailbox).absolutePath()).filePath(name + ".xmp");
    if (om_engine_canvas_sidecar(engine, path.toUtf8().constData()))
        return false;
    ++epoch;
    journal = "sidecar " + QByteArray::number(epoch) + " " + name.toUtf8() + "\n";
    return true;
}
