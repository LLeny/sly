import SwiftUI

struct ContentView: View {
    @State private var player: Player?
    
    var body: some View {
        if let player {
            PlayerView(player: player) { self.player = nil }
        } else {
            BrowserView(player: $player)
        }
    }
}

#Preview {
    ContentView()
}
