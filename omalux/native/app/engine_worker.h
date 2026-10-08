// SPDX-License-Identifier: GPL-3.0-or-later
#pragma once
#include "work_types.h"
#include "style_catalog.h"
#include "comparison_bridge.h"
#include "module_catalog.h"
#include <QObject>
#include <QVariantMap>
#include <condition_variable>
#include <mutex>
#include <thread>
#include <vector>
// Queue methods are thread-safe. All engine/catalogue/bridge work is confined to
// the owned thread; only copied results cross back to the Qt thread via signals.
class EngineWorker : public QObject {
    Q_OBJECT
  public:
    EngineWorker(QString source, std::vector<QByteArray> arguments, QString styles, QString mailbox);
    ~EngineWorker() override;
    void start();
    WorkTicket controls(const ControlValues &, int index);
    WorkTicket interactive(bool active);
    WorkTicket action(EditorAction action);
    // Generic module edits queue in order and coalesce like slider values: consecutive
    // parameter edits of the same module merge while the worker is busy.
    WorkTicket moduleEdit(ModuleEdit edit);
    quint64 hover(QString id);
    // A runtime list of one module row (profiles, lenses, files ...), answered by choicesReady
    // without rendering; see om_engine_module_choices.
    void choices(QString operation, int instance, QString list, QString query);
  signals:
    void initialized(ControlValues values, QVariantMap metadata, QVariantList styles,
                     QVariantList cameraDefaults, QString modules);
    void controlsReady(ControlValues values, quint64 revision);
    void metadataReady(QString source, QVariantMap metadata, QVariantList cameraDefaults);
    void modulesReady(QString catalog);
    void moduleReady(QString operation, int instance, QString module);
    // The JSON result of a module tool (picked values, histogram, GUI values), see module_tools.h.
    void moduleToolReady(QString operation, int instance, QString tool, QString result, int error);
    void historyReady(QVariantList rows);
    void stylesReady(QVariantList styles);
    void styleReady(QString name);
    void frameReady(RenderResult result);
    void hoverReady(QImage image, quint64 revision, double aspectRatio, QVector4D textureTransform);
    // The overlay of the module whose on-canvas tool is shown, after each render that changed it.
    void canvasReady(QString operation, int instance, QString overlay);
    void failed(WorkTicket ticket, QString message);
    void choicesReady(QString operation, int instance, QString list, QString query, QString result);

  private:
    struct ChoiceQuery {
        QString operation;
        int instance = 0;
        QString list, query;
    };
    struct Request {
        ControlValues values{};
        ControlRevisions revisions{};
        WorkTicket ticket;
        EditorAction action;
        std::vector<ModuleEdit> edits;
        bool draft = false;
        QString hoverId;
        quint64 hoverRevision = 0;
        std::vector<ChoiceQuery> choices;
    };
    bool take(Request &);
    void run();
    void process(OmEngine *, Request &, ControlRevisions &processed);
    void renderHover(OmEngine *, const Request &);
    void replaceControls(OmEngine *, Request &, ControlRevisions &);
    void refreshModule(OmEngine *, const QString &operation, int instance);
    QString applyModuleEdits(OmEngine *, Request &, ControlRevisions &, QStringList &recipes);
    // On-canvas tools (engine_worker_canvas.cpp); the canvas state belongs to the worker thread.
    static bool mergeCanvasEdit(ModuleEdit &last, const ModuleEdit &edit);
    int applyCanvasEdit(OmEngine *, const ModuleEdit &);
    void publishCanvas(OmEngine *);
    QString canvasOperation, publishedCanvas;
    int canvasInstance = -1;
    QString source;
    std::vector<QByteArray> arguments;
    StyleCatalog catalog;
    ComparisonBridge bridge;
    ModuleCatalog moduleCatalog;
    std::mutex mutex;
    std::condition_variable wake;
    std::thread thread;
    ControlValues requested{};
    ControlRevisions revisions{};
    WorkTicket ticket;
    EditorAction pendingAction;
    std::vector<ModuleEdit> pendingEdits;
    // Edits queued after a pending action wait for it (an export before the file is chosen, an
    // image opened before its first edit): their count before the action and its ticket.
    size_t editsBeforeAction = SIZE_MAX;
    WorkTicket actionTicket;
    std::vector<ChoiceQuery> pendingChoices;
    bool stopping = false, pending = true, dragging = false, hoverPending = false;
    QString hoverId;
    quint64 hoverRevision = 0;
};
