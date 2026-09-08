// SPDX-License-Identifier: GPL-3.0-or-later
#pragma once
#include <QDir>
#include <QFile>
#include <QXmlStreamReader>
#include <vector>

struct PresetFile {
    QString id, path, name, description, error;
};

inline std::vector<PresetFile> discoverPresets(const QString &directory) {
    std::vector<PresetFile> result;
    QDir dir(directory);
    for(const auto &entry:dir.entryInfoList({"*.dtstyle"}, QDir::Files, QDir::Name)) {
        PresetFile preset{entry.fileName(), entry.absoluteFilePath(), entry.completeBaseName(), {}, {}};
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
        result.push_back(preset);
    }
    for(auto &preset:result) {
        int count=0; for(const auto &other:result) if(other.name==preset.name) ++count;
        if(count>1) preset.error="Duplicate style name; give each style a unique name";
    }
    return result;
}
