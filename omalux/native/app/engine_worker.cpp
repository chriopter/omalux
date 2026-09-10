// SPDX-License-Identifier: GPL-3.0-or-later
#include "engine_worker.h"
#include "frames.h"
#include "image_export.h"
#include <QJsonDocument>
#include <QJsonObject>
#include <QJsonArray>
#include <QElapsedTimer>
#include <QDebug>
#include <memory>
#include <stdexcept>
static QJsonDocument takeJson(char *raw) {
    const auto document = QJsonDocument::fromJson(raw ? raw : "");
    om_engine_free_json(raw);
    return document;
}
static void requireEngine(int result, const char *message) {
    if (result)
        throw std::runtime_error(message);
}
EngineWorker::EngineWorker(QString image, std::vector<QByteArray> args, QString styles, QString mailbox)
    : source(std::move(image)), arguments(std::move(args)), catalog(std::move(styles)),
      bridge(std::move(mailbox)) {
    for (unsigned i = 0; i < OM_CONTROL_COUNT; ++i)
        requested[i] = om_controls[i].initial;
    qRegisterMetaType<ControlValues>();
    qRegisterMetaType<WorkTicket>();
    qRegisterMetaType<RenderResult>();
}
EngineWorker::~EngineWorker() {
    {
        std::lock_guard lock(mutex);
        stopping = true;
    }
    wake.notify_one();
    if (thread.joinable())
        thread.join();
}
void EngineWorker::start() {
    thread = std::thread([this] { run(); });
}
WorkTicket EngineWorker::controls(const ControlValues &values, int index) {
    std::lock_guard lock(mutex);
    requested = values;
    ++ticket.revision;
    pending = true;
    for (unsigned i = 0; i < OM_CONTROL_COUNT; ++i)
        if (index < 0 || int(i) == index)
            revisions[i] = ticket.revision;
    wake.notify_one();
    return ticket;
}
WorkTicket EngineWorker::interactive(bool active) {
    std::lock_guard lock(mutex);
    if (dragging != active) {
        dragging = active;
        if (!active) {
            ++ticket.epoch;
            ++ticket.revision;
            pending = true;
            wake.notify_one();
        }
    }
    return ticket;
}
WorkTicket EngineWorker::parameter(const QString &operation, int instance, const QString &field,
                                   double value) {
    // One field of one module, addressed by name; the worker owns the engine.
    return action({ActionKind::SetParameter,
                   QStringList{operation, QString::number(instance), field, QString::number(value, 'g', 9)}
                       .join('\x1f')});
}
WorkTicket EngineWorker::action(EditorAction action) {
    std::lock_guard lock(mutex);
    pendingAction = std::move(action);
    ++ticket.epoch;
    ++ticket.revision;
    pending = true;
    wake.notify_one();
    return ticket;
}
quint64 EngineWorker::hover(QString id) {
    std::lock_guard lock(mutex);
    hoverId = std::move(id);
    ++hoverRevision;
    hoverPending = !hoverId.isEmpty();
    wake.notify_one();
    return hoverRevision;
}
bool EngineWorker::take(Request &request) {
    std::unique_lock lock(mutex);
    wake.wait(lock, [this] { return stopping || pending || hoverPending; });
    if (stopping)
        return false;
    if (pending) {
        request.values = requested;
        request.revisions = revisions;
        request.ticket = ticket;
        request.draft = dragging;
        request.action = std::move(pendingAction);
        pendingAction = {};
        pending = false;
    } else {
        request.hoverId = hoverId;
        request.hoverRevision = hoverRevision;
        hoverPending = false;
    }
    return true;
}
static QString takeModuleCatalog(OmEngine *engine) {
    char *text = om_engine_modules(engine);
    const QString result = QString::fromUtf8(text ? text : "[]");
    om_engine_free_json(text);
    return result;
}
void EngineWorker::run() {
    std::vector<char *> argv;
    for (auto &arg : arguments)
        argv.push_back(arg.data());
    argv.push_back(nullptr);
    std::unique_ptr<OmEngine, decltype(&om_engine_cleanup)> engine(
        om_engine_create(int(arguments.size()), argv.data()), om_engine_cleanup);
    if (!engine) {
        emit failed({}, "darktable initialization failed");
        return;
    }
    if (om_engine_open(engine.get(), source.toUtf8().constData())) {
        emit failed({}, "Could not open image with darktable");
        return;
    }
    ControlValues initial;
    om_engine_read_controls(engine.get(), initial.data());
    {
        std::lock_guard lock(mutex);
        requested = initial;
    }
    emit initialized(initial, takeJson(om_engine_metadata(engine.get())).object().toVariantMap(),
                     catalog.reload(engine.get()),
                     takeJson(om_engine_camera_defaults(engine.get())).object().toVariantMap()["entries"].toList(),
                     takeModuleCatalog(engine.get()));
    ControlRevisions processed{};
    for (;;) {
        Request request;
        if (!take(request))
            break;
        try {
            if (!request.hoverId.isEmpty())
                renderHover(engine.get(), request);
            else
                process(engine.get(), request, processed);
        } catch (const std::exception &error) {
            if (!request.hoverId.isEmpty())
                qWarning() << "Could not preview style" << request.hoverId << error.what();
            else
                emit failed(request.ticket, QString::fromUtf8(error.what()));
        }
    }
}
void EngineWorker::renderHover(OmEngine *engine, const Request &request) {
    const auto *style = catalog.find(request.hoverId);
    if (!style)
        return;
    unsigned char *pixels = nullptr;
    int width = 0, height = 0;
    OmPreviewGeometry geometry{};
    requireEngine(om_engine_preview_style(engine, style->path.toUtf8().constData(),
                                          style->name.toUtf8().constData(), &pixels, &width, &height,
                                          &geometry),
                  "Could not render style preview");
    const auto image = copyDisplayPixels(pixels, width, height);
    om_engine_free_preview(pixels);
    emit hoverReady(image, request.hoverRevision, geometry.aspect_ratio,
                    QVector4D(geometry.scale_x, geometry.scale_y, geometry.offset_x, geometry.offset_y));
}
void EngineWorker::replaceControls(OmEngine *engine, Request &request, ControlRevisions &processed) {
    om_engine_read_controls(engine, request.values.data());
    request.revisions.fill(0);
    processed.fill(0);
    {
        std::lock_guard lock(mutex);
        requested = request.values;
        revisions.fill(0);
    }
    emit controlsReady(request.values, request.ticket.revision);
}
void EngineWorker::process(OmEngine *engine, Request &request, ControlRevisions &processed) {
    auto &values = request.values;
    const auto &action = request.action;
    const quint64 revision = request.ticket.revision;
    std::array<unsigned char, OM_CONTROL_COUNT> dirty{};
    for (unsigned i = 0; i < OM_CONTROL_COUNT; ++i)
        dirty[i] = request.revisions[i] != processed[i];
    requireEngine(om_engine_update_controls(engine, values.data(), dirty.data()),
                  "Could not update controls");
    om_engine_read_controls(engine, values.data());
    {
        std::lock_guard lock(mutex);
        if (revision == ticket.revision)
            requested = values;
    }
    emit controlsReady(values, revision);
    processed = request.revisions;
    if (action.kind == ActionKind::SetParameter) {
        const auto parts = action.value.split('\x1f');
        const auto operation = parts.value(0).toUtf8(), field = parts.value(2).toUtf8();
        const auto message = QString("Could not set %1.%2").arg(parts.value(0), parts.value(2)).toUtf8();
        requireEngine(om_engine_set_parameter(engine, operation.constData(), parts.value(1).toInt(),
                                              field.constData(), parts.value(3).toDouble()),
                      message.constData());
        replaceControls(engine, request, processed);
    }
    if (action.kind == ActionKind::Halation) {
        requireEngine(om_engine_halation(engine), "Could not configure diffuse or sharpen");
        replaceControls(engine, request, processed);
    }
    QStringList recipes;
    for (unsigned i = 0; i < OM_CONTROL_COUNT; ++i)
        if (dirty[i] && !strcmp(om_controls[i].action, "@recipe") && !recipes.contains(om_controls[i].module))
            recipes.append(om_controls[i].module);
    if (action.kind == ActionKind::Halation)
        recipes.append("diffuse");
    bridge.modules(engine, recipes, revision);
    if (action.kind == ActionKind::ApplyStyle) {
        const auto *style = catalog.find(action.value);
        if (!style)
            throw std::runtime_error("Style is unavailable");
        requireEngine(om_engine_apply_style(engine, style->path.toUtf8().constData(),
                                            style->name.toUtf8().constData(), values.data()),
                      "Could not apply style; check its module compatibility");
        emit styleReady(style->name);
    }
    if (action.kind == ActionKind::History || action.kind == ActionKind::ApplyStyle) {
        if (action.kind == ActionKind::History) {
            requireEngine(om_engine_history_select(engine, action.value.toInt()),
                          "Could not restore history step");
            emit styleReady({});
        }
        replaceControls(engine, request, processed);
        if (!bridge.history(engine, revision))
            throw std::runtime_error(
                "State restored, but comparison snapshot is unsupported or could not be written");
    }
    QString savedDirectory;
    switch (action.kind) {
    case ActionKind::Open:
        requireEngine(om_engine_open(engine, action.value.toUtf8().constData()), "Could not open image");
        source = action.value;
        bridge.reset();
        replaceControls(engine, request, processed);
        emit metadataReady(source, takeJson(om_engine_metadata(engine)).object().toVariantMap(),
                           takeJson(om_engine_camera_defaults(engine)).object().toVariantMap()["entries"].toList());
        emit modulesReady(takeModuleCatalog(engine));
        emit styleReady({});
        break;
    case ActionKind::ExportImage:
        exportImage(engine, action.value, action.quality);
        break;
    case ActionKind::SaveStyle:
        savedDirectory = catalog.save(engine, action.value, source);
        break;
    case ActionKind::DeleteStyle:
        catalog.remove(action.value);
        emit stylesReady(catalog.reload(engine));
        break;
    case ActionKind::ExportStyle:
        catalog.exportBundle(action.value, action.destination);
        break;
    default:
        break;
    }
    emit modulesReady(takeModuleCatalog(engine));
    bridge.publish(source, revision, values, request.revisions);
    emit historyReady(takeJson(om_engine_history(engine)).array().toVariantList());
    QElapsedTimer timer;
    timer.start();
    const unsigned char *pixels = nullptr;
    int width = 0, height = 0;
    OmPreviewGeometry geometry{};
    requireEngine(om_engine_render(engine, &pixels, &width, &height, request.draft, &geometry),
                  "darktable preview failed");
    RenderResult result{request.ticket,
                        copyDisplayPixels(pixels, width, height),
                        {},
                        QString::fromUtf8(om_engine_gpu_warning(engine))};
    result.aspectRatio = geometry.aspect_ratio;
    result.textureTransform =
        QVector4D(geometry.scale_x, geometry.scale_y, geometry.offset_x, geometry.offset_y);
    if (!savedDirectory.isEmpty()) {
        catalog.finishPreview(savedDirectory, result.image);
        emit stylesReady(catalog.reload(engine));
    }
    result.status = QString("%1 · %2 · %3 ms")
                        .arg(request.draft ? "Preview" : "Ready")
                        .arg(result.gpuWarning.isEmpty() ? "OpenCL auto" : "CPU")
                        .arg(timer.elapsed());
    if (action.kind == ActionKind::ExportImage)
        result.status = "Saved " + action.value;
    if (action.kind == ActionKind::SaveStyle)
        result.status = "Saved style: " + action.value;
    if (action.kind == ActionKind::ExportStyle)
        result.status = "Style bundle exported; use the selected folder as darktable LUT root";
    emit frameReady(result);
}
