// SPDX-License-Identifier: GPL-3.0-or-later
#pragma once
#include <QString>
#include <vector>
struct StyleFile {
    QString id, path, name, description, error, previewUrl;
};
std::vector<StyleFile> discoverStyles(const QString &directory);
