// SPDX-License-Identifier: GPL-3.0-or-later
#pragma once
#include <QKeyEvent>
#include <QWheelEvent>
#include <QQuickItem>
#include <QKeySequence>
// Optional deterministic integration driver; inactive during normal use.
static void install_smoke(QGuiApplication &app, Editor &editor, Frames *frames, QQmlApplicationEngine &engine) {
    const auto path=qEnvironmentVariable("OMALUX_SMOKE_SCRIPT"); if(path.isEmpty()) return;
    QFile file(path); if(!file.open(QIODevice::ReadOnly)) {app.exit(2);return;}
    const auto steps=QJsonDocument::fromJson(file.readAll()).array();
    auto index=std::make_shared<int>(0);auto previous=std::make_shared<QString>();auto waiting=std::make_shared<bool>(false);
    auto historyMarks=std::make_shared<QVariantMap>();
    auto *timer=new QTimer(&app);timer->setInterval(150);
    QObject::connect(timer,&QTimer::timeout,&app,[&,frames,steps,index,previous,waiting,timer,historyMarks] {
        if(!editor.presetError().isEmpty()) {qCritical()<<editor.presetError();app.exit(2);return;}
        if(!editor.presetsReady() || editor.preview().isEmpty() || editor.styleBusy()) return;
        if(*waiting && editor.preview()==*previous) return;
        *waiting=false;
        if(*index>=steps.size()) {qInfo()<<"Smoke complete";timer->stop();QTimer::singleShot(qEnvironmentVariableIntValue("OMALUX_SMOKE_SETTLE"),&app,[&app]{app.quit();});return;}
        const auto step=steps[(*index)++].toObject();
        qInfo()<<"Smoke step"<<*index<<step;
        *previous=editor.preview();
        if(step.contains("control")) {
            const auto id=step["control"].toString();const auto value=step["value"].toDouble();
            if(editor.controlValues()[id].toDouble()!=value) {editor.setControl(id,value);*waiting=true;}
        } else if(step.contains("controls")) {editor.setControls(step["controls"].toObject().toVariantMap());*waiting=true;}
        else if(step.contains("halation")) {editor.applyHalation();*waiting=true;}
        else if(step.contains("preset")) {editor.applyPreset(step["preset"].toString());*waiting=true;}
        else if(step.contains("open")) {editor.openPhoto(QUrl::fromLocalFile(step["open"].toString()));*waiting=true;}
        else if(step.contains("savePreset")) {editor.savePreset(step["savePreset"].toString());*waiting=true;}
        else if(step.contains("applyNamed")) {
            for(const auto &p:editor.presets()) if(p.toMap()["name"]==step["applyNamed"].toVariant()) {editor.applyPreset(p.toMap()["id"].toString());*waiting=true;break;}
            if(!*waiting) {qCritical()<<"Saved preset missing";app.exit(2);}
        }
        else if(step.contains("geometry")) {
            auto *panel=engine.rootObjects().first()->findChild<QObject*>("geometryPanel");
            if(!panel) {app.exit(2);return;}
            if(step.contains("ratio")) panel->setProperty("aspectRatio",step["ratio"].toDouble());
            if(!QMetaObject::invokeMethod(panel,step["geometry"].toString().toUtf8().constData())) {app.exit(2);return;}
        }
        else if(step.contains("exportNamed") || step.contains("deleteNamed")) {
            const auto name=step.contains("exportNamed")?step["exportNamed"]:step["deleteNamed"];
            bool found=false;
            for(const auto &p:editor.presets()) if(p.toMap()["name"]==name.toVariant()) {
                const auto id=p.toMap()["id"].toString();found=true;
                if(step.contains("exportNamed")) editor.exportPreset(id,QUrl::fromLocalFile(step["destination"].toString()));
                else {editor.deletePreset(id);*waiting=true;}
                break;
            }
            if(!found) {qCritical()<<"Preset missing";app.exit(2);return;}
        }
        else if(step.contains("export")) {editor.exportPhoto(QUrl::fromLocalFile(step["export"].toString()),90);*waiting=true;}
        else if(step.contains("capture")) {
            auto *window=qobject_cast<QQuickWindow*>(engine.rootObjects().first());
            window->grabWindow().save(step["capture"].toString());frames->image().save(step["capture"].toString()+".preview.png");
        } else if(step.contains("reveal")) QMetaObject::invokeMethod(engine.rootObjects().first(),"revealControl",Q_ARG(QVariant,step["reveal"].toVariant()));
        else if(step.contains("panel")) engine.rootObjects().first()->setProperty("selectedPanel",step["panel"].toInt());
        else if(step.contains("rememberHistory")) {
            for(const auto &row:editor.history()) if(row.toMap()["current"].toBool())
                (*historyMarks)[step["rememberHistory"].toString()]=row.toMap()["step"];
        }
        else if(step.contains("selectHistory") || step.contains("clickHistory")) {
            const auto name=step[step.contains("clickHistory") ? "clickHistory" : "selectHistory"].toString();
            if(!historyMarks->contains(name)) {app.exit(2);return;}
            const int position=(*historyMarks)[name].toInt();
            if(step.contains("clickHistory")) {
                const auto target="history-step-"+QString::number(position);
                const auto findItem=[&](auto &&self,QQuickItem *item)->QQuickItem* {
                    if(item->objectName()==target) return item;
                    for(auto *child:item->childItems()) if(auto *found=self(self,child)) return found;
                    return nullptr;
                };
                auto *entry=findItem(findItem,qobject_cast<QQuickWindow*>(engine.rootObjects().first())->contentItem());
                if(!entry || !QMetaObject::invokeMethod(entry,"clicked")) {qCritical()<<"History row unavailable";app.exit(2);return;}
            } else editor.selectHistory(position);
            *waiting=true;
        }
        else if(step.contains("wheelSidebar") || step.contains("pixelWheelSidebar")) {
            auto *window=qobject_cast<QQuickWindow*>(engine.rootObjects().first());
            const QPointF point(window->width()-100,step["y"].toDouble(250));
            QWheelEvent event(point,window->mapToGlobal(point.toPoint()),QPoint(0,step["pixelWheelSidebar"].toInt()),QPoint(0,step["wheelSidebar"].toInt()),Qt::NoButton,Qt::NoModifier,static_cast<Qt::ScrollPhase>(step["phase"].toInt()),false);
            auto *panel=engine.rootObjects().first()->findChild<QObject*>("filtersScroll");
            auto *flick=panel ? panel->property("contentItem").value<QObject*>() : nullptr;
            if(flick) qInfo()<<"Scroll before"<<flick->property("contentY");
            QGuiApplication::sendEvent(window,&event);
            if(flick) qInfo()<<"Scroll immediate"<<flick->property("contentY");
        }
        else if(step.contains("checkScrolled")) {
            auto *panel=engine.rootObjects().first()->findChild<QObject*>("filtersScroll");
            auto *flick=panel ? panel->property("contentItem").value<QObject*>() : nullptr;
            if(flick) qInfo()<<"Scroll settled"<<flick->property("contentY");
            if(!flick || flick->property("contentY").toDouble()<step["checkScrolled"].toDouble(1)
                || (step.contains("maximum") && flick->property("contentY").toDouble()>step["maximum"].toDouble())) {qCritical()<<"Sidebar did not scroll";app.exit(2);return;}
        }
        else if(step.contains("key")) {
            auto *window=qobject_cast<QQuickWindow*>(engine.rootObjects().first());
            const auto key=QKeySequence(step["key"].toString())[0];
            QKeyEvent press(QEvent::KeyPress,key.key(),key.keyboardModifiers());
            QKeyEvent release(QEvent::KeyRelease,key.key(),key.keyboardModifiers());
            QGuiApplication::sendEvent(window,&press);
            QGuiApplication::sendEvent(window,&release);
        }
        else if(step.contains("checkPanel")) {
            if(engine.rootObjects().first()->property("selectedPanel").toInt()!=step["checkPanel"].toInt()) {
                qCritical()<<"Unexpected sidebar panel";app.exit(2);return;
            }
        }
        else if(step.contains("historyContains") || step.contains("historyAbsent")) {
            const bool expected=step.contains("historyContains");
            const auto operation=step[expected ? "historyContains" : "historyAbsent"].toString();
            bool found=false;
            for(const auto &row:editor.history())
                if(row.toMap()["operation"].toString()==operation) found=true;
            if(found!=expected) {qCritical()<<"Unexpected history contents"<<step<<editor.history();app.exit(2);return;}
            int current=0;
            for(const auto &row:editor.history()) if(row.toMap()["current"].toBool()) ++current;
            if(current!=1) {qCritical()<<"Expected one current history row";app.exit(2);return;}
        }
        else if(step.contains("check")) {
            const auto expected=step["check"].toObject();
            for(auto it=expected.begin();it!=expected.end();++it)
                if(std::abs(editor.controlValues()[it.key()].toDouble()-it.value().toDouble())>.01) {qCritical()<<"Unexpected control"<<it.key()<<editor.controlValues()[it.key()];app.exit(2);return;}
        }
    });
    timer->start();QTimer::singleShot(180000,&app,[&app]{qCritical()<<"Smoke timeout";app.exit(2);});
}
