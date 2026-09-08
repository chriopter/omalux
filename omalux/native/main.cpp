// SPDX-License-Identifier: GPL-3.0-or-later
#include <QQuickWindow>
#include <QQuickStyle>
#include <QGuiApplication>
#include <QQmlApplicationEngine>
#include <QQmlContext>
#include <QQuickImageProvider>
#include <QImage>
#include <QTimer>
#include <QElapsedTimer>
#include <QFileInfo>
#include <QSaveFile>
#include <QUuid>
#include <QStandardPaths>
#include <QDebug>
#include "controls.h"
#include "presets.h"
#include <QJsonDocument>
#include <QJsonObject>
#include <QJsonArray>
#include <QVariantList>
#include <QVariantMap>
#include <array>
#include <cmath>
#include <condition_variable>
#include <mutex>
#include <memory>
#include <thread>
#include <vector>

extern "C" {
int om_engine_init(int, char **);
const char *om_engine_gpu_warning();
int om_engine_open(const char *);
int om_engine_apply_style(const char *, const char *, float *);
int om_engine_render(const unsigned char **, int *, int *);
int om_engine_update_controls(const float *, const unsigned char *);
void om_engine_read_controls(float *);
char *om_engine_style_details(const char *, const char *);
void om_engine_free_json(char *);
void om_engine_cleanup();
char *om_engine_metadata();
char *om_engine_history();
int om_engine_history_select(int);
char *om_engine_history_snapshot(const char *);
const char *om_engine_version();
char *om_engine_snapshot(const char *, const char *);
int om_engine_halation();
char *om_engine_module_snapshot(const char *, const char *);
int om_engine_export(const char *, const char *, int);
}
class Frames : public QQuickImageProvider {
public:
    Frames() : QQuickImageProvider(QQuickImageProvider::Image) {}
    QImage requestImage(const QString &, QSize *size, const QSize &) override {
        std::lock_guard lock(mutex); if(size) *size = frame.size(); return frame;
    }
    void set(QImage next) { std::lock_guard lock(mutex); frame = std::move(next); }
    QImage image() { std::lock_guard lock(mutex); return frame; }
private:
    std::mutex mutex; QImage frame;
};
class Editor : public QObject {
    Q_OBJECT
    Q_PROPERTY(QString preview READ preview NOTIFY changed)
    Q_PROPERTY(QString status READ status NOTIFY changed)
    Q_PROPERTY(QString gpuWarning READ gpuWarning NOTIFY changed)
    Q_PROPERTY(QString filename READ filename NOTIFY changed)
    Q_PROPERTY(QVariantList history READ history NOTIFY historyChanged)
    Q_PROPERTY(QVariantMap metadata READ metadata NOTIFY changed)
    Q_PROPERTY(QVariantList presets READ presets NOTIFY presetsChanged)
    Q_PROPERTY(bool presetsReady READ presetsReady NOTIFY presetsChanged)
    Q_PROPERTY(QString presetError READ presetError NOTIFY changed)
    Q_PROPERTY(bool styleBusy READ styleBusy NOTIFY changed)
    Q_PROPERTY(QString applyingPreset READ applyingPreset NOTIFY changed)
    Q_PROPERTY(QString activeStyle READ activeStyle NOTIFY changed)
    Q_PROPERTY(QVariantList controls READ controls CONSTANT)
    Q_PROPERTY(QVariantMap controlValues READ controlValues NOTIFY controlsChanged)
public:
    Editor(Frames *frames, QString source, std::vector<QByteArray> arguments)
        : frames(frames), source(source), arguments(std::move(arguments)) {
        for(unsigned int i=0; i<OM_CONTROL_COUNT; ++i) values[i]=requested[i]=om_controls[i].initial;
        presetFiles=discoverPresets(qEnvironmentVariable("OMALUX_PRESETS_DIR"));
        worker = std::thread([this] { run(); });
    }
    ~Editor() override {
        {std::lock_guard lock(mutex); stopping = true;}
        wake.notify_one(); worker.join();
    }
    QString preview() const { return url; }
    QString status() const { return message; }
    QString gpuWarning() const { return gpuMessage; }
    QString filename() const { return QFileInfo(source).fileName(); }
    QVariantList history() const { return historyRows; }
    QVariantMap metadata() const { return imageMetadata; }
    bool styleBusy() const { return applyingStyle; }
    QString applyingPreset() const { return applyingId; }
    QString activeStyle() const { return styleName; }
    QVariantList presets() const { return presetCatalog; }
    bool presetsReady() const { return catalogReady; }
    QString presetError() const { return styleError; }
    Q_INVOKABLE void applyPreset(const QString &id) {
        if(applyingStyle || url.isEmpty()) return;
        bool available=false;
        for(const auto &entry:presetCatalog)
            if(entry.toMap()["id"].toString()==id) available=entry.toMap()["error"].toString().isEmpty();
        if(!available) {styleError="This preset is unavailable"; emit changed(); return;}
        applyingStyle=true; applyingId=id; styleError.clear();
        {std::lock_guard lock(mutex); pendingStyle=id; ++generation; pending=true;}
        wake.notify_one(); emit changed();
    }
    Q_INVOKABLE void selectHistory(int step) {
        for(const auto &row:historyRows) if(row.toMap()["step"].toInt()==step) {
            if(!row.toMap()["current"].toBool()) queueAction("history",QString::number(step));
            return;
        }
    }
    Q_INVOKABLE void applyHalation() { queueAction("halation",""); }
    Q_INVOKABLE void openPhoto(const QUrl &url) {
        const QString path=url.toLocalFile();
        if(!QFileInfo(path).isFile()) {message="Image does not exist"; emit changed(); return;}
        queueAction("open",path);
    }
    Q_INVOKABLE void savePreset(const QString &name) {
        if(name.trimmed().isEmpty()) return;
        for(const auto &p:presetCatalog) if(p.toMap()["name"].toString()==name.trimmed()) {
            styleError="A preset with this name already exists";emit changed();return;
        }
        queueAction("preset",name.trimmed());
    }
    Q_INVOKABLE void exportPhoto(const QUrl &url, int quality) {
        QString path=url.toLocalFile();
        if(path==source || QFileInfo(path).canonicalFilePath()==QFileInfo(source).canonicalFilePath()) {
            message="Choose a different output file to preserve the original";emit changed();return;
        }
        if(!path.endsWith(".jpg",Qt::CaseInsensitive) && !path.endsWith(".jpeg",Qt::CaseInsensitive) && !path.endsWith(".png",Qt::CaseInsensitive)) {
            message="Choose a .jpg or .png output filename";emit changed();return;
        }
        exportQuality=qBound(1,quality,100); queueAction("export",path);
    }
    Q_INVOKABLE void deletePreset(const QString &id) {
        if(!id.startsWith("my-presets/") || applyingStyle) return;
        const QDir root(qEnvironmentVariable("OMALUX_PRESETS_DIR"));
        const QString directory=QFileInfo(root.filePath(id)).canonicalPath();
        if(!directory.startsWith(root.canonicalPath()+"/my-presets/")) return;
        if(!QDir(directory).removeRecursively()) {message="Could not delete preset";emit changed();return;}
        queueAction("reload","");
    }
    Q_INVOKABLE void exportPreset(const QString &id, const QUrl &destination) {
        if(applyingStyle) return;
        const QDir root(qEnvironmentVariable("OMALUX_PRESETS_DIR"));
        const QString sourceDir=QFileInfo(root.filePath(id)).canonicalPath();
        if(!sourceDir.startsWith(root.canonicalPath()+"/")) return;
        // Package the catalogue-relative subtree so LUT references stay valid.
        const QString target=QDir(destination.toLocalFile()).filePath(QFileInfo(id).path());
        if(QFileInfo::exists(target)) {message="Export destination already exists";emit changed();return;}
        const auto absoluteTarget=QDir::cleanPath(QFileInfo(target).absoluteFilePath());
        if(absoluteTarget==sourceDir || absoluteTarget.startsWith(sourceDir+"/")) {message="Choose a destination outside the source bundle";emit changed();return;}
        if(!copyDirectory(sourceDir,target)) {QDir(target).removeRecursively();message="Could not export preset bundle";emit changed();return;}
        message="Preset bundle exported; use the selected folder as darktable LUT root";emit changed();
    }
    QVariantList controls() const {
        QVariantList result;
        for(unsigned int i=0; i<OM_CONTROL_COUNT; ++i) {
            const auto &c=om_controls[i];
            result.append(QVariantMap{{"id", c.id}, {"module", c.module}, {"label", c.label}, {"minimum", c.minimum},
                {"maximum", c.maximum}, {"softMinimum", c.soft_minimum!=c.soft_maximum ? c.soft_minimum : c.minimum}, {"softMaximum", c.soft_minimum!=c.soft_maximum ? c.soft_maximum : c.maximum}, {"step", c.step}, {"unit", c.unit}, {"decimals", c.decimals}, {"group", c.group}, {"section", c.section}, {"colors", c.colors}, {"detail", bool(c.detail)}, {"initial", c.initial}});
        }
        return result;
    }
    QVariantMap controlValues() const {
        QVariantMap result;
        for(unsigned int i=0; i<OM_CONTROL_COUNT; ++i) result[om_controls[i].id]=values[i];
        return result;
    }
    Q_INVOKABLE void setControl(const QString &id, double next) {
        if(applyingStyle || url.isEmpty() || !std::isfinite(next)) return;
        for(unsigned int i=0; i<OM_CONTROL_COUNT; ++i) {
            const auto &c=om_controls[i];
            if(id != QLatin1String(c.id)) continue;
            next=qBound(double(c.minimum), next, double(c.maximum));
            if(values[i] == next) return;
            values[i]=next;
            queueControls(int(i));
            return;
        }
    }
    Q_INVOKABLE void setControls(const QVariantMap &updates) {
        if(applyingStyle || url.isEmpty()) return;
        for(auto it=updates.begin();it!=updates.end();++it) setControl(it.key(),it.value().toDouble());
    }
    Q_INVOKABLE void adjustControl(const QString &id, int steps) {
        for(unsigned int i=0; i<OM_CONTROL_COUNT; ++i)
            if(id == QLatin1String(om_controls[i].id)) setControl(id, values[i]+steps*om_controls[i].step);
    }
    Q_INVOKABLE void resetControl(const QString &id) {
        for(const auto &c:om_controls) if(id==c.id) setControl(id,c.initial);
    }

signals:
    void historyChanged();
    void changed();
    void controlsChanged();
    void presetsChanged();
private:
    static bool copyDirectory(const QString &source, const QString &destination) {
        if(!QDir().mkpath(destination)) return false;
        for(const auto &file:QDir(source).entryInfoList(QDir::Files|QDir::Dirs|QDir::NoDotAndDotDot)) {
            if(file.isSymLink()) return false;
            const auto target=QDir(destination).filePath(file.fileName());
            if(file.isDir() ? !copyDirectory(file.absoluteFilePath(),target) : !QFile::copy(file.absoluteFilePath(),target)) return false;
        }
        return true;
    }
    void queueAction(const QString &kind,const QString &value) {
        if(applyingStyle || url.isEmpty()) return;
        applyingStyle=true;styleError.clear();message="Working…";
        {std::lock_guard lock(mutex);actionKind=kind;actionValue=value;++generation;pending=true;}
        wake.notify_one();emit changed();
    }
    void queueControls(int index=-1) {
        {std::lock_guard lock(mutex); requested=values; ++generation; pending=true;
         for(unsigned int i=0;i<OM_CONTROL_COUNT;++i) if(index<0 || int(i)==index) requestedRevisions[i]=generation;}
        wake.notify_one(); emit controlsChanged();
    }
    void fail(const QString &text) {
        QMetaObject::invokeMethod(this,[this,text]{message = text; if(applyingStyle) styleError=text; applyingStyle=false; emit changed();},Qt::QueuedConnection);
    }
    QByteArray comparisonControls(unsigned long epoch, const std::array<float,OM_CONTROL_COUNT> &snapshot,
                                  const std::array<unsigned long,OM_CONTROL_COUNT> &revisions) {
        QByteArray command;
        for(unsigned int i=0;i<OM_CONTROL_COUNT;++i) {
            if(!strcmp(om_controls[i].action,"@recipe")) continue;
            command += "control "+QByteArray::number(epoch)+" "+om_controls[i].module+" "+QUrl::toPercentEncoding(om_controls[i].action)+" "
                +QByteArray::number(om_parameter_value(i,snapshot[i]),'g',9)+" "+QByteArray::number(revisions[i])+"\n";
        }
        return command;
    }
    QVariantList loadPresetCatalog() {
        QVariantList catalog;
        for(const auto &preset:presetFiles) {
            QVariantMap entry{{"id",preset.id},{"name",preset.name},{"description",preset.description},{"previewUrl",preset.previewUrl},{"error",preset.error},{"modules",QVariantList{}}};
            if(preset.error.isEmpty()) {
                char *raw=om_engine_style_details(preset.path.toUtf8().constData(),preset.name.toUtf8().constData());
                auto details=QJsonDocument::fromJson(raw).object(); om_engine_free_json(raw);
                entry["error"]=details.isEmpty() ? "Could not read style settings" : details["error"].toString();
                QVariantList modules;
                for(const auto &value:details["modules"].toArray()) {
                    auto module=value.toObject().toVariantMap(); QVariantList settings;
                    for(const auto &row:module["settings"].toList()) {
                        auto setting=row.toMap(); const auto v=setting["value"];
                        QString display;
                        if(v.metaType().id()==QMetaType::Bool) display=v.toBool() ? "on" : "off";
                        else if(v.metaType().id()==QMetaType::Double || v.metaType().id()==QMetaType::LongLong) {
                            display=QString::number(v.toDouble(),'g',6);
                            for(const auto &c:om_controls)
                                if(module["operation"].toString()==c.module && setting["parameter"].toString()==c.parameter)
                                    display=QString::number((v.toDouble()-c.offset)/c.scale,'f',c.decimals)+c.unit;
                            if(module["operation"].toString()=="exposure" && (setting["parameter"].toString()=="exposure" || setting["parameter"].toString()=="deflicker_target_level"))
                                display=QString::number(v.toDouble(),'f',setting["parameter"].toString()=="exposure" ? 3 : 2)+" EV";
                            if(module["operation"].toString()=="exposure" && setting["parameter"].toString()=="deflicker_percentile")
                                display=QString::number(v.toDouble(),'f',2)+"%";
                            if(module["operation"].toString()=="exposure" && setting["parameter"].toString()=="black")
                                display=QString::number(v.toDouble(),'f',4);
                        } else if(v.metaType().id()==QMetaType::QVariantList || v.metaType().id()==QMetaType::QVariantMap)
                            display=QString::fromUtf8(QJsonDocument::fromVariant(v).toJson(QJsonDocument::Compact));
                        else display=v.toString();
                        setting["display"]=display; settings.append(setting);
                    }
                    module["settings"]=settings; modules.append(module);
                }
                entry["modules"]=modules;
            }
            catalog.append(entry);
        }
        return catalog;
    }
    void run() {
        std::vector<char *> argv;
        for(auto &arg:arguments) argv.push_back(arg.data());
        argv.push_back(nullptr);
        if(om_engine_init(int(arguments.size()),argv.data())) {fail("darktable initialization failed");return;}
        const QString warning = QString::fromUtf8(om_engine_gpu_warning());
        QMetaObject::invokeMethod(this,[this,warning]{gpuMessage=warning; emit changed();},Qt::QueuedConnection);
        if(om_engine_open(source.toUtf8().constData())) {
            fail("Could not open image with darktable"); om_engine_cleanup(); return;
        }
        om_engine_read_controls(requested.data());
        const auto initial=requested;
        comparisonSource=source;
        char *metadataRaw=om_engine_metadata();const auto info=QJsonDocument::fromJson(metadataRaw).object().toVariantMap();om_engine_free_json(metadataRaw);
        QMetaObject::invokeMethod(this,[this,info]{imageMetadata=info;emit changed();},Qt::QueuedConnection);
        const auto catalog=loadPresetCatalog();
        QMetaObject::invokeMethod(this,[this,initial,catalog]{values=initial; presetCatalog=catalog; catalogReady=true; emit controlsChanged(); emit presetsChanged();},Qt::QueuedConnection);
        std::array<unsigned long,OM_CONTROL_COUNT> processedRevisions{};
        while(true) {
            std::array<float, OM_CONTROL_COUNT> next;
            std::array<unsigned long,OM_CONTROL_COUNT> nextRevisions;
            unsigned long revision; QString styleId, kind, action, savedDirectory; int quality;
            {std::unique_lock lock(mutex); wake.wait(lock,[this]{return stopping || pending;});
             if(stopping) break; next=requested; nextRevisions=requestedRevisions; revision=generation; pending=false; styleId=pendingStyle; pendingStyle.clear(); kind=actionKind; action=actionValue; quality=exportQuality; actionKind.clear(); actionValue.clear();}
            // Flush pending edits before a style so unrelated modules keep them.
            std::array<unsigned char,OM_CONTROL_COUNT> dirty{};
            for(unsigned int i=0;i<OM_CONTROL_COUNT;++i) dirty[i]=nextRevisions[i]!=processedRevisions[i];
            if(om_engine_update_controls(next.data(),dirty.data())) {fail("Could not update controls");continue;}
            om_engine_read_controls(next.data());
            {std::lock_guard lock(mutex); if(revision==generation) requested=next;}
            QMetaObject::invokeMethod(this,[this,next,revision]{
                {std::lock_guard lock(mutex); if(revision!=generation) return;}
                values=next; emit controlsChanged();
            },Qt::QueuedConnection);
            processedRevisions=nextRevisions;
            if(kind=="halation") {
                if(om_engine_halation()) {fail("Could not configure diffuse or sharpen");continue;}
                om_engine_read_controls(next.data());
                {std::lock_guard lock(mutex);requested=next;}
                QMetaObject::invokeMethod(this,[this,next]{values=next;emit controlsChanged();},Qt::QueuedConnection);
            }
            // Curves and compound recipes have no scalar GTK action. Synchronize
            // only the affected module as a private style, before later style events.
            QStringList recipeModules;
            for(unsigned int i=0;i<OM_CONTROL_COUNT;++i)
                if(dirty[i] && !strcmp(om_controls[i].action,"@recipe") && !recipeModules.contains(om_controls[i].module)) recipeModules.append(om_controls[i].module);
            if(kind=="halation") recipeModules.append("diffuse");
            const QString bridge=qEnvironmentVariable("OMALUX_COMPARISON_MAILBOX");
            if(!bridge.isEmpty()) for(const auto &module:recipeModules) {
                const QString name="omalux-sync-"+module+"-"+QString::number(revision);
                char *raw=om_engine_module_snapshot(name.toUtf8().constData(),module.toUtf8().constData());
                if(!raw) {qWarning()<<"Could not synchronize module"<<module;continue;}
                const auto snapshot=QJsonDocument::fromJson(raw).object();om_engine_free_json(raw);
                const QString path=QDir(QFileInfo(bridge).absolutePath()).filePath(name+".dtstyle");
                QSaveFile file(path);const auto xml=snapshot["xml"].toString().toUtf8();
                if(file.open(QIODevice::WriteOnly) && file.write(xml)==xml.size() && file.commit())
                    styleJournal+="module "+QByteArray::number(styleRevision)+" "+QByteArray::number(revision)+" "+module.toUtf8()+" "+name.toUtf8()+"\n";
            }
            if(!styleId.isEmpty()) {
                const QByteArray precedingControls=comparisonControls(styleRevision,next,nextRevisions);
                const PresetFile *preset=nullptr;
                for(const auto &file:presetFiles) if(file.id==styleId) preset=&file;
                if(!preset || om_engine_apply_style(preset->path.toUtf8().constData(),preset->name.toUtf8().constData(),next.data())) {
                    fail("Could not apply preset; check its module compatibility"); continue;
                }
                ++styleRevision; appliedFilename=preset->id; appliedName=preset->name;
                styleJournal += precedingControls+"style "+QByteArray::number(styleRevision)+" "
                    +QUrl::toPercentEncoding(appliedFilename)+" "+QUrl::toPercentEncoding(appliedName)+"\n";
                processedRevisions.fill(0); nextRevisions.fill(0);
                {std::lock_guard lock(mutex); requested=next; requestedRevisions.fill(0);}
                const QString name=preset->name;
                QMetaObject::invokeMethod(this,[this,next,name]{values=next; styleName=name; emit controlsChanged(); emit changed();},Qt::QueuedConnection);
            }
            if(kind=="history") {
                if(om_engine_history_select(action.toInt())) {fail("Could not restore history step");continue;}
                om_engine_read_controls(next.data());
                nextRevisions.fill(0);processedRevisions.fill(0);
                {std::lock_guard lock(mutex);requested=next;requestedRevisions.fill(0);}
                QMetaObject::invokeMethod(this,[this,next]{values=next;styleName.clear();emit controlsChanged();emit changed();},Qt::QueuedConnection);
                if(!bridge.isEmpty()) {
                    const auto name="omalux-history-"+QString::number(revision);
                    const auto path=QDir(QFileInfo(bridge).absolutePath()).filePath(name+".dtstyle");
                    char *raw=om_engine_history_snapshot(name.toUtf8().constData());
                    const auto snapshot=raw ? QJsonDocument::fromJson(raw).object() : QJsonObject();
                    if(raw) om_engine_free_json(raw);
                    QSaveFile file(path);const auto xml=snapshot["xml"].toString().toUtf8();
                    if(xml.isEmpty() || !file.open(QIODevice::WriteOnly) || file.write(xml)!=xml.size() || !file.commit()) {
                        fail("History restored, but comparison snapshot is unsupported or could not be written");continue;
                    }
                    ++styleRevision;
                    styleJournal="history "+QByteArray::number(styleRevision)+" "+name.toUtf8()+"\n";
                }
            }
            if(kind=="open") {
                if(om_engine_open(action.toUtf8().constData())) {fail("Could not open image");continue;}
                om_engine_read_controls(next.data()); nextRevisions.fill(0);processedRevisions.fill(0);
                styleRevision=0;styleJournal.clear();
                comparisonSource=action;
                {std::lock_guard lock(mutex);requested=next;requestedRevisions.fill(0);}
                char *raw=om_engine_metadata();const auto info=QJsonDocument::fromJson(raw).object().toVariantMap();om_engine_free_json(raw);
                QMetaObject::invokeMethod(this,[this,action,next,info]{source=action;values=next;imageMetadata=info;styleName.clear();emit controlsChanged();emit changed();},Qt::QueuedConnection);
            }
            if(kind=="export") {
                const QString extension=QFileInfo(action).suffix().toLower()=="png" ? "png" : "jpeg";
                const QString temporary=QDir(QFileInfo(action).absolutePath()).filePath(".omalux-export-"+QUuid::createUuid().toString(QUuid::WithoutBraces)+"."+extension);
                if(om_engine_export(temporary.toUtf8().constData(),extension.toUtf8().constData(),quality)) {QFile::remove(temporary);fail("Image export failed");continue;}
                QFile input(temporary);QSaveFile output(action);
                bool okay=input.open(QIODevice::ReadOnly) && output.open(QIODevice::WriteOnly);
                while(okay && !input.atEnd()) {const auto bytes=input.read(1024*1024);okay=!bytes.isEmpty() && output.write(bytes)==bytes.size();}
                okay=okay && output.commit(); input.close();QFile::remove(temporary);
                if(!okay) {fail("Could not publish exported image");continue;}
                QMetaObject::invokeMethod(this,[this,action]{message="Saved "+action;emit changed();},Qt::QueuedConnection);
            }
            if(kind=="preset") {
                const QString relative="my-presets/"+QUuid::createUuid().toString(QUuid::WithoutBraces);
                QDir root(qEnvironmentVariable("OMALUX_PRESETS_DIR"));
                const QString directory=root.filePath(relative);
                char *raw=om_engine_snapshot(action.toUtf8().constData(),relative.toUtf8().constData());
                if(!raw) {fail("This recipe contains masks, instances or external assets that cannot yet be saved portably");continue;}
                const auto snapshot=QJsonDocument::fromJson(raw).object();om_engine_free_json(raw);
                bool okay=QDir().mkpath(directory);QJsonArray assets;
                for(const auto &value:snapshot["assets"].toArray()) {
                    auto asset=value.toObject();const QString origin=QFileInfo(root.filePath(asset["source"].toString())).canonicalFilePath();
                    const QString target=QDir(directory).filePath(asset["path"].toString());
                    okay=okay && origin.startsWith(root.canonicalPath()+"/") && QDir().mkpath(QFileInfo(target).absolutePath()) && QFile::copy(origin,target);
                    asset.remove("source");assets.append(asset);
                }
                QSaveFile style(QDir(directory).filePath("preset.dtstyle"));const auto xml=snapshot["xml"].toString().toUtf8();
                okay=okay && style.open(QIODevice::WriteOnly) && style.write(xml)==xml.size() && style.commit();
                QJsonObject manifest{{"version",1}};if(!assets.isEmpty()) manifest["assets"]=assets;
                const QDir repository(QDir::cleanPath(QFileInfo(QStringLiteral(OMALUX_QML)).absolutePath()+"/../.."));
                const auto relativeSource=repository.relativeFilePath(comparisonSource);
                manifest["preview"]=QJsonObject{{"source",relativeSource.startsWith("../") ? comparisonSource : relativeSource},{"darktable_version",om_engine_version()}};
                QSaveFile file(QDir(directory).filePath("preset.json"));const auto bytes=QJsonDocument(manifest).toJson();
                okay=okay && file.open(QIODevice::WriteOnly) && file.write(bytes)==bytes.size() && file.commit();
                if(!okay) {QDir(directory).removeRecursively();fail("Could not save preset bundle");continue;}
                savedDirectory=directory;
            }
            if(kind=="preset" || kind=="reload") {
                presetFiles=discoverPresets(qEnvironmentVariable("OMALUX_PRESETS_DIR"));const auto fresh=loadPresetCatalog();
                QMetaObject::invokeMethod(this,[this,fresh]{presetCatalog=fresh;emit presetsChanged();},Qt::QueuedConnection);
            }
            // Private latest-value mailbox for the optional comparison process.
            // Atomic replacement; never wait for darktable to consume or render it.
            const QString mailbox=qEnvironmentVariable("OMALUX_COMPARISON_MAILBOX");
            if(!mailbox.isEmpty()) {
                QSaveFile file(mailbox);
                const QByteArray command="omalux-controls-v3 " + QByteArray::number(revision)+"\nsource "+QUrl::toPercentEncoding(comparisonSource)+"\n"+styleJournal+comparisonControls(styleRevision,next,nextRevisions);
                if(!file.open(QIODevice::WriteOnly) || file.write(command)!=command.size() || !file.commit())
                    qWarning() << "Could not synchronize comparison controls:" << file.errorString();
            }
            char *historyRaw=om_engine_history();
            const auto history=QJsonDocument::fromJson(historyRaw).array().toVariantList();
            om_engine_free_json(historyRaw);
            QMetaObject::invokeMethod(this,[this,history] {
                if(historyRows != history) { historyRows=history; emit historyChanged(); }
            },Qt::QueuedConnection);
            QElapsedTimer timer; timer.start();
            const unsigned char *pixels = nullptr; int width=0,height=0;
            if(om_engine_render(&pixels,&width,&height)) {fail("darktable preview failed");continue;}
            QImage result(pixels,width,height,width*4,QImage::Format_RGB32);
            auto copy=result.copy();
            // darktable's display buffer is BGRx; gamma leaves x undefined.
            // Qt RGB32 requires 0xff alpha, including during texture upload.
            for(int y=0; y<copy.height(); ++y) {
                auto *row=reinterpret_cast<QRgb *>(copy.scanLine(y));
                for(int x=0; x<copy.width(); ++x) row[x] |= 0xff000000u;
            }
            if(!savedDirectory.isEmpty()) {
                copy.scaled(384,256,Qt::KeepAspectRatio,Qt::SmoothTransformation).save(QDir(savedDirectory).filePath("thumbnail.jpg"),"JPG",90);
                presetFiles=discoverPresets(qEnvironmentVariable("OMALUX_PRESETS_DIR"));
                const auto fresh=loadPresetCatalog();
                QMetaObject::invokeMethod(this,[this,fresh]{presetCatalog=fresh;emit presetsChanged();},Qt::QueuedConnection);
            }
            {std::lock_guard lock(mutex); if(revision != generation) continue;}
            const qint64 elapsed=timer.elapsed();
            const QString warning=QString::fromUtf8(om_engine_gpu_warning());
            QMetaObject::invokeMethod(this,[this,copy,elapsed,revision,warning,kind,action] {
                gpuMessage=warning;
                {std::lock_guard lock(mutex); if(revision != generation) return;}
                applyingStyle=false;
                frames->set(copy); url=QString("image://preview/%1").arg(revision);
                message=QString("Ready · %1 · %2 ms").arg(warning.isEmpty() ? "OpenCL auto" : "CPU").arg(elapsed);
                if(kind=="export") message="Saved "+action;
                else if(kind=="preset") message="Saved preset: "+action;
                qInfo().noquote() << message; emit changed();
            },Qt::QueuedConnection);
        }
        om_engine_cleanup();
    }
    std::vector<PresetFile> presetFiles; QVariantList presetCatalog; bool catalogReady=false;
    QVariantList historyRows;
    QVariantMap imageMetadata;QString actionKind,actionValue,comparisonSource;int exportQuality=90;
    QString styleError, pendingStyle, appliedFilename, appliedName, applyingId;
    QByteArray styleJournal;
    std::array<unsigned long,OM_CONTROL_COUNT> requestedRevisions{};
    bool applyingStyle=false; unsigned long styleRevision=0; QString styleName;
    Frames *frames; QString source,url,gpuMessage,message="Loading image…";
    std::vector<QByteArray> arguments;
    std::array<float, OM_CONTROL_COUNT> values{}, requested{};
    std::mutex mutex; std::condition_variable wake; std::thread worker;
    bool stopping=false,pending=true; unsigned long generation=1;
};
#include "smoke.h"
int main(int argc,char **argv) {
    QQuickStyle::setStyle("Basic");
    QGuiApplication app(argc,argv);
    const auto args=app.arguments();
    if(args.size()<4) {qCritical()<<"Use bin/dev [image]";return 1;}
    auto *frames=new Frames;
    std::vector<QByteArray> dtargs;
    for(int i=3;i<args.size();++i) dtargs.push_back(args[i].toUtf8());
    // Destroy Editor before the QML engine releases its image provider.
    QQmlApplicationEngine engine;
    Editor editor(frames,args[1],std::move(dtargs));
    engine.addImageProvider("preview",frames);
    engine.rootContext()->setContextProperty("editor",&editor);
    engine.rootContext()->setContextProperty("assetsRoot",QUrl::fromLocalFile(args[2]+"/"));
    engine.load(QUrl::fromLocalFile(QStringLiteral(OMALUX_QML)));
    if(engine.rootObjects().isEmpty()) return 1;
    install_smoke(app,editor,frames,engine);
    // Batch thumbnails advance only after a completed engine render, without per-style startup.
    if(qEnvironmentVariableIsSet("OMALUX_PREVIEW_DIR")) {
        auto ids=std::make_shared<QStringList>();
        for(const auto &id:QJsonDocument::fromJson(qgetenv("OMALUX_PREVIEW_IDS")).array()) ids->append(id.toString());
        auto index=std::make_shared<int>(0);
        auto waiting=std::make_shared<bool>(false);
        auto advance=[&,ids,index,waiting] {
            if(!editor.presetsReady() || editor.preview().isEmpty() || editor.styleBusy()) return;
            if(!editor.presetError().isEmpty() || !editor.status().startsWith("Ready")) {
                qCritical() << "Batch render failed:" << editor.presetError() << editor.status(); app.exit(2); return;
            }
            if(*waiting) {
                const QString path=QDir(qEnvironmentVariable("OMALUX_PREVIEW_DIR")).filePath(ids->at(*index)+".png");
                if(!QDir().mkpath(QFileInfo(path).absolutePath()) || !frames->image().save(path)) {
                    qCritical() << "Could not save preview:" << path; app.exit(2); return;
                }
                qInfo().noquote() << "Rendered" << ids->at(*index);
                ++*index; *waiting=false;
            }
            if(*index==ids->size()) {app.quit(); return;}
            *waiting=true; editor.applyPreset(ids->at(*index));
        };
        QObject::connect(&editor,&Editor::changed,&app,[&app,advance]{QTimer::singleShot(0,&app,advance);});
        QTimer::singleShot(0,&app,advance);
        QTimer::singleShot(300000,&app,[&]{qCritical() << "Batch preview timed out"; app.exit(2);});
    }
    // Optional offscreen development capture, no effect during normal launches.
    if(qEnvironmentVariableIsSet("OMALUX_CAPTURE")) {
        QTimer::singleShot(qEnvironmentVariableIntValue("OMALUX_CAPTURE_DELAY") > 0 ? qEnvironmentVariableIntValue("OMALUX_CAPTURE_DELAY") / 2 : 2000,&app,[&]{frames->image().save(qEnvironmentVariable("OMALUX_CAPTURE")+".neutral.png");
            if(qEnvironmentVariableIsSet("OMALUX_CAPTURE_STYLE")) engine.rootObjects().first()->setProperty("selectedPanel",1);
            if(qEnvironmentVariableIsSet("OMALUX_CAPTURE_EXPAND"))
                QMetaObject::invokeMethod(engine.rootObjects().first(),"showPresetDetails",Q_ARG(QVariant,QVariant(qEnvironmentVariable("OMALUX_CAPTURE_EXPAND"))));
            if(qEnvironmentVariableIsSet("OMALUX_CAPTURE_STYLE")) editor.applyPreset(qEnvironmentVariable("OMALUX_CAPTURE_STYLE")=="1" ? "chromatic/preset.dtstyle" : qEnvironmentVariable("OMALUX_CAPTURE_STYLE")); else editor.setControl(qEnvironmentVariable("OMALUX_CAPTURE_CONTROL", "brightness"), qEnvironmentVariableIsSet("OMALUX_CAPTURE_VALUE") ? qEnvironmentVariable("OMALUX_CAPTURE_VALUE").toDouble() : 0.30);});
        QTimer::singleShot(qEnvironmentVariableIntValue("OMALUX_CAPTURE_DELAY") > 0 ? qEnvironmentVariableIntValue("OMALUX_CAPTURE_DELAY") : 4500,&app,[&]{
            if(editor.preview().isEmpty() || editor.styleBusy() || !editor.presetError().isEmpty()
               || (qEnvironmentVariableIsSet("OMALUX_CAPTURE_STYLE") && editor.activeStyle().isEmpty())) {
                qCritical() << "Capture did not finish applying the requested settings"; app.exit(2); return;
            }
            auto window=qobject_cast<QQuickWindow*>(engine.rootObjects().first());
            if(window) window->grabWindow().save(qEnvironmentVariable("OMALUX_CAPTURE"));
            frames->image().save(qEnvironmentVariable("OMALUX_CAPTURE")+".preview.png");
            app.quit();
        });
    }
    const int result = app.exec();
    for(auto *root : engine.rootObjects()) delete root;
    return result;
}
#include "main.moc"
