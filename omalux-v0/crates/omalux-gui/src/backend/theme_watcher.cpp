#include "theme_watcher.h"

#include "omalux-gui/src/backend/mod.cxxqt.h"

#include <QDir>
#include <QFile>
#include <QFileInfo>
#include <QFileSystemWatcher>
#include <QStringList>
#include <QTimer>

namespace {

constexpr auto watcherName = "omalux-theme-watcher";
constexpr auto debounceTimerName = "omalux-theme-watcher-debounce";
constexpr int debounceMilliseconds = 50;

void rearmThemePaths(QFileSystemWatcher& watcher)
{
    const QStringList watched = watcher.files() + watcher.directories();
    if (!watched.isEmpty())
        watcher.removePaths(watched);

    const QString currentDirectory =
        QDir::homePath() + QStringLiteral("/.local/state/omarchy/current");
    const QString omarchyState = QFileInfo(currentDirectory).absolutePath();
    const QString themeDirectory = currentDirectory + QStringLiteral("/theme");
    const QString colorsPath = themeDirectory + QStringLiteral("/colors.toml");

    if (QDir(omarchyState).exists())
        watcher.addPath(omarchyState);
    if (QDir(currentDirectory).exists())
        watcher.addPath(currentDirectory);
    if (QDir(themeDirectory).exists())
        watcher.addPath(themeDirectory);
    if (QFile::exists(colorsPath))
        watcher.addPath(colorsPath);
}

} // namespace

void installThemeWatcher(PhotoBackend& backend)
{
    auto* watcher = backend.findChild<QFileSystemWatcher*>(
        QString::fromLatin1(watcherName), Qt::FindDirectChildrenOnly);
    if (watcher) {
        rearmThemePaths(*watcher);
        return;
    }

    watcher = new QFileSystemWatcher(&backend);
    watcher->setObjectName(QString::fromLatin1(watcherName));
    auto* debounceTimer = new QTimer(&backend);
    debounceTimer->setObjectName(QString::fromLatin1(debounceTimerName));
    debounceTimer->setSingleShot(true);
    debounceTimer->setInterval(debounceMilliseconds);
    auto* backendPointer = &backend;
    const auto scheduleReload = [debounceTimer]() { debounceTimer->start(); };
    QObject::connect(
        watcher,
        &QFileSystemWatcher::fileChanged,
        &backend,
        scheduleReload);
    QObject::connect(
        watcher,
        &QFileSystemWatcher::directoryChanged,
        &backend,
        scheduleReload);
    QObject::connect(
        debounceTimer,
        &QTimer::timeout,
        &backend,
        [watcher, backendPointer]() {
            backendPointer->reloadTheme();
            rearmThemePaths(*watcher);
        });
    rearmThemePaths(*watcher);
}
