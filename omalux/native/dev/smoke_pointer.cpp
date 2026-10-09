// SPDX-License-Identifier: GPL-3.0-or-later
// Smoke steps that use the mouse exactly as a person does: the events enter through Qt's
// window-system interface (as QTest does), so hover, press grabs, focus changes, double-click
// detection and everything a control does between press and release run as in the real app.
// Items are found by objectName or, with a "text:" prefix, by their text; the first visible one
// wins. Positions are fractions of the item ("at", default the centre).
//   {"pointerClick": "sidebar-area-edit", "at": [x, y], "button": "right", "clicks": 2,
//    "modifiers": "ctrl", "expectTop": true}   expectTop fails when another item covers it
//   {"pointerDrag": name, "from": [x, y], "to": [x, y], "modifiers": "shift"}
//   {"pointerHover": name}, {"waitMs": 1000}; the name "@last" repeats the previous point
//   {"rememberItem": name} notes where an item is in the window and how large;
//   {"checkItemUnmoved": name} fails when it has moved or changed size since
//   {"pointerDrag": name, ..., "hold": true} keeps the button down at "to" (the state in the
//   middle of a drag can be checked); {"pointerRelease": true} lets it go there
//   {"checkItem": name, "property": "checked", "value": true}  or "visible": false, or
//   "inWindow": true (shown on screen, not scrolled away)
#include "smoke_pointer.h"
#include <QDebug>
#include <QElapsedTimer>
#include <QGuiApplication>
#include <QHash>
#include <QJsonArray>
#include <QJsonObject>
#include <QPointer>
#include <QQmlApplicationEngine>
#include <QQuickItem>
#include <QQuickWindow>
#include <QTimer>
#include <algorithm>
#include <cmath>
#include <functional>

// Defined in QtGui (qwindowsysteminterface.cpp) and declared by QtTest's qtestmouse.h: delivers
// a mouse event synchronously through QGuiApplication, the path of a real pointer.
Q_GUI_EXPORT void qt_handleMouseEvent(QWindow *window, const QPointF &local, const QPointF &global,
                                      Qt::MouseButtons state, Qt::MouseButton button, QEvent::Type type,
                                      Qt::KeyboardModifiers mods, int timestamp);

namespace {
// A clock for the events: separate steps are far apart, so they never count as a double click.
int stamp(int advance) {
    static int now = 1000;
    now += advance;
    return now;
}

bool shown(QQuickItem *item) {
    for (auto *p = item; p; p = p->parentItem())
        if (!p->isVisible() || p->opacity() <= 0)
            return false;
    return item->width() > 0 && item->height() > 0;
}

// The point of the item is on screen: inside the window and every clipping parent.
bool onScreen(QQuickItem *item, QQuickWindow *window, QPointF scene) {
    if (!QRectF(0, 0, window->width(), window->height()).contains(scene))
        return false;
    for (auto *p = item->parentItem(); p; p = p->parentItem())
        if (p->clip() && !p->contains(p->mapFromScene(scene)))
            return false;
    return true;
}

QQuickItem *findItem(QQuickItem *root, const QString &name) {
    const bool byText = name.startsWith("text:");
    const QString wanted = byText ? name.mid(5) : name;
    std::function<QQuickItem *(QQuickItem *)> walk = [&](QQuickItem *item) -> QQuickItem * {
        if (!item->isVisible())
            return nullptr;
        const bool match =
            byText ? item->property("text").toString() == wanted : item->objectName() == wanted;
        if (match && shown(item))
            return item;
        for (auto *child : item->childItems())
            if (auto *found = walk(child))
                return found;
        return nullptr;
    };
    return walk(root);
}

// The topmost item under a scene point that takes mouse buttons (diagnostics: what a click hits).
QQuickItem *topmost(QQuickItem *item, const QPointF &scene) {
    if (!item->isVisible() || item->opacity() <= 0)
        return nullptr;
    const bool inside = item->contains(item->mapFromScene(scene));
    if (item->clip() && !inside)
        return nullptr;
    auto children = item->childItems();
    std::stable_sort(children.begin(), children.end(),
                     [](QQuickItem *a, QQuickItem *b) { return a->z() < b->z(); });
    for (auto it = children.rbegin(); it != children.rend(); ++it)
        if (auto *found = topmost(*it, scene))
            return found;
    // The popup layer spans the window but lets clicks through while no modal popup is open.
    if (inside && item->isEnabled() && item->acceptedMouseButtons() != Qt::NoButton &&
        qstrcmp(item->metaObject()->className(), "QQuickOverlay") != 0)
        return item;
    return nullptr;
}

QString describe(QQuickItem *item) {
    if (!item)
        return "nothing";
    QStringList chain;
    for (auto *p = item; p && chain.size() < 4; p = p->parentItem())
        chain << QString("%1(%2)").arg(p->metaObject()->className(), p->objectName());
    return chain.join(" < ");
}

Qt::KeyboardModifiers modifiersOf(const QJsonObject &step) {
    const auto name = step["modifiers"].toString();
    return name == "ctrl"    ? Qt::ControlModifier
           : name == "shift" ? Qt::ShiftModifier
           : name == "alt"   ? Qt::AltModifier
                             : Qt::NoModifier;
}

QPointF fraction(const QJsonValue &value, double x, double y) {
    const auto a = value.toArray();
    return a.size() == 2 ? QPointF(a[0].toDouble(), a[1].toDouble()) : QPointF(x, y);
}
} // namespace

SmokeResult pointerSmokeStep(const QJsonObject &step, QQmlApplicationEngine &engine,
                             std::shared_ptr<bool> busy) {
    if (step.contains("waitMs")) {
        // Time passes with the pointer at rest (tooltip delays, hover timers).
        *busy = true;
        QTimer::singleShot(step["waitMs"].toInt(), qApp, [busy] { *busy = false; });
        return SmokeResult::Done;
    }
    // The point and button of a drag that is still held ("hold").
    static QPointF heldPoint;
    static Qt::MouseButton heldButton = Qt::NoButton;
    if (step.contains("pointerRelease")) {
        auto *held = qobject_cast<QQuickWindow *>(engine.rootObjects().first());
        if (heldButton == Qt::NoButton) {
            qCritical() << "No held drag to release";
            return SmokeResult::Fail;
        }
        qt_handleMouseEvent(held, heldPoint, held->mapToGlobal(heldPoint), Qt::NoButton, heldButton,
                            QEvent::MouseButtonRelease, Qt::NoModifier, stamp(60));
        heldButton = Qt::NoButton;
        return SmokeResult::Done;
    }
    if (step.contains("rememberItem") || step.contains("checkItemUnmoved")) {
        static QHash<QString, QRectF> remembered;
        const bool check = step.contains("checkItemUnmoved");
        const auto which = step[check ? "checkItemUnmoved" : "rememberItem"].toString();
        auto *shown = qobject_cast<QQuickWindow *>(engine.rootObjects().first());
        shown->grabWindow(); // lay out as a screen would (see below)
        auto *found = findItem(shown->contentItem(), which);
        if (!found) {
            qCritical() << "Missing or hidden item" << which;
            return SmokeResult::Fail;
        }
        const QRectF now(found->mapToScene(QPointF(0, 0)), QSizeF(found->width(), found->height()));
        if (!check) {
            remembered[which] = now;
            return SmokeResult::Done;
        }
        const QRectF then = remembered.value(which);
        if (!remembered.contains(which) || std::abs(then.x() - now.x()) > .01 ||
            std::abs(then.y() - now.y()) > .01 || std::abs(then.width() - now.width()) > .01 ||
            std::abs(then.height() - now.height()) > .01) {
            qCritical() << "Item moved" << which << then << now;
            return SmokeResult::Fail;
        }
        return SmokeResult::Done;
    }
    const char *keys[] = {"pointerClick", "pointerDrag", "pointerHover", "checkItem"};
    const char *kind = nullptr;
    for (const char *key : keys)
        if (step.contains(key))
            kind = key;
    if (!kind)
        return SmokeResult::NotHandled;
    auto *window = qobject_cast<QQuickWindow *>(engine.rootObjects().first());
    // A shown window lays out and draws a frame within milliseconds; offscreen nothing draws
    // until asked, so positioners may still hold the previous layout. Draw one, as a screen would.
    window->grabWindow();
    const auto name = step[kind].toString();
    // "@last": the window point of the previous gesture again (a second click at the same spot).
    static QPointF lastPoint;
    auto *item = name == "@last" ? window->contentItem() : findItem(window->contentItem(), name);
    if (QString(kind) == "checkItem") {
        if (step.contains("visible")) {
            if ((item != nullptr) != step["visible"].toBool()) {
                qCritical() << "Unexpected visibility of" << name;
                return SmokeResult::Fail;
            }
            return SmokeResult::Done;
        }
        if (!item) {
            qCritical() << "Missing item" << name;
            return SmokeResult::Fail;
        }
        if (step.contains("inWindow")) {
            // Scrolled into view: its centre lies inside the window and inside every clipping parent.
            const QPointF centre = item->mapToScene(QPointF(item->width() / 2, item->height() / 2));
            if (onScreen(item, window, centre) != step["inWindow"].toBool()) {
                qCritical() << "Unexpected placement of" << name << centre;
                return SmokeResult::Fail;
            }
            return SmokeResult::Done;
        }
        const auto actual = item->property(step["property"].toString().toUtf8()).toString();
        if (actual != step["value"].toVariant().toString()) {
            qCritical() << "Unexpected" << step["property"].toString() << "of" << name << actual;
            return SmokeResult::Fail;
        }
        return SmokeResult::Done;
    }
    if (!item) {
        qCritical() << "Missing or hidden item" << name;
        return SmokeResult::Fail;
    }
    const auto modifiers = modifiersOf(step);
    const auto button = step["button"].toString() == "right" ? Qt::RightButton : Qt::LeftButton;
    auto scene = [item](QPointF f) {
        return item->mapToScene(QPointF(f.x() * item->width(), f.y() * item->height()));
    };
    QPointF start = name == "@last"
                        ? lastPoint
                        : scene(fraction(step[QString(kind) == "pointerDrag" ? "from" : "at"], .5, .5));
    // Out of view in a scrolled pane: scroll it into view first, as a person would.
    if (name != "@last" && !onScreen(item, window, start))
        for (auto *p = item->parentItem(); p; p = p->parentItem())
            if (p->property("flickableDirection").isValid() && p->property("contentY").isValid()) {
                auto *content = p->property("contentItem").value<QQuickItem *>();
                const double y = item->mapToItem(content, QPointF(0, 0)).y();
                // Never past the end of the pane: no wheel gets there, and a pane left beyond its
                // bounds snaps back at the next change of its content.
                const double end = std::max(0.0, p->property("contentHeight").toDouble() +
                                                     p->property("bottomMargin").toDouble() - p->height());
                const double target = std::min(end, std::max(0.0, y - p->height() / 3));
                p->setProperty("contentY", target);
                start = scene(fraction(step[QString(kind) == "pointerDrag" ? "from" : "at"], .5, .5));
                qInfo() << "Scrolled" << p << "to" << target;
                break;
            }
    if (name != "@last" && !onScreen(item, window, start)) {
        qCritical() << "Item outside the window" << name << start;
        return SmokeResult::Fail;
    }
    lastPoint = start;
    auto *hit = topmost(window->contentItem(), start);
    const bool covered = hit && hit != item && !item->isAncestorOf(hit) && !hit->isAncestorOf(item);
    qInfo() << "Pointer" << kind << name << "at" << start << "hits" << describe(hit);
    if (covered && step["expectTop"].toBool()) {
        qCritical() << "Item covered by" << describe(hit);
        return SmokeResult::Fail;
    }
    struct Trace : QObject {
        bool eventFilter(QObject *o, QEvent *e) override {
            if (e->type() >= QEvent::MouseButtonPress && e->type() <= QEvent::MouseMove)
                qInfo() << "TRACE" << o << e;
            return false;
        }
    };
    static Trace *trace = nullptr;
    // OMALUX_SMOKE_TRACE_POINTER=1 logs every mouse event and the object it reaches.
    if (!trace && qEnvironmentVariableIsSet("OMALUX_SMOKE_TRACE_POINTER")) {
        trace = new Trace;
        qApp->installEventFilter(trace);
    }
    QPointer<QQuickWindow> target(window);
    auto send = [target, modifiers](QPointF point, Qt::MouseButtons state, Qt::MouseButton changed,
                                    QEvent::Type type, int advance) {
        if (target)
            qt_handleMouseEvent(target, point, target->mapToGlobal(point), state, changed, type, modifiers,
                                stamp(advance));
    };
    // The pointer arrives first, as a hand on a mouse would bring it.
    send(start + QPointF(0, -1), Qt::NoButton, Qt::NoButton, QEvent::MouseMove, 500);
    send(start, Qt::NoButton, Qt::NoButton, QEvent::MouseMove, 16);
    if (QString(kind) == "pointerHover")
        return SmokeResult::Done;
    *busy = true;
    // Each gesture is a list of timed events; the event loop runs between them (~60 ms a press).
    struct Event {
        QPointF point;
        Qt::MouseButtons state;
        QEvent::Type type;
        int delay;
    };
    QList<Event> events;
    if (QString(kind) == "pointerClick") {
        const int clicks = std::max(1, step["clicks"].toInt(1));
        for (int i = 0; i < clicks; ++i) {
            events.append({start, button, QEvent::MouseButtonPress, i ? 60 : 30});
            events.append({start, Qt::NoButton, QEvent::MouseButtonRelease, 60});
        }
    } else {
        const QPointF end = scene(fraction(step["to"], .5, .5));
        events.append({start, button, QEvent::MouseButtonPress, 30});
        for (int i = 1; i <= 12; ++i)
            events.append({start + (end - start) * i / 12.0, button, QEvent::MouseMove, 16});
        if (step["hold"].toBool()) {
            heldPoint = end;
            heldButton = button;
        } else
            events.append({end, Qt::NoButton, QEvent::MouseButtonRelease, 60});
    }
    auto queue = std::make_shared<QList<Event>>(events);
    auto next = std::make_shared<std::function<void()>>();
    *next = [queue, next, send, busy, button] {
        if (queue->isEmpty()) {
            *busy = false;
            QTimer::singleShot(0, qApp, [next] { *next = nullptr; }); // break the cycle
            return;
        }
        const auto event = queue->takeFirst();
        send(event.point, event.state, event.type == QEvent::MouseMove ? Qt::NoButton : button, event.type,
             event.delay);
        const int delay = queue->isEmpty() ? 30 : queue->first().delay;
        QTimer::singleShot(delay, qApp, [next] {
            if (*next)
                (*next)();
        });
    };
    QTimer::singleShot(events.first().delay, qApp, [next] { (*next)(); });
    return SmokeResult::Done;
}
