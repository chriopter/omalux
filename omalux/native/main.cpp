// SPDX-License-Identifier: GPL-3.0-or-later
#include <QQuickWindow>
#include <QGuiApplication>
#include <QQmlApplicationEngine>
#include <QQmlContext>
#include <QQuickImageProvider>
#include <QImage>
#include <QTimer>
#include <QElapsedTimer>
#include <QFileInfo>
#include <QDebug>
#include <atomic>
#include <condition_variable>
#include <mutex>
#include <thread>
#include <vector>

extern "C" {
int om_engine_init(int, char **);
int om_engine_open(const char *);
int om_engine_render(float, const unsigned char **, int *, int *);
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
    Q_PROPERTY(QString filename READ filename CONSTANT)
    Q_PROPERTY(double brightness READ brightness WRITE setBrightness NOTIFY changed)
public:
    Editor(Frames *frames, QString source, std::vector<QByteArray> arguments)
        : frames(frames), source(source), arguments(std::move(arguments)) {
        worker = std::thread([this] { run(); });
    }
    ~Editor() override {
        {std::lock_guard lock(mutex); stopping = true;}
        wake.notify_one(); worker.join();
    }
    QString preview() const { return url; }
    QString status() const { return message; }
    QString filename() const { return QFileInfo(source).fileName(); }
    double brightness() const { return value; }
    void setBrightness(double next) {
        next = qBound(-100.0, next, 100.0);
        if(value == next) return;
        value = next;
        {std::lock_guard lock(mutex); requested = next; ++generation; pending = true;}
        wake.notify_one(); emit changed();
    }
signals:
    void changed();
private:
    void fail(const QString &text) {
        QMetaObject::invokeMethod(this,[this,text]{message = text; emit changed();},Qt::QueuedConnection);
    }
    void run() {
        std::vector<char *> argv;
        for(auto &arg:arguments) argv.push_back(arg.data());
        argv.push_back(nullptr);
        if(om_engine_init(int(arguments.size()),argv.data())) {fail("darktable initialization failed");return;}
        if(om_engine_open(source.toUtf8().constData())) {
            fail("Could not open image with darktable"); om_engine_cleanup(); return;
        }
        while(true) {
            float next; unsigned long revision;
            {std::unique_lock lock(mutex); wake.wait(lock,[this]{return stopping || pending;});
             if(stopping) break; next = requested; revision = generation; pending = false;}
            QElapsedTimer timer; timer.start();
            const unsigned char *pixels = nullptr; int width=0,height=0;
            if(om_engine_render(next,&pixels,&width,&height)) {fail("darktable preview failed");continue;}
            QImage result(pixels,width,height,width*4,QImage::Format_RGB32);
            auto copy=result.copy();
            {std::lock_guard lock(mutex); if(revision != generation) continue;}
            const qint64 elapsed=timer.elapsed();
            QMetaObject::invokeMethod(this,[this,copy,elapsed,revision] {
                {std::lock_guard lock(mutex); if(revision != generation) return;}
                frames->set(copy); url=QString("image://preview/%1").arg(revision);
                message=QString("Ready · darktable · %1 ms").arg(elapsed);
                qInfo().noquote() << message; emit changed();
            },Qt::QueuedConnection);
        }
        om_engine_cleanup();
    }
    Frames *frames; QString source,url,message="Loading image…";
    std::vector<QByteArray> arguments;
    double value=0; float requested=0;
    std::mutex mutex; std::condition_variable wake; std::thread worker;
    bool stopping=false,pending=true; unsigned long generation=1;
};
int main(int argc,char **argv) {
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
        QTimer::singleShot(2000,&app,[&]{frames->image().save(qEnvironmentVariable("OMALUX_CAPTURE")+".neutral.png"); editor.setBrightness(30);});
        QTimer::singleShot(4500,&app,[&]{
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
