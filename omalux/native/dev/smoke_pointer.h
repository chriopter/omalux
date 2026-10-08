// SPDX-License-Identifier: GPL-3.0-or-later
#pragma once
#include "smoke_canvas.h"
#include <memory>
class QJsonObject;
class QQmlApplicationEngine;
// Smoke steps with genuine pointer input on sidebar and toolbar items (smoke_pointer.cpp).
// `busy` stays true while a gesture is still being delivered; the driver waits for it.
SmokeResult pointerSmokeStep(const QJsonObject &step, QQmlApplicationEngine &engine, std::shared_ptr<bool> busy);
