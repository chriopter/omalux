// SPDX-License-Identifier: GPL-3.0-or-later
#include "module_catalog.h"
static QString key(const QString &operation, int instance) {
    return operation + '/' + QString::number(instance);
}
static QString take(char *text) {
    const QString result = QString::fromUtf8(text ? text : "");
    om_engine_free_json(text);
    return result;
}
void ModuleCatalog::reload(OmEngine *engine) {
    entries.clear();
    positions.clear();
    const int count = om_engine_module_count(engine);
    for (int position = 0; position < count; ++position) {
        int instance = 0;
        const char *operation = om_engine_module_identity(engine, position, &instance);
        if (!operation)
            break;
        positions.insert(key(QString::fromUtf8(operation), instance), entries.size());
        entries.append(take(om_engine_module_at(engine, position)));
    }
    changed = true;
}
QString ModuleCatalog::update(OmEngine *engine, const QString &operation, int instance) {
    const int position = positions.value(key(operation, instance), -1);
    if (position < 0)
        return {};
    QString entry = take(om_engine_module_at(engine, position));
    if (entry.isEmpty())
        return {};
    if (entry != entries[position]) {
        entries[position] = entry;
        changed = true;
    }
    return entry;
}
QString ModuleCatalog::json() const {
    return '[' + entries.join(',') + ']';
}
bool ModuleCatalog::takeChanged() {
    const bool result = changed;
    changed = false;
    return result;
}
