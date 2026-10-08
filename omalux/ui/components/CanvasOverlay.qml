import QtQuick

// The on-canvas tool of one module, laid over the photo with the photo's size. `tool` says
// which ({ operation, instance, kind }); `overlayJson` is what the engine reports for it
// (engine/canvas.h), `moduleState` the module's catalog entry for tools that work on its
// parameters directly (vignetting). Each kind is a component of its own; this item only
// picks it and passes its toolbar entries, gestures and parameter changes through.
Item {
    id: root
    required property var theme
    property var tool: null
    property string overlayJson: ""
    property var moduleState
    property bool editable: true
    // The viewport's shorter side over the displayed photo's (1 at fit), for liquify's stamp.
    property real viewScale: 1
    signal edited(var gesture)
    signal parametersEdited(var changes)
    signal interactionChanged(bool active)

    readonly property string kind: tool ? tool.kind : ""
    readonly property var overlay: {
        if (!tool || !overlayJson) return ({})
        try {
            const o = JSON.parse(overlayJson)
            return o.operation === tool.operation && o.instance === tool.instance ? o : ({})
        } catch (e) { return ({}) }
    }
    readonly property var tools: loader.item && loader.item.tools ? loader.item.tools : []
    // A drawing tool is active: presses on the photo draw instead of panning it.
    readonly property bool capturing: !!loader.item && !!loader.item.capturing
    function toolClicked(key, modifiers) { if (loader.item) loader.item.toolClicked(key, modifiers || 0) }
    // Escape: leave the drawing tool.
    function cancel() { if (loader.item && loader.item.cancel) loader.item.cancel(); else if (loader.item && "mode" in loader.item) loader.item.mode = "" }

    Loader {
        id: loader
        anchors.fill: parent
        sourceComponent: ({ vignette: vignette, line: line, ashift: ashift, shapes: shapes, liquify: liquify })[root.kind] || null
    }
    Component {
        id: vignette
        VignetteOverlay {
            theme: root.theme
            moduleState: root.moduleState
            editable: root.editable
            onParametersEdited: changes => root.parametersEdited(changes)
            onInteractionChanged: active => root.interactionChanged(active)
        }
    }
    Component {
        id: line
        GradientLineOverlay {
            theme: root.theme
            overlay: root.overlay
            editable: root.editable
            onEdited: gesture => root.edited(gesture)
            onInteractionChanged: active => root.interactionChanged(active)
        }
    }
    Component {
        id: ashift
        StructureOverlay {
            theme: root.theme
            overlay: root.overlay
            editable: root.editable
            onEdited: gesture => root.edited(gesture)
            onInteractionChanged: active => root.interactionChanged(active)
        }
    }
    Component {
        id: shapes
        ShapesOverlay {
            theme: root.theme
            overlay: root.overlay
            editable: root.editable
            onEdited: gesture => root.edited(gesture)
            onInteractionChanged: active => root.interactionChanged(active)
        }
    }
    Component {
        id: liquify
        LiquifyOverlay {
            theme: root.theme
            overlay: root.overlay
            editable: root.editable
            viewScale: root.viewScale
            onEdited: gesture => root.edited(gesture)
            onInteractionChanged: active => root.interactionChanged(active)
        }
    }
}
