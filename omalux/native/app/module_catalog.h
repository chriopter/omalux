// SPDX-License-Identifier: GPL-3.0-or-later
#pragma once
#include "engine/engine.h"
#include <QHash>
#include <QString>
#include <QStringList>
// Worker-owned copy of darktable's module description. Describing every module costs
// several milliseconds, so after an edit only the touched modules are described again.
class ModuleCatalog {
  public:
    // Describe every module again; needed when the module stack itself may have changed.
    void reload(OmEngine *);
    // Describe one module again and return its entry, or an empty string if it is unknown.
    QString update(OmEngine *, const QString &operation, int instance);
    // The whole catalog as a JSON array of the module entries.
    QString json() const;
    // True once after reload() or update() changed anything since the last call.
    bool takeChanged();

  private:
    QStringList entries;
    QHash<QString, int> positions;
    bool changed = false;
};
