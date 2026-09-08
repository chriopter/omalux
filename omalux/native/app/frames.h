// SPDX-License-Identifier: GPL-3.0-or-later
#pragma once
#include <QQuickImageProvider>
#include <QImage>
#include <mutex>
// QML owns providers. Frames always own their pixels independently of darktable.
class Frames : public QQuickImageProvider {
  public:
    Frames();
    QImage requestImage(const QString &, QSize *, const QSize &) override;
    void set(QImage image);
    QImage image();

  private:
    std::mutex mutex;
    QImage frame;
};
QImage copyDisplayPixels(const unsigned char *pixels, int width, int height);
