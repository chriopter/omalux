// SPDX-License-Identifier: GPL-3.0-or-later
#pragma once
#include "engine/engine.h"
#include "work_types.h"
#include <QStringList>
// Worker-owned, one-way development comparison. Hover never calls this bridge.
class ComparisonBridge {
  public:
    explicit ComparisonBridge(QString mailbox);
    void reset();
    void modules(OmEngine *, const QStringList &, quint64 revision);
    bool history(OmEngine *, quint64 revision);
    void publish(const QString &source, quint64 revision, const ControlValues &, const ControlRevisions &);

  private:
    QString mailbox;
    quint64 epoch = 0;
    QByteArray journal;
    bool writeSnapshot(const QString &name, char *json);
};
