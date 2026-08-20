import SwiftUI

struct ContentView: View {
    @State private var player: Player?
    
    var body: some View {
        if let player {
            PlayerView(player: player) { self.player = nil }
        } else {
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
}

#Preview {
    ContentView()
}
