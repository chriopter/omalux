import QtQuick
import "qrc:/qt/qml/org/omalux/qml" as App

App.Main {
    id: window
    width: 1440
    height: 920

    Component {
        id: captureSurface
        Rectangle {
            width: window.width
            height: window.height
            color: window.pageColor
        }
    }

    // Main calls this only once Qt has loaded the developed photo into its Image.
    // Override the export hook so the capture follows the real preview lifecycle.
    function continueCliExport() {
        settle.restart()
    }

    Timer {
        id: settle
        interval: 300
        onTriggered: {
            // ApplicationWindow's native contentItem cannot grabToImage itself.
            // Keep its actual children and background on a QML capture surface.
            const children = Array.from(window.contentItem.children)
            const surface = captureSurface.createObject(window.contentItem)
            for (const child of children)
                child.parent = surface
            const accepted = surface.grabToImage(function(result) {
                if (!result.saveToFile(window.argumentValue("--capture", ""))) {
                    console.error("Could not save website screenshot")
                    Qt.exit(1)
                    return
                }
                Qt.exit(0)
            })
            if (!accepted) {
                console.error("Could not capture application window")
                Qt.exit(1)
            }
        }
    }

    Timer {
        interval: 45000
        running: true
        onTriggered: {
            console.error("Timed out waiting for the website screenshot")
            Qt.exit(1)
        }
    }
}
