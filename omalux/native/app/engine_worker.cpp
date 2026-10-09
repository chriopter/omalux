// SPDX-License-Identifier: GPL-3.0-or-later
#include "engine_worker.h"
#include <QFileInfo>
#include "frames.h"
#include "image_export.h"
#include "engine/module_instances.h"
#include "engine/module_tools.h"
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
        requested[i] = defaults[i] = om_controls[i].initial;
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
WorkTicket EngineWorker::moduleEdit(ModuleEdit edit) {
    std::lock_guard lock(mutex);
    // Never merge an edit into one queued before a pending action (it must run after it).
    const bool atAction = editsBeforeAction != SIZE_MAX && pendingEdits.size() == editsBeforeAction;
    auto *last = pendingEdits.empty() || atAction ? nullptr : &pendingEdits.back();
    if (last && edit.kind == ActionKind::SetParameters && last->kind == ActionKind::SetParameters &&
        last->operation == edit.operation && last->instance == edit.instance) {
        for (auto it = edit.values.cbegin(); it != edit.values.cend(); ++it)
            last->values.insert(it.key(), it.value());
    } else if (!last || !mergeCanvasEdit(*last, edit))
        pendingEdits.push_back(std::move(edit));
    // A revision, not a new epoch: frames rendered meanwhile stay presentable during a drag.
    ++ticket.revision;
    pending = true;
    wake.notify_one();
    return ticket;
}
WorkTicket EngineWorker::action(EditorAction action) {
    std::lock_guard lock(mutex);
    pendingAction = std::move(action);
    ++ticket.epoch;
    ++ticket.revision;
    editsBeforeAction = pendingEdits.size();
    actionTicket = ticket;
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
void EngineWorker::choices(QString operation, int instance, QString list, QString query) {
    std::lock_guard lock(mutex);
    // A newer query for the same row replaces one that has not started yet.
    for (auto &queued : pendingChoices)
        if (queued.operation == operation && queued.instance == instance && queued.list == list) {
            queued.query = std::move(query);
            return;
        }
    pendingChoices.push_back({std::move(operation), instance, std::move(list), std::move(query)});
    wake.notify_one();
}
bool EngineWorker::take(Request &request) {
    std::unique_lock lock(mutex);
    wake.wait(lock, [this] { return stopping || pending || hoverPending || !pendingChoices.empty(); });
    if (stopping)
        return false;
    // Lists are cheap and do not render; answer them before the next render or hover.
    if (!pendingChoices.empty()) {
        request.choices.swap(pendingChoices);
        pendingChoices.clear();
        return true;
    }
    if (pending) {
        request.values = requested;
        request.revisions = revisions;
        request.ticket = ticket;
        request.draft = dragging;
        request.action = std::move(pendingAction);
        pendingAction = {};
        if (editsBeforeAction != SIZE_MAX && editsBeforeAction < pendingEdits.size()) {
            // Edits queued after the action run in the next request, after it (area E: the
            // worker used to apply them first, e.g. choosing an overlay file before its export).
            request.ticket = actionTicket;
            request.edits.assign(std::make_move_iterator(pendingEdits.begin()),
                                 std::make_move_iterator(pendingEdits.begin() + editsBeforeAction));
            pendingEdits.erase(pendingEdits.begin(), pendingEdits.begin() + editsBeforeAction);
            editsBeforeAction = SIZE_MAX;
            return true;
        }
        editsBeforeAction = SIZE_MAX;
        request.edits.swap(pendingEdits);
        pendingEdits.clear();
        pending = false;
    } else {
        request.hoverId = hoverId;
        request.hoverRevision = hoverRevision;
        hoverPending = false;
    }
    return true;
}
// Opens the file or says why not. 2: the file is there but holds no readable image; the
// photograph shown before is kept (engine.c).
void EngineWorker::openImage(OmEngine *engine, const QString &path) {
    switch (om_engine_open(engine, path.toUtf8().constData())) {
    case 0:
        return;
    case 2:
        throw std::runtime_error(
            ("Could not read " + QFileInfo(path).fileName() + ": damaged or not a photograph").toStdString());
    default:
        throw std::runtime_error(("Could not open " + QFileInfo(path).fileName()).toStdString());
    }
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
    // Without a readable photograph the editor stays open and empty: the worker keeps running
    // and waits for an Open that succeeds; everything else needs an image and is dropped.
    bool opened = false;
    const auto announce = [&] {
        ControlValues initial, resets;
        om_engine_read_controls(engine.get(), initial.data());
        om_engine_default_controls(engine.get(), resets.data());
        {
            std::lock_guard lock(mutex);
            requested = initial;
            defaults = resets;
        }
        emit initialized(
            initial, takeJson(om_engine_metadata(engine.get())).object().toVariantMap(),
            catalog.reload(engine.get()),
            takeJson(om_engine_camera_defaults(engine.get())).object().toVariantMap()["entries"].toList(),
            (moduleCatalog.reload(engine.get()), moduleCatalog.json()));
        moduleCatalog.takeChanged();
        publishedCamera.clear();
        publishCamera(engine.get());
        opened = true;
        return initial;
    };
    try {
        openImage(engine.get(), source);
        announce();
    } catch (const std::exception &error) {
        emit failed({}, QString::fromUtf8(error.what()));
    }
    ControlRevisions processed{};
    for (;;) {
        Request request;
        if (!take(request))
            break;
        try {
            if (!opened) {
                if (request.action.kind != ActionKind::Open || !request.hoverId.isEmpty() ||
                    !request.choices.empty())
                    continue;
                openImage(engine.get(), request.action.value);
                source = request.action.value;
                request.values = announce();
                request.revisions.fill(0);
                emit metadataReady(
                    source, takeJson(om_engine_metadata(engine.get())).object().toVariantMap(),
                    takeJson(om_engine_camera_defaults(engine.get())).object().toVariantMap()["entries"].toList());
                // The image is open; what remains of the request is its first preview.
                request.action.kind = ActionKind::None;
            }
            if (!request.choices.empty()) {
                for (const auto &query : request.choices) {
                    char *raw = om_engine_module_choices(engine.get(), query.operation.toUtf8().constData(),
                                                         query.instance, query.list.toUtf8().constData(),
                                                         query.query.toUtf8().constData());
                    const QString result = QString::fromUtf8(raw ? raw : "");
                    om_engine_free_json(raw);
                    emit choicesReady(query.operation, query.instance, query.list, query.query, result);
                }
            } else if (!request.hoverId.isEmpty())
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
ControlValues EngineWorker::controlDefaults() {
    std::lock_guard lock(mutex);
    return defaults;
}
void EngineWorker::replaceControls(OmEngine *engine, Request &request, ControlRevisions &processed) {
    om_engine_read_controls(engine, request.values.data());
    request.revisions.fill(0);
    processed.fill(0);
    {
        // Slider values queued after this request was taken stay pending; they are newer.
        std::lock_guard lock(mutex);
        for (unsigned i = 0; i < OM_CONTROL_COUNT; ++i)
            if (revisions[i] <= request.ticket.revision) {
                requested[i] = request.values[i];
                revisions[i] = 0;
            }
    }
    emit controlsReady(request.values, request.ticket.revision);
}
// The camera pane's data follows every edit: a preset is "applied" while its module carries it.
void EngineWorker::publishCamera(OmEngine *engine) {
    char *defaults = om_engine_camera_defaults(engine), *presets = om_engine_camera_presets(engine);
    const QByteArray state =
        QByteArray(defaults ? defaults : "") + '\n' + QByteArray(presets ? presets : "");
    const auto entries = takeJson(defaults).object().toVariantMap()["entries"].toList();
    const auto list = takeJson(presets).array().toVariantList();
    if (state == publishedCamera)
        return;
    publishedCamera = state;
    emit cameraReady(entries, list);
}
void EngineWorker::refreshModule(OmEngine *engine, const QString &operation, int instance) {
    const QString entry = moduleCatalog.update(engine, operation, instance);
    if (!entry.isEmpty())
        emit moduleReady(operation, instance, entry);
}
QString EngineWorker::applyModuleEdits(OmEngine *engine, Request &request, ControlRevisions &processed,
                                       QStringList &recipes) {
    if (request.edits.empty())
        return {};
    QString error;
    QList<QPair<QString, int>> edited;
    for (const auto &edit : request.edits) {
        const auto operation = edit.operation.toUtf8();
        int result = 0;
        if (edit.kind == ActionKind::CanvasSelect) {
            applyCanvasEdit(engine, edit);
            continue;
        }
        if (edit.kind == ActionKind::CanvasEdit)
            result = applyCanvasEdit(engine, edit);
        else if (edit.kind == ActionKind::ResetModule)
            result = om_engine_reset_module(engine, operation.constData(), edit.instance);
        else if (edit.kind == ActionKind::CameraPreset) {
            result = om_engine_camera_preset(
                engine, edit.values.value("name").toString().toUtf8().constData(),
                edit.values.value("on").toBool());
            // A film profile has a LUT 3D instance of its own before the base one: the module
            // list may have grown, and an instance's place cannot travel in a style, so split
            // mode gets the whole history as an XMP sidecar, as for moved instances.
            if (!result && edit.values.value("film").toBool()) {
                moduleCatalog.reload(engine);
                if (!bridge.instances(engine, request.ticket.revision))
                    qWarning() << "Could not synchronize the film profile";
                continue;
            }
        }
        else if (edit.kind == ActionKind::ModuleInstance) {
            // New, duplicated, moved, renamed or deleted instances change the module list:
            // the whole catalog is described again.
            int kept = edit.instance;
            result =
                om_engine_module_instance(engine, operation.constData(), edit.instance,
                                          edit.values.value("action").toString().toUtf8().constData(),
                                          edit.values.value("name").toString().toUtf8().constData(), &kept);
            if (!result) {
                moduleCatalog.reload(engine);
                // Deleting or moving an instance cannot travel in a style: the whole history goes
                // to split mode as an XMP sidecar (area E).
                const auto action = edit.values.value("action").toString();
                if (action == "delete" || action == "up" || action == "down") {
                    if (!bridge.instances(engine, request.ticket.revision))
                        qWarning() << "Could not synchronize instances of" << edit.operation;
                } else if (!recipes.contains(edit.operation))
                    recipes.append(edit.operation);
                continue;
            }
        }
        else if (edit.kind == ActionKind::ModuleTool) {
            // Pickers and module buttons: the tool reports which modules it wrote.
            const auto request =
                QJsonDocument(QJsonObject::fromVariantMap(edit.values)).toJson(QJsonDocument::Compact);
            char *raw = nullptr;
            const int status =
                om_engine_module_tool(engine, operation.constData(), edit.instance, request.constData(), &raw);
            const auto output = takeJson(raw).object();
            for (const auto &changed : output["changed"].toArray()) {
                const QString name = changed.toString();
                const int target = name == edit.operation ? edit.instance : 0;
                if (!edited.contains({name, target}))
                    edited.append({name, target});
                if (!recipes.contains(name))
                    recipes.append(name);
            }
            emit moduleToolReady(edit.operation, edit.instance, edit.values["tool"].toString(),
                                 QString::fromUtf8(QJsonDocument(output).toJson(QJsonDocument::Compact)), status);
            if (status && status != 6 && error.isEmpty())
                error = QString("Could not run %1 of %2 (%3)")
                            .arg(edit.values["tool"].toString(), edit.operation)
                            .arg(status);
            continue;
        } else {
            const auto values =
                QJsonDocument(QJsonObject::fromVariantMap(edit.values)).toJson(QJsonDocument::Compact);
            result =
                om_engine_set_parameters(engine, operation.constData(), edit.instance, values.constData());
        }
        if (result) {
            if (error.isEmpty())
                error = QString("Could not %1 %2 (%3)")
                            .arg(edit.kind == ActionKind::ResetModule ? "reset"
                                 : edit.kind == ActionKind::CameraPreset ? "change the camera preset of"
                                 : edit.kind == ActionKind::ModuleInstance
                                     ? edit.values.value("action").toString()
                                     : "edit",
                                 edit.operation)
                            .arg(result);
            continue;
        }
        if (!edited.contains({edit.operation, edit.instance}))
            edited.append({edit.operation, edit.instance});
        if (!recipes.contains(edit.operation))
            recipes.append(edit.operation);
    }
    replaceControls(engine, request, processed);
    for (const auto &[operation, instance] : edited)
        refreshModule(engine, operation, instance);
    if (!error.isEmpty())
        qWarning().noquote() << error;
    return error;
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
    QStringList recipes;
    const QString editError = applyModuleEdits(engine, request, processed, recipes);
    if (action.kind == ActionKind::Halation) {
        requireEngine(om_engine_halation(engine), "Could not configure diffuse or sharpen");
        replaceControls(engine, request, processed);
        refreshModule(engine, "diffuse", 0);
    }
    // Curated sliders edit instance 0 of their module; keep its catalog entry current too.
    QStringList curated;
    for (unsigned i = 0; i < OM_CONTROL_COUNT; ++i)
        if (dirty[i] && !curated.contains(om_controls[i].module))
            curated.append(om_controls[i].module);
    for (const auto &operation : curated)
        refreshModule(engine, operation, 0);
    for (unsigned i = 0; i < OM_CONTROL_COUNT; ++i)
        if (dirty[i] && !strcmp(om_controls[i].action, "@recipe") && !recipes.contains(om_controls[i].module))
            recipes.append(om_controls[i].module);
    if (action.kind == ActionKind::Halation && !recipes.contains("diffuse"))
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
        moduleCatalog.reload(engine);
        if (!bridge.history(engine, revision))
            throw std::runtime_error(
                "State restored, but comparison snapshot is unsupported or could not be written");
    }
    QString savedDirectory;
    switch (action.kind) {
    case ActionKind::Open:
        openImage(engine, action.value);
        source = action.value;
        bridge.reset();
        replaceControls(engine, request, processed);
        {
            ControlValues resets;
            om_engine_default_controls(engine, resets.data());
            std::lock_guard lock(mutex);
            defaults = resets;
        }
        emit metadataReady(
            source, takeJson(om_engine_metadata(engine)).object().toVariantMap(),
            takeJson(om_engine_camera_defaults(engine)).object().toVariantMap()["entries"].toList());
        moduleCatalog.reload(engine);
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
    // The whole catalog is announced once a gesture ends; during it only single modules are.
    if (!request.draft && moduleCatalog.takeChanged())
        emit modulesReady(moduleCatalog.json());
    bridge.publish(source, revision, values, request.revisions);
    if (!request.draft)
        publishCamera(engine);
    emit historyReady(takeJson(om_engine_history(engine)).array().toVariantList());
    QElapsedTimer timer;
    timer.start();
    const unsigned char *pixels = nullptr;
    int width = 0, height = 0;
    OmPreviewGeometry geometry{};
    requireEngine(om_engine_render(engine, &pixels, &width, &height, request.draft, &geometry),
                  "darktable preview failed");
    publishCanvas(engine);
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
    if (!editError.isEmpty())
        result.status = editError;
    if (action.kind == ActionKind::ExportImage)
        result.status = "Saved " + action.value;
    if (action.kind == ActionKind::SaveStyle)
        result.status = "Saved style: " + action.value;
    if (action.kind == ActionKind::ExportStyle)
        result.status = "Style bundle exported; use the selected folder as darktable LUT root";
    emit frameReady(result);
}
