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
#include <QDebug>
#include "controls.h"
#include <QVariantList>
#include <QVariantMap>
#include <array>
#include <cmath>
#include <condition_variable>
#include <mutex>
#include <thread>
#include <vector>

extern "C" {
int om_engine_init(int, char **);
const char *om_engine_gpu_warning();
int om_engine_open(const char *);
int om_engine_render(const float *, const unsigned char **, int *, int *);
void om_engine_cleanup();
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
    Q_PROPERTY(QString filename READ filename CONSTANT)
    Q_PROPERTY(QVariantList controls READ controls CONSTANT)
    Q_PROPERTY(QVariantMap controlValues READ controlValues NOTIFY controlsChanged)
public:
    Editor(Frames *frames, QString source, std::vector<QByteArray> arguments)
        : frames(frames), source(source), arguments(std::move(arguments)) {
        for(unsigned int i=0; i<OM_CONTROL_COUNT; ++i) values[i]=requested[i]=om_controls[i].initial;
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
    QVariantList controls() const {
        QVariantList result;
        for(unsigned int i=0; i<OM_CONTROL_COUNT; ++i) {
            const auto &c=om_controls[i];
            result.append(QVariantMap{{"id", c.id}, {"label", c.label}, {"minimum", c.minimum},
                {"maximum", c.maximum}, {"step", c.step}});
        }
        return result;
    }
    QVariantMap controlValues() const {
        QVariantMap result;
        for(unsigned int i=0; i<OM_CONTROL_COUNT; ++i) result[om_controls[i].id]=values[i];
        return result;
    }
    Q_INVOKABLE void setControl(const QString &id, double next) {
        if(!std::isfinite(next)) return;
        for(unsigned int i=0; i<OM_CONTROL_COUNT; ++i) {
            const auto &c=om_controls[i];
            if(id != QLatin1String(c.id)) continue;
            next=qBound(double(c.minimum), next, double(c.maximum));
            if(values[i] == next) return;
            values[i]=next;
            queueControls();
            return;
        }
    }
    Q_INVOKABLE void adjustControl(const QString &id, int steps) {
        for(unsigned int i=0; i<OM_CONTROL_COUNT; ++i)
            if(id == QLatin1String(om_controls[i].id)) setControl(id, values[i]+steps*om_controls[i].step);
    }
    Q_INVOKABLE void resetControls() {
        for(unsigned int i=0; i<OM_CONTROL_COUNT; ++i) values[i]=om_controls[i].initial;
        queueControls();
    }
signals:
    void changed();
    void controlsChanged();
private:
    void queueControls() {
        {std::lock_guard lock(mutex); requested=values; ++generation; pending=true;}
        wake.notify_one(); emit controlsChanged();
    }
    void fail(const QString &text) {
        QMetaObject::invokeMethod(this,[this,text]{message = text; emit changed();},Qt::QueuedConnection);
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
        while(true) {
            std::array<float, OM_CONTROL_COUNT> next; unsigned long revision;
            {std::unique_lock lock(mutex); wake.wait(lock,[this]{return stopping || pending;});
             if(stopping) break; next = requested; revision = generation; pending = false;}
            // Private latest-value mailbox for the optional comparison process.
            // Atomic replacement; never wait for darktable to consume or render it.
            const QString mailbox=qEnvironmentVariable("OMALUX_COMPARISON_MAILBOX");
            if(!mailbox.isEmpty()) {
                QSaveFile file(mailbox);
                QByteArray command="omalux-controls-v1 " + QByteArray::number(revision)+"\n";
                for(unsigned int i=0; i<OM_CONTROL_COUNT; ++i)
                    command += QByteArray(om_controls[i].module)+" "+om_controls[i].parameter+" "
                        +QByteArray::number(om_parameter_value(i,next[i]), 'g', 9)+"\n";
                if(!file.open(QIODevice::WriteOnly) || file.write(command)!=command.size() || !file.commit())
                    qWarning() << "Could not synchronize comparison controls:" << file.errorString();
            }
            QElapsedTimer timer; timer.start();
            const unsigned char *pixels = nullptr; int width=0,height=0;
            if(om_engine_render(next.data(),&pixels,&width,&height)) {fail("darktable preview failed");continue;}
            QImage result(pixels,width,height,width*4,QImage::Format_RGB32);
            auto copy=result.copy();
            // darktable's display buffer is BGRx; gamma leaves x undefined.
            // Qt RGB32 requires 0xff alpha, including during texture upload.
            for(int y=0; y<copy.height(); ++y) {
                auto *row=reinterpret_cast<QRgb *>(copy.scanLine(y));
                for(int x=0; x<copy.width(); ++x) row[x] |= 0xff000000u;
            }
            {std::lock_guard lock(mutex); if(revision != generation) continue;}
            const qint64 elapsed=timer.elapsed();
            const QString warning=QString::fromUtf8(om_engine_gpu_warning());
            QMetaObject::invokeMethod(this,[this,copy,elapsed,revision,warning] {
                gpuMessage=warning;
                {std::lock_guard lock(mutex); if(revision != generation) return;}
                frames->set(copy); url=QString("image://preview/%1").arg(revision);
                message=QString("Ready · %1 · %2 ms").arg(warning.isEmpty() ? "OpenCL auto" : "CPU").arg(elapsed);
                qInfo().noquote() << message; emit changed();
            },Qt::QueuedConnection);
        }
        om_engine_cleanup();
    }
    Frames *frames; QString source,url,gpuMessage,message="Loading image…";
    std::vector<QByteArray> arguments;
    std::array<float, OM_CONTROL_COUNT> values{}, requested{};
    std::mutex mutex; std::condition_variable wake; std::thread worker;
    bool stopping=false,pending=true; unsigned long generation=1;
};
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
    // Optional offscreen development capture, no effect during normal launches.
    if(qEnvironmentVariableIsSet("OMALUX_CAPTURE")) {
        QTimer::singleShot(qEnvironmentVariableIntValue("OMALUX_CAPTURE_DELAY") > 0 ? qEnvironmentVariableIntValue("OMALUX_CAPTURE_DELAY") / 2 : 2000,&app,[&]{frames->image().save(qEnvironmentVariable("OMALUX_CAPTURE")+".neutral.png"); editor.setControl(qEnvironmentVariable("OMALUX_CAPTURE_CONTROL", "brightness"), qEnvironmentVariableIsSet("OMALUX_CAPTURE_VALUE") ? qEnvironmentVariable("OMALUX_CAPTURE_VALUE").toDouble() : 30);});
        QTimer::singleShot(qEnvironmentVariableIntValue("OMALUX_CAPTURE_DELAY") > 0 ? qEnvironmentVariableIntValue("OMALUX_CAPTURE_DELAY") : 4500,&app,[&]{
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
