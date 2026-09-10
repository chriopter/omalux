// SPDX-License-Identifier: GPL-3.0-or-later
#include "app/editor.h"
#include "app/frames.h"
#include "dev/development_tools.h"
#include <QGuiApplication>
#include <QQmlApplicationEngine>
#include <QQmlContext>
#include <QQuickStyle>
#include <QDebug>
int main(int argc, char **argv) {
    QQuickStyle::setStyle("Basic");
    QGuiApplication app(argc, argv);
    QCoreApplication::setOrganizationName("Omalux");
    QCoreApplication::setOrganizationDomain("omalux.org");
    const auto args = app.arguments();
    if (args.size() < 4) {
        qCritical() << "Use development/start [image]";
        return 1;
    }
    auto *frames = new Frames;
    auto *hoverFrames = new Frames;
    std::vector<QByteArray> dtargs;
    for (int i = 3; i < args.size(); ++i)
        dtargs.push_back(args[i].toUtf8());
    // Editor joins its worker before QML destroys the image providers.
    QQmlApplicationEngine engine;
    Editor editor(frames, hoverFrames, args[1], std::move(dtargs));
    engine.addImageProvider("preview", frames);
    engine.addImageProvider("hover", hoverFrames);
    engine.rootContext()->setContextProperty("editor", &editor);
    engine.rootContext()->setContextProperty("assetsRoot", QUrl::fromLocalFile(args[2] + "/"));
    engine.load(QUrl::fromLocalFile(QStringLiteral(OMALUX_QML)));
    if (engine.rootObjects().isEmpty())
        return 1;
    installDevelopmentTools(app, editor, frames, engine);
    const int result = app.exec();
    for (auto *root : engine.rootObjects())
        delete root;
    return result;
}
