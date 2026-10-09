// SPDX-License-Identifier: GPL-3.0-or-later
#pragma once
#include "engine_worker.h"
#include "frames.h"
#include <QUrl>
#include <memory>
// Qt-thread presentation state only. The worker owns native state and I/O.
class Editor : public QObject {
    Q_OBJECT
    Q_PROPERTY(QString preview READ preview NOTIFY changed)
    Q_PROPERTY(double previewAspectRatio READ previewAspectRatio NOTIFY changed)
    Q_PROPERTY(QVector4D previewTextureTransform READ previewTextureTransform NOTIFY changed)
    Q_PROPERTY(QString status READ status NOTIFY changed)
    Q_PROPERTY(QString gpuWarning READ gpuWarning NOTIFY changed)
    // The photograph given at start (or opened first) could not be read: nothing to edit yet.
    Q_PROPERTY(bool photoMissing READ photoMissing NOTIFY changed)
    // Before/after: the view shows the photograph as it was opened (camera defaults, no edits)
    // instead of the current edit. A view only: nothing in the history changes, and the next
    // frame of an edit, style or history step returns to the edit.
    Q_PROPERTY(bool comparing READ comparing WRITE setComparing NOTIFY changed)
    Q_PROPERTY(QString filename READ filename NOTIFY changed)
    Q_PROPERTY(QVariantList history READ history NOTIFY historyChanged)
    Q_PROPERTY(QVariantMap metadata READ metadata NOTIFY changed)
    Q_PROPERTY(QVariantList cameraDefaults READ cameraDefaults NOTIFY changed)
    // Every Omalux camera preset with its state for the open photograph (engine.h,
    // om_engine_camera_presets).
    Q_PROPERTY(QVariantList cameraPresets READ cameraPresets NOTIFY cameraPresetsChanged)
    Q_PROPERTY(QString moduleCatalog READ moduleCatalog NOTIFY modulesChanged)
    Q_PROPERTY(QVariantList styles READ styles NOTIFY stylesChanged)
    Q_PROPERTY(bool stylesReady READ stylesReady NOTIFY stylesChanged)
    Q_PROPERTY(QString styleError READ styleError NOTIFY changed)
    Q_PROPERTY(bool styleBusy READ styleBusy NOTIFY changed)
    Q_PROPERTY(QString applyingStyle READ applyingStyle NOTIFY changed)
    Q_PROPERTY(QString activeStyle READ activeStyle NOTIFY changed)
    Q_PROPERTY(QVariantList controls READ controls CONSTANT)
    Q_PROPERTY(bool developerMode READ developerMode CONSTANT)
    Q_PROPERTY(QString displayData READ displayData CONSTANT)
    Q_PROPERTY(QString layoutData READ layoutData CONSTANT)
    Q_PROPERTY(QString blendLayoutData READ blendLayoutData CONSTANT)
    Q_PROPERTY(QVariantMap controlValues READ controlValues NOTIFY controlsChanged)
    // The overlay of the on-canvas tool shown on the photo (JSON, engine/canvas.h).
    Q_PROPERTY(QString canvasOverlay READ canvasOverlay NOTIFY canvasChanged)
  public:
    Editor(Frames *, Frames *, Frames *before, QString source, std::vector<QByteArray> arguments);
    bool comparing() const;
    void setComparing(bool on);
    ~Editor() override;
    QString preview() const;
    double previewAspectRatio() const;
    QVector4D previewTextureTransform() const;
    QString status() const;
    QString gpuWarning() const;
    QString filename() const;
    QVariantList history() const;
    QVariantMap metadata() const;
    QVariantList cameraDefaults() const;
    QVariantList cameraPresets() const;
    // Put a camera preset on the photograph by hand (on) or take it off; one history step.
    Q_INVOKABLE void applyCameraPreset(const QString &name, bool on);
    bool styleBusy() const;
    bool photoMissing() const;
    QString applyingStyle() const;
    QString activeStyle() const;
    QVariantList styles() const;
    bool stylesReady() const;
    QString styleError() const;
    QVariantList controls() const;
    bool developerMode() const;
    QString displayData() const;
    QString layoutData() const;
    QString blendLayoutData() const;
    QVariantMap controlValues() const;
    Q_INVOKABLE void hoverStyle(const QString &id, bool active);
    Q_INVOKABLE void applyStyle(const QString &id);
    Q_INVOKABLE void selectHistory(int step);
    Q_INVOKABLE void applyHalation();
    Q_INVOKABLE void openPhoto(const QUrl &url);
    Q_INVOKABLE void saveStyle(const QString &name);
    Q_INVOKABLE void exportPhoto(const QUrl &url, int quality);
    Q_INVOKABLE void deleteStyle(const QString &id);
    Q_INVOKABLE void exportStyle(const QString &id, const QUrl &destination);
    Q_INVOKABLE void setInteractive(bool active);
    Q_INVOKABLE void setControl(const QString &id, double value);
    Q_INVOKABLE void setControls(const QVariantMap &updates);
    Q_INVOKABLE void adjustControl(const QString &id, int steps);
    Q_INVOKABLE void resetControl(const QString &id);
    // What resetControl would set: the image's own default of a registered control.
    Q_INVOKABLE double controlDefault(const QString &id);
    QString moduleCatalog() const;
    // Generic module edits through darktable's introspection. A path is relative to the
    // module's params, e.g. "exposure", "tonecurve[0][1].x" or "@enabled".
    Q_INVOKABLE void setParameter(const QString &operation, int instance, const QString &path, double value);
    Q_INVOKABLE void setParameters(const QString &operation, int instance, const QVariantMap &values);
    Q_INVOKABLE void resetModule(const QString &operation, int instance);
    // A runtime list of one module row (profiles, lenses, LUT files ...): answered by
    // choicesReady with the JSON of om_engine_module_choices. Choosing an item is
    // setParameters with its "set" object, which may contain strings.
    Q_INVOKABLE void requestChoices(const QString &operation, int instance, const QString &list,
                                    const QString &query);
    // darktable's multi-instance menu: "new", "duplicate", "up", "down", "delete" or "rename".
    Q_INVOKABLE void moduleInstance(const QString &operation, int instance, const QString &action,
                                    const QString &name = {});
    // darktable's colour pickers and module buttons (engine/module_tools.h). request carries
    // "tool", the normalised "box" on the displayed image and GUI-only "gui" values.
    Q_INVOKABLE void runModuleTool(const QString &operation, int instance, const QVariantMap &request);
    // The tools the engine implements: JSON array of {operation, tool}.
    Q_INVOKABLE QString moduleTools() const;
    // Drawing on the image (editor_canvas.cpp): which module's tool the photo shows ("" for
    // none), and one gesture of it in preview coordinates.
    QString canvasOverlay() const;
    Q_INVOKABLE void setCanvasModule(const QString &operation, int instance);
    Q_INVOKABLE void editCanvas(const QString &operation, int instance, const QVariantMap &gesture);
  signals:
    void historyChanged();
    void cameraPresetsChanged();
    void changed();
    void controlsChanged();
    void stylesChanged();
    void modulesChanged();
    // One module's catalog entry (a JSON object as in moduleCatalog) after a generic edit.
    void moduleUpdated(QString operation, int instance, QString moduleJson);
    void choicesReady(QString operation, int instance, QString list, QString query, QString result);
    // A module tool finished: its JSON result (see module_tools.h) and the engine status.
    void moduleToolResult(QString operation, int instance, QString tool, QString result, int error);
    void canvasChanged();

  private:
    bool styleAvailable(const QString &id) const;
    void queueAction(EditorAction action);
    void queueControls(int index);
    void queueModuleEdit(ModuleEdit edit);
    void showFrame(RenderResult result);
    std::unique_ptr<EngineWorker> worker;
    Frames *frames, *hoverFrames, *beforeFrames;
    double normalAspectRatio = 1, hoverAspectRatio = 1, beforeAspectRatio = 1;
    QVector4D normalTextureTransform{1, 1, 0, 0}, hoverTextureTransform{1, 1, 0, 0},
        beforeTextureTransform{1, 1, 0, 0};
    // The first frame after a photograph opens is kept as "before".
    QString beforeUrl;
    bool captureBefore = true, showBefore = false;
    quint64 beforeSerial = 0;
    QString source, url, hoverUrl, hoverId, gpuMessage, message = "Loading image…";
    QString errorText, applyingId, styleName;
    ControlValues values{};
    QVariantList styleCatalog, historyRows;
    QVariantMap imageMetadata;
    QVariantList imageCameraDefaults, imageCameraPresets;
    QString modules;
    bool catalogReady = false, applying = false, startFailed = false;
    WorkTicket requestedTicket;
    quint64 presentedRevision = 0, hoverRevision = 0;
    void connectCanvas();
    QString canvasJson, canvasOperation;
    int canvasInstance = -1;
    bool canvasConnected = false;
};
