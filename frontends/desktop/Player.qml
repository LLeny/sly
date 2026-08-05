import QtQuick
import QtQuick.Controls
import desktop

Item {
    id: player

    required property url romUrl
    required property string romName

    PlayerCore {
        id: core
        romUrl: player.romUrl
        Component.onCompleted: load()
    }

    FrameAnimation {
        running: core.loaded
        onTriggered: core.runFrame()
    }

    Viewport {
        id: screen

        readonly property int zoom: core.loaded
            ? Math.max(1, Math.min(Math.floor(player.width / core.frameSize.width),
                                   Math.floor(player.height / core.frameSize.height)))
            : 1

        anchors.centerIn: parent
        source: core
        visible: core.loaded
        width: core.frameSize.width * screen.zoom
        height: core.frameSize.height * screen.zoom
    }

    Label {
        anchors.centerIn: parent
        visible: !core.loaded
        horizontalAlignment: Text.AlignHCenter
        text: core.error !== "" ? core.error : player.romName
    }
}
