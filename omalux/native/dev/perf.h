// SPDX-License-Identifier: GPL-3.0-or-later
#pragma once
class QGuiApplication;
class QQmlApplicationEngine;
class Editor;
// Interface timing driver (OMALUX_PERF_SCRIPT), see perf.cpp.
void install_perf(QGuiApplication &, Editor &, QQmlApplicationEngine &);
