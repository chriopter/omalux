// SPDX-License-Identifier: GPL-3.0-or-later
#include "frames.h"
Frames::Frames() : QQuickImageProvider(QQuickImageProvider::Image) {
}
QImage Frames::requestImage(const QString &, QSize *size, const QSize &) {
    std::lock_guard lock(mutex);
    if (size)
        *size = frame.size();
    return frame;
}
void Frames::set(QImage next) {
    std::lock_guard lock(mutex);
    frame = std::move(next);
}
QImage Frames::image() {
    std::lock_guard lock(mutex);
    return frame;
}
QImage copyDisplayPixels(const unsigned char *pixels, int width, int height) {
    auto copy = QImage(pixels, width, height, width * 4, QImage::Format_RGB32).copy();
    // darktable's BGRx display output leaves x undefined; Qt requires opaque alpha.
    for (int y = 0; y < height; ++y) {
        auto *row = reinterpret_cast<QRgb *>(copy.scanLine(y));
        for (int x = 0; x < width; ++x)
            row[x] |= 0xff000000u;
    }
    return copy;
}
