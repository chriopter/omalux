// SPDX-License-Identifier: GPL-3.0-or-later
// Smoke steps for drawing on the image. Positions are fractions of the photo as shown.
//   {"canvasModule": {"operation", "instance"}}               show a module's tool
//   {"canvasEdit": {"operation", "instance", "gesture", "wait"}} one gesture through Editor
//   {"rememberCanvas": {"path": "shapes.0.id", "as": "first"}}  "$first" in later gestures
//   {"checkCanvas": {"path", "count" | "value" | "equals" | "near", "tolerance"}}
//   {"canvasPointer": {"points": [[x, y], ...], "button", "modifiers", "wheel", "wait"}}
//   {"drawnShape": {"operation", "instance", "shape"}}  a blend section's shape button
//   {"checkCanvasItem": {"property": "kind", "equals": "shapes"}} on the overlay item
//   {"rememberPixels": "name"}, {"checkPixels": {"name", "same": true, "tolerance"}}: the
//   preview against a remembered one by mean absolute 8-bit difference (drawn shapes must
//   invalidate darktable's pipe cache); {"checkPixels": {"grey": true, "tolerance"}} by the
//   mean spread between the channels (a mask shown in grey)
#include "smoke_canvas.h"
#include <algorithm>
#include "app/editor.h"
#include "app/frames.h"
#include <QGuiApplication>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QMouseEvent>
#include <QQmlApplicationEngine>
#include <QQuickItem>
#include <QQuickWindow>
#include <QWheelEvent>
#include <QDebug>
#include <cmath>
#include <functional>

static QVariantMap &remembered() {
    static QVariantMap values;
    return values;
}

static QMap<QString, QImage> &rememberedImages() {
    static QMap<QString, QImage> images;
    return images;
}

static double meanDifference(const QImage &a, const QImage &b) {
    if (a.size() != b.size() || a.isNull())
        return 255.0;
    const QImage x = a.convertToFormat(QImage::Format_RGB32), y = b.convertToFormat(QImage::Format_RGB32);
    double sum = 0;
    for (int row = 0; row < x.height(); ++row) {
        const QRgb *p = reinterpret_cast<const QRgb *>(x.constScanLine(row));
        const QRgb *q = reinterpret_cast<const QRgb *>(y.constScanLine(row));
        for (int col = 0; col < x.width(); ++col)
            sum += std::abs(qRed(p[col]) - qRed(q[col])) + std::abs(qGreen(p[col]) - qGreen(q[col])) +
                   std::abs(qBlue(p[col]) - qBlue(q[col]));
    }
    return sum / (3.0 * x.width() * x.height());
}

static QJsonValue lookup(QJsonValue value, const QString &path) {
    if (path.isEmpty())
        return value;
    for (const auto &part : path.split('.')) {
        bool numeric = false;
        const int index = part.toInt(&numeric);
        if (numeric && value.isArray())
            value = value.toArray().at(index);
        else if (part == "length" && value.isArray())
            value = value.toArray().size();
        else
            value = value.toObject().value(part);
    }
    return value;
}

static QVariant substitute(const QVariant &value) {
    if (value.typeId() == QMetaType::QString && value.toString().startsWith('$'))
        return remembered().value(value.toString().mid(1), value);
    if (value.typeId() == QMetaType::QVariantMap) {
        QVariantMap map = value.toMap();
        for (auto it = map.begin(); it != map.end(); ++it)
            it.value() = substitute(it.value());
        return map;
    }
    if (value.typeId() == QMetaType::QVariantList) {
        QVariantList list = value.toList();
        for (auto &item : list)
            item = substitute(item);
        return list;
    }
    return value;
}

static QQuickItem *findItem(QQuickItem *parent, const QString &name) {
    if (parent->objectName() == name && parent->isVisible())
        return parent;
    for (auto *child : parent->childItems())
        if (auto *found = findItem(child, name))
            return found;
    return nullptr;
}

static Qt::KeyboardModifiers modifiersOf(const QString &text) {
    Qt::KeyboardModifiers result;
    if (text.contains("ctrl"))
        result |= Qt::ControlModifier;
    if (text.contains("shift"))
        result |= Qt::ShiftModifier;
    if (text.contains("alt"))
        result |= Qt::AltModifier;
    return result;
}

SmokeResult canvasSmokeStep(const QJsonObject &step, Editor &editor, QQmlApplicationEngine &engine) {
    if (step.contains("canvasModule")) {
        const auto call = step["canvasModule"].toObject();
        editor.setCanvasModule(call["operation"].toString(), call["instance"].toInt());
        return SmokeResult::Done;
    }
    if (step.contains("drawnShape")) {
        // A shape button of a blend section, as EditorSidebar.drawnShapeRequested reaches Main.
        const auto call = step["drawnShape"].toObject();
        QMetaObject::invokeMethod(engine.rootObjects().first(), "requestDrawnShape",
                                  Q_ARG(QVariant, call["operation"].toString()), Q_ARG(QVariant, call["instance"].toInt()),
                                  Q_ARG(QVariant, call["shape"].toInt()));
        return SmokeResult::Done;
    }
    if (step.contains("canvasEdit")) {
        const auto call = step["canvasEdit"].toObject();
        editor.editCanvas(call["operation"].toString(), call["instance"].toInt(),
                          substitute(call["gesture"].toObject().toVariantMap()).toMap());
        return call["wait"].toBool(true) ? SmokeResult::Wait : SmokeResult::Done;
    }
    const auto overlay = QJsonDocument::fromJson(editor.canvasOverlay().toUtf8()).object();
    if (step.contains("rememberCanvas")) {
        const auto call = step["rememberCanvas"].toObject();
        const auto value = lookup(overlay, call["path"].toString());
        if (value.isUndefined() || value.isNull())
            return SmokeResult::Retry;
        remembered()[call["as"].toString()] = value.toVariant();
        qInfo() << "Canvas remembered" << call["as"].toString() << value;
        return SmokeResult::Done;
    }
    if (step.contains("checkCanvas")) {
        const auto call = step["checkCanvas"].toObject();
        const auto value = lookup(overlay, call["path"].toString());
        const double tolerance = call["tolerance"].toDouble(1e-3);
        bool ok = !value.isUndefined();
        if (call.contains("count"))
            ok = value.isArray() && value.toArray().size() == call["count"].toInt();
        else if (call.contains("value"))
            ok = value.isDouble() && std::abs(value.toDouble() - call["value"].toDouble()) <= tolerance;
        else if (call.contains("equals"))
            ok = value.toVariant() == substitute(call["equals"].toVariant());
        else if (call.contains("near")) {
            const auto expected = call["near"].toArray();
            ok = value.isArray() && std::hypot(value.toArray().at(0).toDouble() - expected.at(0).toDouble(),
                                               value.toArray().at(1).toDouble() - expected.at(1).toDouble()) <= tolerance;
        } else if (call.contains("differs"))
            ok = value.toVariant() != substitute(call["differs"].toVariant());
        if (!ok) {
            qInfo().noquote() << "Canvas check pending" << call["path"].toString()
                              << QString::fromUtf8(QJsonDocument(QJsonArray{value}).toJson(QJsonDocument::Compact)).left(160);
            return SmokeResult::Retry;
        }
        qInfo().noquote() << "Canvas" << call["path"].toString()
                          << QString::fromUtf8(QJsonDocument(QJsonArray{value}).toJson(QJsonDocument::Compact)).left(160);
        return SmokeResult::Done;
    }
    auto *frames = static_cast<Frames *>(engine.imageProvider("preview"));
    if (step.contains("rememberPixels")) {
        rememberedImages()[step["rememberPixels"].toString()] = frames->image();
        return SmokeResult::Done;
    }
    if (step.contains("checkPixels") && step["checkPixels"].toObject().contains("grey")) {
        // {"checkPixels": {"grey": true, "tolerance": 1}}: the preview is grey (a mask shown as
        // grey levels), by the mean 8-bit spread between the channels of each pixel.
        const auto call = step["checkPixels"].toObject();
        const QImage image = frames->image().convertToFormat(QImage::Format_RGB32);
        double spread = 0;
        for (int y = 0; y < image.height(); ++y) {
            const auto *line = reinterpret_cast<const QRgb *>(image.constScanLine(y));
            for (int x = 0; x < image.width(); ++x) {
                const int r = qRed(line[x]), g = qGreen(line[x]), b = qBlue(line[x]);
                spread += std::max({r, g, b}) - std::min({r, g, b});
            }
        }
        spread /= std::max<qsizetype>(1, qsizetype(image.width()) * image.height());
        if ((spread <= call["tolerance"].toDouble(1)) != call["grey"].toBool()) {
            qInfo() << "Pixels pending" << call << spread;
            return SmokeResult::Retry;
        }
        qInfo() << "Pixels grey" << call["grey"].toBool() << "mean spread" << spread;
        return SmokeResult::Done;
    }
    if (step.contains("checkPixels")) {
        const auto call = step["checkPixels"].toObject();
        const double difference = meanDifference(rememberedImages().value(call["name"].toString()), frames->image());
        const bool same = difference <= call["tolerance"].toDouble(0.5);
        if (same != call["same"].toBool(true)) {
            qInfo() << "Pixels pending" << call << difference;
            return SmokeResult::Retry;
        }
        qInfo() << "Pixels" << call["name"].toString() << "mean difference" << difference;
        return SmokeResult::Done;
    }
    auto *window = qobject_cast<QQuickWindow *>(engine.rootObjects().first());
    if (step.contains("checkCanvasItem")) {
        const auto call = step["checkCanvasItem"].toObject();
        auto *item = findItem(window->contentItem(), "canvas-overlay");
        const QString property = call["property"].toString("kind");
        if (!item || item->property(property.toUtf8()).toString() != call["equals"].toVariant().toString()) {
            qInfo() << "Canvas item pending" << call << (item ? item->property(property.toUtf8()) : QVariant());
            return SmokeResult::Retry;
        }
        return SmokeResult::Done;
    }
    if (step.contains("canvasPointer")) {
        const auto call = step["canvasPointer"].toObject();
        auto *item = findItem(window->contentItem(), call["item"].toString("canvas-overlay"));
        if (!item) {
            qCritical() << "Canvas overlay not shown";
            return SmokeResult::Fail;
        }
        // area E: a point may be a remembered one ("$name")
        const auto points = QJsonValue::fromVariant(substitute(call["points"].toVariant())).toArray();
        const auto at = [item](const QJsonValue &p) {
            return item->mapToScene(QPointF(p.toArray().at(0).toDouble() * item->width(),
                                            p.toArray().at(1).toDouble() * item->height()));
        };
        const auto modifiers = modifiersOf(call["modifiers"].toString());
        if (call.contains("wheel")) {
            const QPointF point = at(points.at(0));
            const int delta = call["wheel"].toObject()["up"].toBool(true) ? 120 : -120;
            QWheelEvent event(point, window->mapToGlobal(point.toPoint()), QPoint(), QPoint(0, delta),
                              Qt::NoButton, modifiers, Qt::NoScrollPhase, false);
            QGuiApplication::sendEvent(window, &event);
        } else {
            const Qt::MouseButton button = call["button"].toString() == "right" ? Qt::RightButton : Qt::LeftButton;
            const auto send = [&](QEvent::Type type, const QPointF &point, Qt::MouseButtons buttons) {
                QMouseEvent event(type, point, window->mapToGlobal(point.toPoint()),
                                  type == QEvent::MouseMove ? Qt::NoButton : button, buttons, modifiers);
                QGuiApplication::sendEvent(window, &event);
            };
            for (int click = 0; click < std::max(1, call["clicks"].toInt(1)); ++click) {
                send(QEvent::MouseButtonPress, at(points.at(0)), button);
                if (click > 0)
                    send(QEvent::MouseButtonDblClick, at(points.at(0)), button);
                for (int i = 1; i < points.size(); ++i) {
                    // Intermediate positions, so drag thresholds and handlers see a movement.
                    const QPointF from = at(points.at(i - 1)), to = at(points.at(i));
                    for (int k = 1; k <= 4; ++k)
                        send(QEvent::MouseMove, from + (to - from) * (k / 4.0), button);
                }
                send(QEvent::MouseButtonRelease, at(points.at(points.size() - 1)), Qt::NoButton);
            }
        }
        return call["wait"].toBool(true) ? SmokeResult::Wait : SmokeResult::Done;
    }
    return SmokeResult::NotHandled;
}
