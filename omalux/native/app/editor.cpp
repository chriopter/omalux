// SPDX-License-Identifier: GPL-3.0-or-later
#include "editor.h"
#include <QFileInfo>
#include <QDebug>
#include <cmath>
QString Editor::preview() const {
    return hoverUrl.isEmpty() ? url : hoverUrl;
}
QString Editor::status() const {
    return hoverUrl.isEmpty() ? message : "Preset preview · click to apply";
}
QString Editor::gpuWarning() const {
    return gpuMessage;
}
QString Editor::filename() const {
    return QFileInfo(source).fileName();
}
QVariantList Editor::history() const {
    return historyRows;
}
QVariantMap Editor::metadata() const {
    return imageMetadata;
}
bool Editor::styleBusy() const {
    return applyingStyle;
}
QString Editor::applyingPreset() const {
    return applyingId;
}
QString Editor::activeStyle() const {
    return styleName;
}
QVariantList Editor::presets() const {
    return presetCatalog;
}
bool Editor::presetsReady() const {
    return catalogReady;
}
QString Editor::presetError() const {
    return styleError;
}
QVariantList Editor::controls() const {
    QVariantList result;
    for (unsigned int i = 0; i < OM_CONTROL_COUNT; ++i) {
        const auto &c = om_controls[i];
        result.append(
            QVariantMap{{"id", c.id},
                        {"module", c.module},
                        {"label", c.label},
                        {"minimum", c.minimum},
                        {"maximum", c.maximum},
                        {"softMinimum", c.soft_minimum != c.soft_maximum ? c.soft_minimum : c.minimum},
                        {"softMaximum", c.soft_minimum != c.soft_maximum ? c.soft_maximum : c.maximum},
                        {"step", c.step},
                        {"unit", c.unit},
                        {"decimals", c.decimals},
                        {"group", c.group},
                        {"section", c.section},
                        {"colors", c.colors},
                        {"detail", bool(c.detail)},
                        {"initial", c.initial}});
    }
    return result;
}
QVariantMap Editor::controlValues() const {
    QVariantMap result;
    for (unsigned int i = 0; i < OM_CONTROL_COUNT; ++i)
        result[om_controls[i].id] = values[i];
    return result;
}
void Editor::setControl(const QString &id, double next) {
    if (applyingStyle || url.isEmpty() || !std::isfinite(next))
        return;
    for (unsigned int i = 0; i < OM_CONTROL_COUNT; ++i) {
        const auto &c = om_controls[i];
        if (id != QLatin1String(c.id))
            continue;
        next = qBound(double(c.minimum), next, double(c.maximum));
        if (values[i] == next)
            return;
        values[i] = next;
        queueControls(int(i));
        return;
    }
}
void Editor::setControls(const QVariantMap &updates) {
    if (applyingStyle || url.isEmpty())
        return;
    for (auto it = updates.begin(); it != updates.end(); ++it)
        setControl(it.key(), it.value().toDouble());
}
void Editor::adjustControl(const QString &id, int steps) {
    for (unsigned int i = 0; i < OM_CONTROL_COUNT; ++i)
        if (id == QLatin1String(om_controls[i].id))
            setControl(id, values[i] + steps * om_controls[i].step);
}
void Editor::resetControl(const QString &id) {
    for (const auto &c : om_controls)
        if (id == c.id)
            setControl(id, c.initial);
}
Editor::Editor(Frames *normal, Frames *hover, QString image, std::vector<QByteArray> arguments)
    : worker(std::make_unique<EngineWorker>(image, std::move(arguments),
                                            qEnvironmentVariable("OMALUX_PRESETS_DIR"),
                                            qEnvironmentVariable("OMALUX_COMPARISON_MAILBOX"))),
      frames(normal), hoverFrames(hover), source(std::move(image)) {
    for (unsigned i = 0; i < OM_CONTROL_COUNT; ++i)
        values[i] = om_controls[i].initial;
    connect(worker.get(), &EngineWorker::initialized, this,
            [this](ControlValues initial, QVariantMap metadata, QVariantList presets) {
                values = initial;
                imageMetadata = metadata;
                presetCatalog = presets;
                catalogReady = true;
                emit controlsChanged();
                emit presetsChanged();
                emit changed();
            });
    connect(worker.get(), &EngineWorker::controlsReady, this, [this](ControlValues next, quint64 revision) {
        if (revision != requestedTicket.revision)
            return;
        values = next;
        emit controlsChanged();
    });
    connect(worker.get(), &EngineWorker::metadataReady, this, [this](QString image, QVariantMap metadata) {
        source = image;
        imageMetadata = metadata;
        emit changed();
    });
    connect(worker.get(), &EngineWorker::historyReady, this, [this](QVariantList rows) {
        if (historyRows != rows) {
            historyRows = rows;
            emit historyChanged();
        }
    });
    connect(worker.get(), &EngineWorker::presetsReady, this, [this](QVariantList presets) {
        presetCatalog = presets;
        emit presetsChanged();
    });
    connect(worker.get(), &EngineWorker::styleReady, this, [this](QString name) {
        styleName = name;
        emit changed();
    });
    connect(worker.get(), &EngineWorker::frameReady, this, &Editor::showFrame);
    connect(worker.get(), &EngineWorker::hoverReady, this, [this](QImage image, quint64 revision) {
        if (revision != hoverRevision || hoverId.isEmpty())
            return;
        hoverFrames->set(image);
        hoverUrl = QString("image://hover/%1").arg(revision);
        emit changed();
    });
    connect(worker.get(), &EngineWorker::failed, this, [this](WorkTicket ticket, QString error) {
        if (ticket.epoch != requestedTicket.epoch)
            return;
        message = error;
        if (applyingStyle)
            styleError = error;
        applyingStyle = false;
        emit changed();
    });
    worker->start();
}
Editor::~Editor() {
    worker.reset();
}
void Editor::showFrame(RenderResult result) {
    // Intermediate slider results are useful; obsolete session/quality epochs are not.
    if (result.ticket.epoch != requestedTicket.epoch || result.ticket.revision <= presentedRevision)
        return;
    presentedRevision = result.ticket.revision;
    gpuMessage = result.gpuWarning;
    message = result.status;
    applyingStyle = false;
    frames->set(result.image);
    url = QString("image://preview/%1").arg(result.ticket.revision);
    qInfo().noquote() << message;
    emit changed();
}
bool Editor::presetAvailable(const QString &id) const {
    for (const auto &entry : presetCatalog)
        if (entry.toMap()["id"].toString() == id)
            return entry.toMap()["error"].toString().isEmpty();
    return false;
}
void Editor::hoverPreset(const QString &id, bool active) {
    if (active) {
        if (applyingStyle || url.isEmpty() || !presetAvailable(id) || hoverId == id)
            return;
    } else if (hoverId.isEmpty() || (!id.isEmpty() && hoverId != id))
        return;
    hoverId = active ? id : QString();
    hoverRevision = worker->hover(hoverId);
    hoverUrl.clear();
    emit changed();
}
void Editor::queueAction(EditorAction action) {
    if (applyingStyle || url.isEmpty())
        return;
    hoverPreset("", false);
    applyingStyle = true;
    styleError.clear();
    message = "Working…";
    requestedTicket = worker->action(std::move(action));
    emit changed();
}
void Editor::queueControls(int index) {
    hoverPreset("", false);
    requestedTicket = worker->controls(values, index);
    emit controlsChanged();
}
void Editor::setInteractive(bool active) {
    requestedTicket = worker->interactive(active);
}
void Editor::applyPreset(const QString &id) {
    if (applyingStyle || url.isEmpty())
        return;
    if (!presetAvailable(id)) {
        styleError = "This preset is unavailable";
        emit changed();
        return;
    }
    applyingId = id;
    queueAction({ActionKind::ApplyPreset, id});
}
void Editor::selectHistory(int step) {
    for (const auto &row : historyRows)
        if (row.toMap()["step"].toInt() == step) {
            if (!row.toMap()["current"].toBool())
                queueAction({ActionKind::History, QString::number(step)});
            return;
        }
}
void Editor::applyHalation() {
    queueAction({ActionKind::Halation});
}
void Editor::openPhoto(const QUrl &url) {
    const QString path = url.toLocalFile();
    if (!QFileInfo(path).isFile()) {
        message = "Image does not exist";
        emit changed();
        return;
    }
    queueAction({ActionKind::Open, path});
}
void Editor::savePreset(const QString &name) {
    if (name.trimmed().isEmpty())
        return;
    for (const auto &p : presetCatalog)
        if (p.toMap()["name"].toString() == name.trimmed()) {
            styleError = "A preset with this name already exists";
            emit changed();
            return;
        }
    queueAction({ActionKind::SavePreset, name.trimmed()});
}
void Editor::exportPhoto(const QUrl &url, int quality) {
    const QString path = url.toLocalFile();
    if (path == source || QFileInfo(path).canonicalFilePath() == QFileInfo(source).canonicalFilePath()) {
        message = "Choose a different output file to preserve the original";
        emit changed();
        return;
    }
    if (!path.endsWith(".jpg", Qt::CaseInsensitive) && !path.endsWith(".jpeg", Qt::CaseInsensitive) &&
        !path.endsWith(".png", Qt::CaseInsensitive)) {
        message = "Choose a .jpg or .png output filename";
        emit changed();
        return;
    }
    queueAction({ActionKind::ExportImage, path, {}, qBound(1, quality, 100)});
}
void Editor::deletePreset(const QString &id) {
    if (id.startsWith("my-presets/"))
        queueAction({ActionKind::DeletePreset, id});
}
void Editor::exportPreset(const QString &id, const QUrl &destination) {
    queueAction({ActionKind::ExportPreset, id, destination.toLocalFile()});
}
