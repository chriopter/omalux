// SPDX-License-Identifier: GPL-3.0-or-later
#include "preset_catalog.h"
#include "engine/controls.h"
#include <QDir>
#include <QFileInfo>
#include <QSaveFile>
#include <QJsonDocument>
#include <QJsonObject>
#include <QJsonArray>
#include <QUuid>
#include <stdexcept>
PresetCatalog::PresetCatalog(QString directory) : root(std::move(directory)) {
}
const PresetFile *PresetCatalog::find(const QString &id) const {
    for (const auto &file : files)
        if (file.id == id)
            return &file;
    return nullptr;
}
QVariantList PresetCatalog::reload(OmEngine *engine) {
    files = discoverPresets(root);
    QVariantList catalog;
    for (const auto &preset : files) {
        QVariantMap entry{{"id", preset.id},
                          {"name", preset.name},
                          {"description", preset.description},
                          {"previewUrl", preset.previewUrl},
                          {"error", preset.error},
                          {"modules", QVariantList{}}};
        if (preset.error.isEmpty()) {
            char *raw = om_engine_style_details(engine, preset.path.toUtf8().constData(),
                                                preset.name.toUtf8().constData());
            auto details = QJsonDocument::fromJson(raw).object();
            om_engine_free_json(raw);
            entry["error"] =
                details.isEmpty() ? "Could not read style settings" : details["error"].toString();
            QVariantList modules;
            for (const auto &value : details["modules"].toArray()) {
                auto module = value.toObject().toVariantMap();
                QVariantList settings;
                for (const auto &row : module["settings"].toList()) {
                    auto setting = row.toMap();
                    const auto v = setting["value"];
                    QString display;
                    if (v.metaType().id() == QMetaType::Bool)
                        display = v.toBool() ? "on" : "off";
                    else if (v.metaType().id() == QMetaType::Double ||
                             v.metaType().id() == QMetaType::LongLong) {
                        display = QString::number(v.toDouble(), 'g', 6);
                        for (const auto &c : om_controls)
                            if (module["operation"].toString() == c.module &&
                                setting["parameter"].toString() == c.parameter)
                                display =
                                    QString::number((v.toDouble() - c.offset) / c.scale, 'f', c.decimals) +
                                    c.unit;
                        if (module["operation"].toString() == "exposure" &&
                            (setting["parameter"].toString() == "exposure" ||
                             setting["parameter"].toString() == "deflicker_target_level"))
                            display = QString::number(v.toDouble(), 'f',
                                                      setting["parameter"].toString() == "exposure" ? 3 : 2) +
                                      " EV";
                        if (module["operation"].toString() == "exposure" &&
                            setting["parameter"].toString() == "deflicker_percentile")
                            display = QString::number(v.toDouble(), 'f', 2) + "%";
                        if (module["operation"].toString() == "exposure" &&
                            setting["parameter"].toString() == "black")
                            display = QString::number(v.toDouble(), 'f', 4);
                    } else if (v.metaType().id() == QMetaType::QVariantList ||
                               v.metaType().id() == QMetaType::QVariantMap)
                        display =
                            QString::fromUtf8(QJsonDocument::fromVariant(v).toJson(QJsonDocument::Compact));
                    else
                        display = v.toString();
                    setting["display"] = display;
                    settings.append(setting);
                }
                module["settings"] = settings;
                modules.append(module);
            }
            entry["modules"] = modules;
        }
        catalog.append(entry);
    }
    return catalog;
}
QString PresetCatalog::save(OmEngine *engine, const QString &name, const QString &source) {
    const QString relative = "my-presets/" + QUuid::createUuid().toString(QUuid::WithoutBraces);
    const QDir directoryRoot(root);
    const QString directory = directoryRoot.filePath(relative);
    char *raw = om_engine_snapshot(engine, name.toUtf8().constData(), relative.toUtf8().constData());
    if (!raw)
        throw std::runtime_error(
            "This recipe contains masks, instances or external assets that cannot yet be saved portably");
    const auto snapshot = QJsonDocument::fromJson(raw).object();
    om_engine_free_json(raw);
    bool okay = QDir().mkpath(directory);
    QJsonArray assets;
    for (const auto &value : snapshot["assets"].toArray()) {
        auto asset = value.toObject();
        const QString origin =
            QFileInfo(directoryRoot.filePath(asset["source"].toString())).canonicalFilePath();
        const QString target = QDir(directory).filePath(asset["path"].toString());
        okay = okay && origin.startsWith(directoryRoot.canonicalPath() + "/") &&
               QDir().mkpath(QFileInfo(target).absolutePath()) && QFile::copy(origin, target);
        asset.remove("source");
        assets.append(asset);
    }
    QSaveFile style(QDir(directory).filePath("preset.dtstyle"));
    const auto xml = snapshot["xml"].toString().toUtf8();
    okay = okay && style.open(QIODevice::WriteOnly) && style.write(xml) == xml.size() && style.commit();
    QJsonObject manifest{{"version", 1}};
    if (!assets.isEmpty())
        manifest["assets"] = assets;
    const QDir repository(QDir::cleanPath(QFileInfo(QStringLiteral(OMALUX_QML)).absolutePath() + "/../.."));
    const auto relativeSource = repository.relativeFilePath(source);
    manifest["preview"] = QJsonObject{{"source", relativeSource.startsWith("../") ? source : relativeSource},
                                      {"darktable_version", om_engine_version()}};
    QSaveFile file(QDir(directory).filePath("preset.json"));
    const auto bytes = QJsonDocument(manifest).toJson();
    okay = okay && file.open(QIODevice::WriteOnly) && file.write(bytes) == bytes.size() && file.commit();
    if (!okay) {
        QDir(directory).removeRecursively();
        throw std::runtime_error("Could not save preset bundle");
    }
    return directory;
}
void PresetCatalog::finishPreview(const QString &directory, const QImage &image) {
    if (!image.scaled(384, 256, Qt::KeepAspectRatio, Qt::SmoothTransformation)
             .save(QDir(directory).filePath("thumbnail.jpg"), "JPG", 90)) {
        QDir(directory).removeRecursively();
        throw std::runtime_error("Could not save preset thumbnail");
    }
}
void PresetCatalog::remove(const QString &id) {
    const QDir directoryRoot(root);
    const QString directory = QFileInfo(directoryRoot.filePath(id)).canonicalPath();
    if (!id.startsWith("my-presets/") ||
        !directory.startsWith(directoryRoot.canonicalPath() + "/my-presets/"))
        throw std::runtime_error("Only user presets can be deleted");
    if (!QDir(directory).removeRecursively())
        throw std::runtime_error("Could not delete preset");
}
static bool copyDirectory(const QString &source, const QString &destination) {
    if (!QDir().mkpath(destination))
        return false;
    for (const auto &file : QDir(source).entryInfoList(QDir::Files | QDir::Dirs | QDir::NoDotAndDotDot)) {
        if (file.isSymLink())
            return false;
        const auto target = QDir(destination).filePath(file.fileName());
        if (file.isDir() ? !copyDirectory(file.absoluteFilePath(), target)
                         : !QFile::copy(file.absoluteFilePath(), target))
            return false;
    }
    return true;
}
void PresetCatalog::exportBundle(const QString &id, const QString &destination) {
    const QDir directoryRoot(root);
    const QString source = QFileInfo(directoryRoot.filePath(id)).canonicalPath();
    if (!source.startsWith(directoryRoot.canonicalPath() + "/"))
        throw std::runtime_error("Invalid preset path");
    const QString target = QDir(destination).filePath(QFileInfo(id).path());
    if (QFileInfo::exists(target))
        throw std::runtime_error("Export destination already exists");
    const auto absoluteTarget = QDir::cleanPath(QFileInfo(target).absoluteFilePath());
    if (absoluteTarget == source || absoluteTarget.startsWith(source + "/"))
        throw std::runtime_error("Choose a destination outside the source bundle");
    if (!copyDirectory(source, target)) {
        QDir(target).removeRecursively();
        throw std::runtime_error("Could not export preset bundle");
    }
}
