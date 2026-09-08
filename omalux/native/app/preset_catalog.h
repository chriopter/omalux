// SPDX-License-Identifier: GPL-3.0-or-later
#pragma once
#include "engine/engine.h"
#include "presets.h"
#include <QVariantList>
#include <QImage>
// All catalogue parsing and bundle I/O runs on the engine worker.
class PresetCatalog {
  public:
    explicit PresetCatalog(QString root);
    QVariantList reload(OmEngine *engine);
    const PresetFile *find(const QString &id) const;
    QString save(OmEngine *, const QString &name, const QString &source);
    void finishPreview(const QString &directory, const QImage &image);
    void remove(const QString &id);
    void exportBundle(const QString &id, const QString &destination);

  private:
    QString root;
    std::vector<PresetFile> files;
};
