import QtQuick
import QtQuick.Layouts
import QtQuick.Controls
import Qt.labs.folderlistmodel

ApplicationWindow {
    id: window
    required property url romsRoot

    width: 720
    height: 520
    minimumWidth: 320
    minimumHeight: 260
    visible: true
    title: qsTr("Sly")

    // roms/<platform>/
    FolderListModel {
        id: platforms
        folder: window.romsRoot
        showFiles: false
        showDotAndDotDot: false
        sortField: FolderListModel.Name
    }

    FolderListModel {
        id: roms
        folder: platformBox.currentIndex >= 0
                ? platforms.get(platformBox.currentIndex, "fileUrl")
                : ""
        showDirs: false
        showDotAndDotDot: false
        sortField: FolderListModel.Name
        nameFilters: ["*.sfc", "*.smc"]
    }

    function launch(romUrl, romName) {
        stack.push(player, { romUrl: romUrl, romName: romName })
    }

    function formatSize(bytes) {
        if (bytes >= 1024 * 1024)
            return (bytes / (1024 * 1024)).toFixed(1) + " MiB"
        if (bytes >= 1024)
            return Math.round(bytes / 1024) + " KiB"
        return bytes + " B"
    }

    readonly property bool browsing: stack.depth <= 1

    header: ToolBar {
        RowLayout {
            anchors.fill: parent
            anchors.leftMargin: 8
            anchors.rightMargin: 8
            spacing: 8

            ToolButton {
                text: qsTr("Back")
                visible: !window.browsing
                onClicked: stack.pop()
            }

            Label {
                text: qsTr("Platform")
                visible: window.browsing
            }

            ComboBox {
                id: platformBox
                model: platforms
                textRole: "fileName"
                currentIndex: platforms.count > 0 ? 0 : -1
                visible: window.browsing
                Layout.preferredWidth: 180
            }

            Item { Layout.fillWidth: true }

            Label {
                visible: window.browsing
                text: roms.count === 1 ? qsTr("1 ROM") : qsTr("%1 ROMs").arg(roms.count)
            }
        }
    }

    StackView {
        id: stack
        anchors.fill: parent
        initialItem: browser
    }

    Component {
        id: player
        Player {}
    }

    Component {
        id: browser

        ListView {
            id: list
            model: roms
            clip: true
            focus: true
            currentIndex: 0
            ScrollBar.vertical: ScrollBar {}

            delegate: ItemDelegate {
                id: row
                required property int index
                required property string fileName
                required property real fileSize
                required property url fileUrl

                width: ListView.view.width
                text: row.fileName
                highlighted: ListView.isCurrentItem
                onClicked: list.currentIndex = row.index
                onDoubleClicked: window.launch(row.fileUrl, row.fileName)

                Label {
                    anchors.right: parent.right
                    anchors.rightMargin: 12
                    anchors.verticalCenter: parent.verticalCenter
                    text: window.formatSize(row.fileSize)
                    opacity: 0.6
                }
            }

            Label {
                anchors.centerIn: parent
                visible: roms.count === 0
                horizontalAlignment: Text.AlignHCenter
                opacity: 0.6
                text: platforms.count === 0
                      ? qsTr("No platform folders in\n%1").arg(window.romsRoot)
                      : qsTr("No ROMs in %1").arg(platformBox.currentText)
            }
        }
    }
}
