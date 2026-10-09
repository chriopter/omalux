// SPDX-License-Identifier: GPL-3.0-or-later
#include "app/editor.h"
#include "app/frames.h"
#include "dev/development_tools.h"
#include <QGuiApplication>
#include <QQmlApplicationEngine>
#include <QQmlContext>
#include <QQuickStyle>
#include <QDebug>
#include <QFileInfo>
int main(int argc, char **argv) {
    // Basic, with the menus themed (omalux/ui/style/Omalux).
    QQuickStyle::setStyle("Omalux");
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
    auto *beforeFrames = new Frames;
    std::vector<QByteArray> dtargs;
    for (int i = 3; i < args.size(); ++i)
        dtargs.push_back(args[i].toUtf8());
    // Editor joins its worker before QML destroys the image providers.
    QQmlApplicationEngine engine;
    engine.addImportPath(QFileInfo(QStringLiteral(OMALUX_QML)).absolutePath() + QStringLiteral("/style"));
    Editor editor(frames, hoverFrames, beforeFrames, args[1], std::move(dtargs));
    engine.addImageProvider("preview", frames);
    engine.addImageProvider("hover", hoverFrames);
    engine.addImageProvider("before", beforeFrames);
    engine.rootContext()->setContextProperty("editor", &editor);
    engine.rootContext()->setContextProperty("assetsRoot", QUrl::fromLocalFile(args[2] + "/"));
    // The example photograph ships with the application like its icons and logo: in the assets
    // folder the launcher or package passes. OMALUX_EXAMPLE_PHOTO names another file; a build
    // run from the source tree without that folder falls back to the repository's own copy.
    QString example;
    for (const auto &candidate :
         {qEnvironmentVariable("OMALUX_EXAMPLE_PHOTO"), args[2] + "/images/beach-volleyball.jpg",
          QFileInfo(QStringLiteral(OMALUX_QML)).absolutePath() + "/../../assets/images/beach-volleyball.jpg"})
        if (example.isEmpty() && !candidate.isEmpty() && QFileInfo(candidate).isFile())
            example = QUrl::fromLocalFile(QFileInfo(candidate).canonicalFilePath()).toString();
    engine.rootContext()->setContextProperty("examplePhoto", example);
    engine.load(QUrl::fromLocalFile(QStringLiteral(OMALUX_QML)));
    if (engine.rootObjects().isEmpty())
        return 1;
    installDevelopmentTools(app, editor, frames, engine);
    const int result = app.exec();
    for (auto *root : engine.rootObjects())
        delete root;
    return result;
}
