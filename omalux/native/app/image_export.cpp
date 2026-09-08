// SPDX-License-Identifier: GPL-3.0-or-later
#include "image_export.h"
#include <QDir>
#include <QFileInfo>
#include <QSaveFile>
#include <QUuid>
#include <stdexcept>
void exportImage(OmEngine *engine, const QString &destination, int quality) {
    const QString extension = QFileInfo(destination).suffix().toLower() == "png" ? "png" : "jpeg";
    const QString temporary =
        QDir(QFileInfo(destination).absolutePath())
            .filePath(".omalux-export-" + QUuid::createUuid().toString(QUuid::WithoutBraces) + "." +
                      extension);
    if (om_engine_export(engine, temporary.toUtf8().constData(), extension.toUtf8().constData(), quality)) {
        QFile::remove(temporary);
        throw std::runtime_error("Image export failed");
    }
    QFile input(temporary);
    QSaveFile output(destination);
    bool okay = input.open(QIODevice::ReadOnly) && output.open(QIODevice::WriteOnly);
    while (okay && !input.atEnd()) {
        const auto bytes = input.read(1024 * 1024);
        okay = !bytes.isEmpty() && output.write(bytes) == bytes.size();
    }
    okay = okay && output.commit();
    input.close();
    QFile::remove(temporary);
    if (!okay)
        throw std::runtime_error("Could not publish exported image");
}
