// SPDX-License-Identifier: GPL-3.0-or-later
#include "style_bundles.h"
#include <QDir>
#include <QJsonDocument>
#include <QJsonObject>
#include <QDirIterator>
#include <QUrl>
#include <algorithm>
#include <QFile>
#include <QXmlStreamReader>
#include <vector>

std::vector<StyleFile> discoverStyles(const QString &directory) {
    std::vector<StyleFile> result;
    QDir dir(directory);
    const auto assetErrors = QJsonDocument::fromJson(qgetenv("OMALUX_STYLE_ASSET_ERRORS")).object();
    QDirIterator files(directory, {"*.dtstyle"}, QDir::Files | QDir::NoSymLinks,
                       QDirIterator::Subdirectories);
    while (files.hasNext()) {
        files.next();
        const auto entry = files.fileInfo();
        StyleFile style{dir.relativeFilePath(entry.absoluteFilePath()),
                          entry.absoluteFilePath(),
                          entry.completeBaseName(),
                          {},
                          {},
                          {}};
        const QFileInfo thumbnail(entry.dir().filePath("thumbnail.jpg"));
        if (thumbnail.isFile())
            style.previewUrl = QUrl::fromLocalFile(thumbnail.absoluteFilePath()).toString();
        QFile file(style.path);
        if (!file.open(QIODevice::ReadOnly))
            style.error = file.errorString();
        else {
            QXmlStreamReader xml(&file);
            bool foundName = false, foundStyle = false;
            if (!xml.readNextStartElement() || xml.name() != QStringLiteral("darktable_style"))
                style.error = "Not a darktable style";
            else
                while (xml.readNextStartElement()) {
                    if (xml.name() == QStringLiteral("info")) {
                        while (xml.readNextStartElement()) {
                            if (xml.name() == QStringLiteral("name")) {
                                style.name = xml.readElementText();
                                foundName = !style.name.trimmed().isEmpty();
                            } else if (xml.name() == QStringLiteral("description"))
                                style.description = xml.readElementText();
                            else if (xml.name() == QStringLiteral("iop_list")) {
                                if (!xml.readElementText().trimmed().isEmpty())
                                    style.error = "Custom module order is not supported yet";
                            } else
                                xml.skipCurrentElement();
                        }
                    } else if (xml.name() == QStringLiteral("style")) {
                        foundStyle = true;
                        while (xml.readNextStartElement()) {
                            if (xml.name() != QStringLiteral("plugin")) {
                                xml.skipCurrentElement();
                                continue;
                            }
                            while (xml.readNextStartElement()) {
                                if (xml.name() == QStringLiteral("operation")) {
                                    if (xml.readElementText() == QStringLiteral("mask_manager"))
                                        style.error = "Styles containing drawn masks are not supported yet";
                                } else
                                    xml.skipCurrentElement();
                            }
                        }
                    } else
                        xml.skipCurrentElement();
                }
            while (!xml.atEnd())
                xml.readNext();
            if (xml.hasError())
                style.error = xml.errorString();
            else if (!foundName || !foundStyle)
                style.error = "Style name or settings missing";
        }
        if (assetErrors.contains(style.id))
            style.error = assetErrors.value(style.id).toString();
        result.push_back(style);
    }
    for (auto &style : result) {
        int count = 0;
        for (const auto &other : result)
            if (other.name == style.name)
                ++count;
        if (count > 1)
            style.error = "Duplicate style name; give each style a unique name";
    }
    std::sort(result.begin(), result.end(), [](const auto &a, const auto &b) { return a.id < b.id; });
    return result;
}
