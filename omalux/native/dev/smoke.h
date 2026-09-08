// SPDX-License-Identifier: GPL-3.0-or-later
#pragma once
class QGuiApplication;
class QQmlApplicationEngine;
class Editor;
class Frames;
void install_smoke(QGuiApplication &, Editor &, Frames *, QQmlApplicationEngine &);
