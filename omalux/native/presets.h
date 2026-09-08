// SPDX-License-Identifier: GPL-3.0-or-later
#pragma once
#include <QDir>
#include <QJsonDocument>
#include <QJsonObject>
#include <QDirIterator>
#include <QUrl>
#include <algorithm>
#include <QFile>
#include <QXmlStreamReader>
#include <vector>

struct PresetFile {
    QString id, path, name, description, error, previewUrl;
};

inline std::vector<PresetFile> discoverPresets(const QString &directory) {
    std::vector<PresetFile> result;
    QDir dir(directory);
    const auto assetErrors = QJsonDocument::fromJson(qgetenv("OMALUX_PRESET_ASSET_ERRORS")).object();
    QDirIterator files(directory, {"*.dtstyle"}, QDir::Files | QDir::NoSymLinks, QDirIterator::Subdirectories);
    while(files.hasNext()) {
        files.next(); const auto entry=files.fileInfo();
        PresetFile preset{dir.relativeFilePath(entry.absoluteFilePath()), entry.absoluteFilePath(), entry.completeBaseName(), {}, {}, {}};
        const QFileInfo thumbnail(entry.dir().filePath("thumbnail.jpg"));
        if(thumbnail.isFile()) preset.previewUrl=QUrl::fromLocalFile(thumbnail.absoluteFilePath()).toString();
        QFile file(preset.path);
        if(!file.open(QIODevice::ReadOnly)) preset.error=file.errorString();
        else {
            QXmlStreamReader xml(&file);
            bool foundName=false, foundStyle=false;
            if(!xml.readNextStartElement() || xml.name()!=QStringLiteral("darktable_style"))
                preset.error="Not a darktable style";
            else while(xml.readNextStartElement()) {
                if(xml.name()==QStringLiteral("info")) {
                    while(xml.readNextStartElement()) {
                        if(xml.name()==QStringLiteral("name")) {preset.name=xml.readElementText(); foundName=!preset.name.trimmed().isEmpty();}
                        else if(xml.name()==QStringLiteral("description")) preset.description=xml.readElementText();
                        else if(xml.name()==QStringLiteral("iop_list")) {
                            if(!xml.readElementText().trimmed().isEmpty()) preset.error="Custom module order is not supported yet";
                        }
                        else xml.skipCurrentElement();
                    }
                } else if(xml.name()==QStringLiteral("style")) {
                    foundStyle=true;
                    while(xml.readNextStartElement()) {
                        if(xml.name()!=QStringLiteral("plugin")) {xml.skipCurrentElement(); continue;}
                        while(xml.readNextStartElement()) {
                            if(xml.name()==QStringLiteral("operation")) {
                                if(xml.readElementText()==QStringLiteral("mask_manager")) preset.error="Styles containing drawn masks are not supported yet";
                            } else xml.skipCurrentElement();
                        }
                    }
                } else xml.skipCurrentElement();
            }
            while(!xml.atEnd()) xml.readNext();
            if(xml.hasError()) preset.error=xml.errorString();
            else if(!foundName || !foundStyle) preset.error="Style name or settings missing";
        }
        if(assetErrors.contains(preset.id)) preset.error = assetErrors.value(preset.id).toString();
        result.push_back(preset);
    }
    for(auto &preset:result) {
        int count=0; for(const auto &other:result) if(other.name==preset.name) ++count;
        if(count>1) preset.error="Duplicate style name; give each style a unique name";
    }
    std::sort(result.begin(),result.end(),[](const auto &a,const auto &b){return a.id<b.id;});
    return result;
}
