import SwiftUI

struct ContentView: View {
    @State private var player: Player?

    var body: some View {
        TabView {
            Tab("Games", systemImage: "gamecontroller") {
                BrowserView(player: $player)
            }

            Tab("Settings", systemImage: "gearshape") {
                SettingsView()
            }
        }
    }
}

#Preview {
    ContentView()
}
