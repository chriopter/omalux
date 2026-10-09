// SPDX-License-Identifier: GPL-3.0-or-later
#pragma once
#include <QString>
#include <vector>
struct StyleFile {
    QString id, path, name, description, error, previewUrl;
    // Display name of the look's family when a folder above it carries a family.json
    // ({"version": 1, "name": "DHH", "order": 1}); empty when the folder names say it all.
    QString family;
    int familyOrder = 0;
};
std::vector<StyleFile> discoverStyles(const QString &directory);
