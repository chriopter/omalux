// SPDX-License-Identifier: GPL-3.0-or-later
// Interface timing driver, inactive during normal use. OMALUX_PERF_SCRIPT names a JSON array
// of steps; each runs once the window has been quiet (no frame for 300 ms, no style busy) and
// is measured until it is quiet again. Every step prints one "Perf" line:
//   sync     time spent handling the input events or the property change itself (GUI thread)
//   frame    from the step's start to the first frame shown after it
//   settled  from the step's start to the last frame before the window went quiet
//   stall    the longest the GUI thread did not get to its event loop during the step
// Sequences (drag, wheel, keys with an interval) also report frames per second while they
// ran, the delay from each event to the next frame (the value shown) and to the next preview
// frame of the engine. Steps:
//   {"name": "...", "set": {"property": "selectedPanel", "value": 1}}   root window property
//   {"name": "...", "call": "revealControl", "args": ["exposure"]}      root window method
//   {"name": "...", "click": "objectName", "at": [0.5, 0.5]}            press and release
//   {"name": "...", "drag": "control-slider-…", "from": 0.2, "to": 0.8, "steps": 60, "interval": 16}
//   {"name": "...", "wheel": "objectName", "dy": -120, "count": 20, "interval": 16}
//   {"name": "...", "key": "Down", "repeat": 10, "interval": 0}
//   {"name": "...", "type": "text", "interval": 60}
//   {"reveal": "objectName"}   scroll the pane so the item is on screen (not measured)
//   {"wait": 500}
// Item names are objectNames of visible items in the window.
#include "perf.h"
#include "app/editor.h"
#include <QGuiApplication>
#include <QQmlApplicationEngine>
#include <QQuickWindow>
#include <QQuickItem>
#include <QJsonDocument>
#include <QJsonArray>
#include <QJsonObject>
#include <QElapsedTimer>
#include <QKeySequence>
#include <QKeyEvent>
#include <QMouseEvent>
#include <QWheelEvent>
#include <QFile>
#include <QTimer>
#include <QPointer>
#include <QDebug>
#include <algorithm>
#include <functional>
#include <memory>
#include <mutex>
#include <vector>
#include <ctime>
#include <unistd.h>

namespace {
// Milliseconds since this process started (from /proc, 10 ms resolution).
double sinceProcessStart() {
    QFile stat("/proc/self/stat");
    if (!stat.open(QIODevice::ReadOnly))
        return -1;
    const auto text = QString::fromUtf8(stat.readAll());
    const auto fields = text.mid(text.lastIndexOf(')') + 2).split(' ');
    if (fields.size() < 20)
        return -1;
    const double start = fields[19].toDouble() / sysconf(_SC_CLK_TCK) * 1000;
    timespec now{};
    clock_gettime(CLOCK_BOOTTIME, &now);
    return now.tv_sec * 1000.0 + now.tv_nsec / 1e6 - start;
}

struct Perf : QObject {
    QGuiApplication &app;
    Editor &editor;
    QQuickWindow *window;
    QJsonArray steps;
    int index = 0;
    QElapsedTimer clock;
    std::mutex lock;
    std::vector<double> frames;      // frame swaps, ms on `clock`
    std::vector<double> previews;    // preview changes
    QString lastPreview;
    QTimer probe, poll;
    double lastTick = 0, maxGap = 0;
    double stepStart = 0, syncTotal = 0, syncMax = 0;
    std::vector<double> events;      // sequence event times
    double sequenceEnd = 0;
    bool startup = true;
    double loaded = 0, ready = -1;

    Perf(QGuiApplication &a, Editor &e, QQuickWindow *w, QJsonArray s)
        : QObject(&a), app(a), editor(e), window(w), steps(std::move(s)) {
        clock.start();
        loaded = sinceProcessStart();
        connect(window, &QQuickWindow::frameSwapped, this, [this] {
            std::lock_guard guard(lock);
            frames.push_back(now());
        }, Qt::DirectConnection);
        connect(&editor, &Editor::changed, this, [this] {
            if (editor.preview() != lastPreview) {
                lastPreview = editor.preview();
                previews.push_back(now());
            }
        });
        // The GUI thread's responsiveness: a 2 ms tick that is late by the time it was blocked.
        probe.setTimerType(Qt::PreciseTimer);
        probe.setInterval(2);
        connect(&probe, &QTimer::timeout, this, [this] {
            const double t = now();
            maxGap = std::max(maxGap, t - lastTick - 2);
            lastTick = t;
        });
        lastTick = now();
        probe.start();
        poll.setInterval(10);
        connect(&poll, &QTimer::timeout, this, [this] { tick(); });
        poll.start();
    }
    double now() const { return clock.nsecsElapsed() / 1e6; }
    double lastFrame() {
        std::lock_guard guard(lock);
        return frames.empty() ? -1e9 : frames.back();
    }
    std::vector<double> framesSince(double t) {
        std::lock_guard guard(lock);
        std::vector<double> out;
        for (double f : frames)
            if (f >= t)
                out.push_back(f);
        return out;
    }
    bool engineIdle() const {
        return editor.stylesReady() && !editor.preview().isEmpty() && !editor.styleBusy();
    }
    bool quiet(double span = 300) { return engineIdle() && now() - lastFrame() >= span; }

    QQuickItem *find(const QString &name) {
        std::function<QQuickItem *(QQuickItem *)> walk = [&](QQuickItem *item) -> QQuickItem * {
            if (!item->isVisible())
                return nullptr;
            if (item->objectName() == name)
                return item;
            for (auto *child : item->childItems())
                if (auto *found = walk(child))
                    return found;
            return nullptr;
        };
        return walk(window->contentItem());
    }
    void mouse(QQuickItem *item, QEvent::Type type, QPointF local, Qt::MouseButtons buttons) {
        const QPointF point = item->mapToScene(local);
        QMouseEvent event(type, point, window->mapToGlobal(point.toPoint()),
                          type == QEvent::MouseMove ? Qt::NoButton : Qt::LeftButton, buttons, Qt::NoModifier);
        QGuiApplication::sendEvent(window, &event);
    }
    void key(const QString &spec) {
        const QKeySequence sequence(spec);
        if (sequence.isEmpty())
            return;
        const auto combination = sequence[0];
        const QString text = spec.size() == 1 ? spec : QString();
        QKeyEvent press(QEvent::KeyPress, combination.key(), combination.keyboardModifiers(), text);
        QGuiApplication::sendEvent(window, &press);
        QKeyEvent release(QEvent::KeyRelease, combination.key(), combination.keyboardModifiers(), text);
        QGuiApplication::sendEvent(window, &release);
    }
    // Times one synchronous piece of input handling.
    void timed(const std::function<void()> &action) {
        const double t = now();
        action();
        const double spent = now() - t;
        syncTotal += spent;
        syncMax = std::max(syncMax, spent);
        events.push_back(t);
    }

    enum class Phase { WaitIdle, Running, Settling } phase = Phase::WaitIdle;
    QJsonObject step;

    void tick() {
        if (startup) {
            // Interactive: the engine's first preview is there and a frame shows it.
            if (ready < 0 && engineIdle())
                ready = now();
            const auto shown = framesSince(ready < 0 ? 1e18 : ready);
            if (shown.empty())
                return;
            qInfo().noquote() << QString("Perf startup: QML loaded %1 ms, interactive %2 ms after process start")
                                     .arg(loaded, 0, 'f', 0)
                                     .arg(loaded + shown.front(), 0, 'f', 0);
            startup = false;
        }
        if (phase == Phase::WaitIdle) {
            if (!quiet())
                return;
            if (index >= steps.size()) {
                qInfo().noquote() << "Perf complete" + (skipped ? QString(", %1 steps skipped (missing items)").arg(skipped) : QString());
                poll.stop();
                QTimer::singleShot(0, &app, [this] { app.quit(); });
                return;
            }
            step = steps[index++].toObject();
            begin();
        } else if (phase == Phase::Settling) {
            const double t = now();
            if (t - sequenceEnd < 300 || !quiet())
                return;
            if (t - stepStart > 30000)
                qWarning() << "Perf step did not settle" << step;
            report();
            phase = Phase::WaitIdle;
        }
    }

    void begin() {
        events.clear();
        syncTotal = syncMax = 0;
        if (step.contains("wait")) {
            phase = Phase::Running;
            QTimer::singleShot(step["wait"].toInt(), this, [this] { phase = Phase::WaitIdle; });
            return;
        }
        if (step.contains("reveal")) {
            reveal(step["reveal"].toString());
            return;
        }
        // Measured steps.
        maxGap = 0;
        lastTick = now();
        stepStart = now();
        phase = Phase::Running;
        if (step.contains("set")) {
            const auto set = step["set"].toObject();
            timed([&] { window->setProperty(set["property"].toString().toUtf8(), set["value"].toVariant()); });
            finish();
        } else if (step.contains("call")) {
            QVariantList args = step["args"].toArray().toVariantList();
            timed([&] {
                const auto name = step["call"].toString().toUtf8();
                bool ok = false;
                switch (args.size()) {
                case 0: ok = QMetaObject::invokeMethod(window, name); break;
                case 1: ok = QMetaObject::invokeMethod(window, name, Q_ARG(QVariant, args[0])); break;
                default:
                    ok = QMetaObject::invokeMethod(window, name, Q_ARG(QVariant, args[0]), Q_ARG(QVariant, args[1]));
                }
                if (!ok)
                    qWarning() << "Perf call failed" << step;
            });
            finish();
        } else if (step.contains("click")) {
            auto *item = find(step["click"].toString());
            if (!item)
                return missing();
            const auto at = step["at"].toArray();
            const QPointF local(item->width() * (at.isEmpty() ? .5 : at[0].toDouble()),
                                item->height() * (at.isEmpty() ? .5 : at[1].toDouble()));
            timed([&] {
                mouse(item, QEvent::MouseButtonPress, local, Qt::LeftButton);
                mouse(item, QEvent::MouseButtonRelease, local, Qt::NoButton);
            });
            finish();
        } else if (step.contains("drag")) {
            auto *item = find(step["drag"].toString());
            if (!item)
                return missing();
            const double from = step["from"].toDouble(.2), to = step["to"].toDouble(.8);
            const int count = step["steps"].toInt(60);
            auto at = [item](double f) { return QPointF(5 + (item->width() - 10) * f, item->height() / 2); };
            QPointer<QQuickItem> target(item);
            timed([&] { mouse(item, QEvent::MouseButtonPress, at(from), Qt::LeftButton); });
            sequence(count, step["interval"].toInt(16), [this, target, at, from, to, count](int i) {
                if (!target)
                    return;
                if (i < count)
                    mouse(target, QEvent::MouseMove, at(from + (to - from) * (i + 1) / count), Qt::LeftButton);
                else
                    mouse(target, QEvent::MouseButtonRelease, at(to), Qt::NoButton);
            }, true);
        } else if (step.contains("wheel")) {
            auto *item = find(step["wheel"].toString());
            if (!item)
                return missing();
            QPointer<QQuickItem> target(item);
            const int dy = step["dy"].toInt(-120);
            sequence(step["count"].toInt(20), step["interval"].toInt(16), [this, target, dy](int) {
                if (!target)
                    return;
                const QPointF point = target->mapToScene(QPointF(target->width() / 2, target->height() / 2));
                QWheelEvent event(point, window->mapToGlobal(point), QPoint(), QPoint(0, dy), Qt::NoButton,
                                  Qt::NoModifier, Qt::NoScrollPhase, false);
                QGuiApplication::sendEvent(window, &event);
            });
        } else if (step.contains("key")) {
            const auto spec = step["key"].toString();
            sequence(step["repeat"].toInt(1), step["interval"].toInt(0), [this, spec](int) { key(spec); });
        } else if (step.contains("type")) {
            const auto text = step["type"].toString();
            sequence(text.size(), step["interval"].toInt(60), [this, text](int i) { key(text.mid(i, 1)); });
        } else {
            qWarning() << "Perf unknown step" << step;
            phase = Phase::WaitIdle;
        }
    }
    // A script names items of panes that keep changing: a missing one is reported and skipped,
    // so the remaining steps are still measured.
    int skipped = 0;
    void missing() {
        qWarning() << "Perf missing item, step skipped" << step;
        ++skipped;
        phase = Phase::WaitIdle;
    }
    void finish() {
        sequenceEnd = now();
        phase = Phase::Settling;
    }
    // `count` events `interval` ms apart (0: all at once); `release` adds one closing event.
    void sequence(int count, int interval, std::function<void(int)> action, bool release = false) {
        const int total = count + (release ? 1 : 0);
        if (interval <= 0) {
            for (int i = 0; i < total; ++i)
                timed([&] { action(i); });
            finish();
            return;
        }
        auto *timer = new QTimer(this);
        timer->setTimerType(Qt::PreciseTimer);
        timer->setInterval(interval);
        auto i = std::make_shared<int>(0);
        connect(timer, &QTimer::timeout, this, [this, timer, i, total, action] {
            timed([&] { action(*i); });
            if (++*i < total)
                return;
            timer->stop();
            timer->deleteLater();
            finish();
        });
        timer->start();
    }
    void reveal(const QString &name) {
        auto *item = find(name);
        if (!item)
            return missing();
        for (auto *p = item->parentItem(); p; p = p->parentItem()) {
            if (!p->inherits("QQuickFlickable"))
                continue;
            auto *content = p->property("contentItem").value<QQuickItem *>();
            const double y = item->mapToItem(content, QPointF(0, 0)).y();
            const double maximum = std::max(0.0, p->property("contentHeight").toDouble() - p->height());
            p->setProperty("contentY", std::clamp(y - 40, 0.0, maximum));
            break;
        }
    }

    void report() {
        const auto shown = framesSince(stepStart);
        const double first = shown.empty() ? -1 : shown.front() - stepStart;
        const double settled = shown.empty() ? 0 : shown.back() - stepStart;
        QString line = QString("Perf %1 [pane %8]: sync %2 ms (max %3), frame %4 ms, settled %5 ms, %6 frames, stall %7 ms")
                           .arg(step["name"].toString(step.keys().join(",")))
                           .arg(syncTotal, 0, 'f', 1)
                           .arg(syncMax, 0, 'f', 1)
                           .arg(first, 0, 'f', 1)
                           .arg(settled, 0, 'f', 1)
                           .arg(shown.size())
                           .arg(maxGap, 0, 'f', 1)
                           .arg(window->property("selectedPanel").toInt());
        if (events.size() > 2) {
            const double span = sequenceEnd - stepStart;
            int during = 0;
            for (double f : shown)
                during += f <= sequenceEnd;
            // Delay from each event to the next frame and to the next preview of the engine.
            double frameSum = 0, frameMax = 0, previewSum = 0;
            int previewCount = 0;
            for (double e : events) {
                const auto next = std::lower_bound(shown.begin(), shown.end(), e);
                if (next != shown.end()) {
                    frameSum += *next - e;
                    frameMax = std::max(frameMax, *next - e);
                }
                const auto preview = std::lower_bound(previews.begin(), previews.end(), e);
                if (preview != previews.end() && *preview - e < 2000) {
                    previewSum += *preview - e;
                    ++previewCount;
                }
            }
            int previewFrames = 0;
            for (double p : previews)
                previewFrames += p >= stepStart;
            line += QString(", %1 fps over %2 ms, event→frame %3 ms (max %4), event→preview %5 ms, %6 previews")
                        .arg(span > 0 ? during * 1000 / span : 0, 0, 'f', 1)
                        .arg(span, 0, 'f', 0)
                        .arg(frameSum / events.size(), 0, 'f', 1)
                        .arg(frameMax, 0, 'f', 1)
                        .arg(previewCount ? previewSum / previewCount : -1, 0, 'f', 1)
                        .arg(previewFrames);
        }
        qInfo().noquote() << line;
    }
};
} // namespace

void install_perf(QGuiApplication &app, Editor &editor, QQmlApplicationEngine &engine) {
    const auto path = qEnvironmentVariable("OMALUX_PERF_SCRIPT");
    if (path.isEmpty())
        return;
    QFile file(path);
    auto *window = qobject_cast<QQuickWindow *>(engine.rootObjects().first());
    if (!file.open(QIODevice::ReadOnly) || !window) {
        app.exit(2);
        return;
    }
    new Perf(app, editor, window, QJsonDocument::fromJson(file.readAll()).array());
}
