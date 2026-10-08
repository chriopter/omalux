// SPDX-License-Identifier: GPL-3.0-or-later
#pragma once
class QJsonObject;
class QQmlApplicationEngine;
class Editor;
// Smoke steps of the on-canvas tools (smoke_canvas.cpp).
enum class SmokeResult { NotHandled, Done, Wait, Retry, Fail };
SmokeResult canvasSmokeStep(const QJsonObject &step, Editor &editor, QQmlApplicationEngine &engine);
