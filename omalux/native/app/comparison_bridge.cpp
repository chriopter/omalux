// SPDX-License-Identifier: GPL-3.0-or-later
#include "comparison_bridge.h"
#include <QDir>
#include <QFileInfo>
#include <QSaveFile>
#include <QJsonDocument>
#include <QJsonObject>
#include <QUrl>
#include <QDebug>
ComparisonBridge::ComparisonBridge(QString path) : mailbox(std::move(path)) {
}
void ComparisonBridge::reset() {
    epoch = 0;
    journal.clear();
}
bool ComparisonBridge::writeSnapshot(const QString &name, char *raw) {
    if (!raw)
        return false;
    const auto xml = QJsonDocument::fromJson(raw).object()["xml"].toString().toUtf8();
    om_engine_free_json(raw);
    QSaveFile file(QDir(QFileInfo(mailbox).absolutePath()).filePath(name + ".dtstyle"));
    return !xml.isEmpty() && file.open(QIODevice::WriteOnly) && file.write(xml) == xml.size() &&
           file.commit();
}
void ComparisonBridge::modules(OmEngine *engine, const QStringList &modules, quint64 revision) {
    if (mailbox.isEmpty())
        return;
    for (const auto &module : modules) {
        const QString name = "omalux-sync-" + module + "-" + QString::number(revision);
        if (writeSnapshot(name, om_engine_module_snapshot(engine, name.toUtf8().constData(),
                                                          module.toUtf8().constData())))
            journal += "module " + QByteArray::number(epoch) + " " + QByteArray::number(revision) + " " +
                       module.toUtf8() + " " + name.toUtf8() + "\n";
        else
            qWarning() << "Could not synchronize module" << module;
    }
}
bool ComparisonBridge::history(OmEngine *engine, quint64 revision) {
    if (mailbox.isEmpty())
        return true;
    const QString name = "omalux-history-" + QString::number(revision);
    if (!writeSnapshot(name, om_engine_history_snapshot(engine, name.toUtf8().constData())))
        return false;
    ++epoch;
    journal = "history " + QByteArray::number(epoch) + " " + name.toUtf8() + "\n";
    return true;
}
void ComparisonBridge::publish(const QString &source, quint64 revision, const ControlValues &values,
                               const ControlRevisions &revisions) {
    if (mailbox.isEmpty())
        return;
    QByteArray command = "omalux-controls-v3 " + QByteArray::number(revision) + "\nsource " +
                         QUrl::toPercentEncoding(source) + "\n" + journal;
    for (unsigned i = 0; i < OM_CONTROL_COUNT; ++i) {
        if (!strcmp(om_controls[i].action, "@recipe"))
            continue;
        command += "control " + QByteArray::number(epoch) + " " + om_controls[i].module + " " +
                   QUrl::toPercentEncoding(om_controls[i].action) + " " +
                   QByteArray::number(om_parameter_value(i, values[i]), 'g', 9) + " " +
                   QByteArray::number(revisions[i]) + "\n";
    }
    QSaveFile file(mailbox);
    if (!file.open(QIODevice::WriteOnly) || file.write(command) != command.size() || !file.commit())
        qWarning() << "Could not synchronize comparison controls:" << file.errorString();
}
