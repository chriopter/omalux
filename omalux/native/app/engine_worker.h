// SPDX-License-Identifier: GPL-3.0-or-later
#pragma once
#include "work_types.h"
#include "preset_catalog.h"
#include "comparison_bridge.h"
#include <QObject>
#include <QVariantMap>
#include <condition_variable>
#include <mutex>
#include <thread>
// Queue methods are thread-safe. All engine/catalogue/bridge work is confined to
// the owned thread; only copied results cross back to the Qt thread via signals.
class EngineWorker : public QObject {
    Q_OBJECT
  public:
    EngineWorker(QString source, std::vector<QByteArray> arguments, QString presets, QString mailbox);
    ~EngineWorker() override;
    void start();
    WorkTicket controls(const ControlValues &, int index);
    WorkTicket interactive(bool active);
    WorkTicket action(EditorAction action);
    quint64 hover(QString id);
  signals:
    void initialized(ControlValues values, QVariantMap metadata, QVariantList presets);
    void controlsReady(ControlValues values, quint64 revision);
    void metadataReady(QString source, QVariantMap metadata);
    void historyReady(QVariantList rows);
    void presetsReady(QVariantList presets);
    void styleReady(QString name);
    void frameReady(RenderResult result);
    void hoverReady(QImage image, quint64 revision);
    void failed(WorkTicket ticket, QString message);

  private:
    struct Request {
        ControlValues values{};
        ControlRevisions revisions{};
        WorkTicket ticket;
        EditorAction action;
        bool draft = false;
        QString hoverId;
        quint64 hoverRevision = 0;
    };
    bool take(Request &);
    void run();
    void process(OmEngine *, Request &, ControlRevisions &processed);
    void renderHover(OmEngine *, const Request &);
    void replaceControls(OmEngine *, Request &, ControlRevisions &);
    QString source;
    std::vector<QByteArray> arguments;
    PresetCatalog catalog;
    ComparisonBridge bridge;
    std::mutex mutex;
    std::condition_variable wake;
    std::thread thread;
    ControlValues requested{};
    ControlRevisions revisions{};
    WorkTicket ticket;
    EditorAction pendingAction;
    bool stopping = false, pending = true, dragging = false, hoverPending = false;
    QString hoverId;
    quint64 hoverRevision = 0;
};
