// SPDX-License-Identifier: GPL-3.0-or-later
#include "editor.h"
#include <QFileInfo>
#include <QDebug>
#include <cmath>
QString Editor::preview() const {
    return hoverUrl.isEmpty() ? url : hoverUrl;
}
double Editor::previewAspectRatio() const {
    return hoverUrl.isEmpty() ? normalAspectRatio : hoverAspectRatio;
}
QVector4D Editor::previewTextureTransform() const {
    return hoverUrl.isEmpty() ? normalTextureTransform : hoverTextureTransform;
}
QString Editor::status() const {
    return hoverUrl.isEmpty() ? message : "Style preview · click to apply";
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
QVariantList Editor::cameraDefaults() const {
    return imageCameraDefaults;
}
QString Editor::moduleCatalog() const {
    return modules;
}
void Editor::setParameter(const QString &operation, int instance, const QString &field, double value) {
    worker->parameter(operation, instance, field, value);
}
bool Editor::styleBusy() const {
    return applying;
}
QString Editor::applyingStyle() const {
    return applyingId;
}
QString Editor::activeStyle() const {
    return styleName;
}
QVariantList Editor::styles() const {
    return styleCatalog;
}
bool Editor::stylesReady() const {
    return catalogReady;
}
QString Editor::styleError() const {
    return errorText;
}
// Set OMALUX_DEV=1 to get the tools we build the interface with, such as the switch
// between the curated panel and the parameters that still have no designed control.
bool Editor::developerMode() const {
    return qEnvironmentVariable("OMALUX_DEV") == QLatin1String("1");
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
    if (applying || url.isEmpty() || !std::isfinite(next))
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
    if (applying || url.isEmpty())
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
                                            qEnvironmentVariable("OMALUX_STYLES_DIR"),
                                            qEnvironmentVariable("OMALUX_COMPARISON_MAILBOX"))),
      frames(normal), hoverFrames(hover), source(std::move(image)) {
    for (unsigned i = 0; i < OM_CONTROL_COUNT; ++i)
        values[i] = om_controls[i].initial;
    connect(worker.get(), &EngineWorker::initialized, this,
            [this](ControlValues initial, QVariantMap metadata, QVariantList styles,
                   QVariantList cameraDefaults, QString moduleCatalog) {
                modules = std::move(moduleCatalog);
                emit modulesChanged();
                values = initial;
                imageMetadata = metadata;
                imageCameraDefaults = std::move(cameraDefaults);
                styleCatalog = styles;
                catalogReady = true;
                emit controlsChanged();
                emit stylesChanged();
                emit changed();
            });
    connect(worker.get(), &EngineWorker::controlsReady, this, [this](ControlValues next, quint64 revision) {
        if (revision != requestedTicket.revision)
            return;
        values = next;
        emit controlsChanged();
    });
    connect(worker.get(), &EngineWorker::metadataReady, this,
            [this](QString image, QVariantMap metadata, QVariantList cameraDefaults) {
        imageCameraDefaults = std::move(cameraDefaults);
        source = image;
        imageMetadata = metadata;
        emit changed();
    });
    connect(worker.get(), &EngineWorker::modulesReady, this, [this](QString catalog) {
        if (catalog == modules)
            return;
        modules = std::move(catalog);
        emit modulesChanged();
    });
    connect(worker.get(), &EngineWorker::historyReady, this, [this](QVariantList rows) {
        if (historyRows != rows) {
            historyRows = rows;
            emit historyChanged();
        }
    });
    connect(worker.get(), &EngineWorker::stylesReady, this, [this](QVariantList styles) {
        styleCatalog = styles;
        emit stylesChanged();
    });
    connect(worker.get(), &EngineWorker::styleReady, this, [this](QString name) {
        styleName = name;
        emit changed();
    });
    connect(worker.get(), &EngineWorker::frameReady, this, &Editor::showFrame);
    connect(worker.get(), &EngineWorker::hoverReady, this,
            [this](QImage image, quint64 revision, double aspectRatio, QVector4D textureTransform) {
                if (revision != hoverRevision || hoverId.isEmpty())
                    return;
                hoverFrames->set(image);
                hoverAspectRatio = aspectRatio;
                hoverTextureTransform = textureTransform;
                hoverUrl =
                    QString("image://hover/%1/%2x%3").arg(revision).arg(image.width()).arg(image.height());
                emit changed();
            });
    connect(worker.get(), &EngineWorker::failed, this, [this](WorkTicket ticket, QString error) {
        if (ticket.epoch != requestedTicket.epoch)
            return;
        message = error;
        if (applying)
            errorText = error;
        applying = false;
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
    applying = false;
    frames->set(result.image);
    normalAspectRatio = result.aspectRatio;
    normalTextureTransform = result.textureTransform;
    url = QString("image://preview/%1").arg(result.ticket.revision);
    qInfo().noquote() << message;
    emit changed();
}
bool Editor::styleAvailable(const QString &id) const {
    for (const auto &entry : styleCatalog)
        if (entry.toMap()["id"].toString() == id)
            return entry.toMap()["error"].toString().isEmpty();
    return false;
}
void Editor::hoverStyle(const QString &id, bool active) {
    if (active) {
        if (applying || url.isEmpty() || !styleAvailable(id) || hoverId == id)
            return;
    } else if (hoverId.isEmpty() || (!id.isEmpty() && hoverId != id))
        return;
    hoverId = active ? id : QString();
    hoverRevision = worker->hover(hoverId);
    hoverUrl.clear();
    emit changed();
}
void Editor::queueAction(EditorAction action) {
    if (applying || url.isEmpty())
        return;
    hoverStyle("", false);
    applying = true;
    errorText.clear();
    message = "Working…";
    requestedTicket = worker->action(std::move(action));
    emit changed();
}
void Editor::queueControls(int index) {
    hoverStyle("", false);
    requestedTicket = worker->controls(values, index);
    emit controlsChanged();
}
void Editor::setInteractive(bool active) {
    requestedTicket = worker->interactive(active);
}
void Editor::applyStyle(const QString &id) {
    if (applying || url.isEmpty())
        return;
    if (!styleAvailable(id)) {
        errorText = "This style is unavailable";
        emit changed();
        return;
    }
    applyingId = id;
    queueAction({ActionKind::ApplyStyle, id});
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
void Editor::saveStyle(const QString &name) {
    if (name.trimmed().isEmpty())
        return;
    for (const auto &p : styleCatalog)
        if (p.toMap()["name"].toString() == name.trimmed()) {
            errorText = "A style with this name already exists";
            emit changed();
            return;
        }
    queueAction({ActionKind::SaveStyle, name.trimmed()});
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
void Editor::deleteStyle(const QString &id) {
    if (id.startsWith("my-styles/"))
        queueAction({ActionKind::DeleteStyle, id});
}
void Editor::exportStyle(const QString &id, const QUrl &destination) {
    queueAction({ActionKind::ExportStyle, id, destination.toLocalFile()});
}
