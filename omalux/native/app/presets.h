// SPDX-License-Identifier: GPL-3.0-or-later
#pragma once
#include <QString>
#include <vector>
struct PresetFile {
    QString id, path, name, description, error, previewUrl;
};
std::vector<PresetFile> discoverPresets(const QString &directory);
