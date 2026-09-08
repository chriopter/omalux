import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

ColumnLayout {
    id: root
    required property var theme
    required property var values
    required property bool editable
    signal edited(string id, real value)
    property int channel: 4
    width: parent.width
    spacing: 8
    Text { text: "mode"; font: root.theme.textFont; color: root.theme.muted }
    ComboBox {
        wheelEnabled: false
        Layout.fillWidth: true
        model: ["non-local means", "wavelets", "compute variance", "non-local means auto", "wavelets auto"]
        currentIndex: root.values.denoise_mode
        enabled: root.editable
        onActivated: root.edited("denoise_mode",currentIndex)
    }
    Text { text: "color mode"; font: root.theme.textFont; color: root.theme.muted }
    ComboBox {
        wheelEnabled: false
        Layout.fillWidth: true
        model: ["RGB", "Y0U0V0"]
        currentIndex: root.values.denoise_color_mode
        enabled: root.editable
        onActivated: root.edited("denoise_color_mode",currentIndex)
    }
    Text {
        Layout.fillWidth: true
        visible: root.values.denoise_color_mode !== 1
        text: "Select Y0U0V0 to edit luminance and chroma separately."
        wrapMode: Text.WordWrap; font: root.theme.textFont; color: root.theme.muted
    }
    ColumnLayout {
        Layout.fillWidth: true
        visible: root.values.denoise_color_mode === 1 && (root.values.denoise_mode === 1 || root.values.denoise_mode === 4)
        RowLayout {
            Button { text: "Y0"; highlighted: root.channel===4; onClicked: root.channel=4; ToolTip.visible:hovered; ToolTip.text:"Luminance" }
            Button { text: "U0V0"; highlighted: root.channel===5; onClicked: root.channel=5; ToolTip.visible:hovered; ToolTip.text:"Chroma" }
            ToolButton { text: "↺"; enabled: root.editable; onClicked: { for(let b=0;b<7;++b) root.edited("denoise_"+root.channel+"_"+b,.5) } }
        }
        Rectangle {
            Layout.fillWidth: true; implicitHeight: 140
            color: "#181825"; border.color: root.theme.line
            Canvas {
                id: graph
                anchors.fill: parent; anchors.margins: 8
                property var values: root.values
                property int channel: root.channel
                onValuesChanged: requestPaint()
                onChannelChanged: requestPaint()
                onPaint: {
                    const c=getContext("2d");c.clearRect(0,0,width,height)
                    c.strokeStyle=root.theme.line;c.lineWidth=1
                    for(let i=1;i<4;++i) {c.beginPath();c.moveTo(0,height*i/4);c.lineTo(width,height*i/4);c.stroke()}
                    c.strokeStyle=root.channel===4?root.theme.ink:"#dc9960";c.lineWidth=2;c.beginPath()
                    for(let i=0;i<7;++i) {
                        const x=i/6*width,y=(1-root.values["denoise_"+root.channel+"_"+i])*height
                        if(i===0)c.moveTo(x,y);else c.lineTo(x,y)
                    }
                    c.stroke()
                    for(let i=0;i<7;++i) {const x=i/6*width,y=(1-root.values["denoise_"+root.channel+"_"+i])*height;c.fillStyle=root.theme.accent;c.fillRect(x-3,y-3,6,6)}
                }
                MouseArea {
                    anchors.fill: parent; enabled: root.editable; preventStealing: true
                    property int band: 0
                    function edit(y) {root.edited("denoise_"+root.channel+"_"+band,Math.max(0,Math.min(1,1-y/height)))}
                    onPressed: mouse => {band=Math.max(0,Math.min(6,Math.round(mouse.x/width*6)));edit(mouse.y)}
                    onPositionChanged: mouse => {if(pressed)edit(mouse.y)}
                    Accessible.name: root.channel===4?"Y0 wavelet control points":"U0V0 wavelet control points"
                }
            }
        }
        RowLayout {
            Text { text:"coarse";color:root.theme.muted;font:root.theme.textFont }
            Item { Layout.fillWidth:true }
            Text { text:"fine";color:root.theme.muted;font:root.theme.textFont }
        }
    }
}
