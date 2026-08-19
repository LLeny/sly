import SwiftUI

struct ContentView: View {
    @State private var player: Player?
    
    var body: some View {
        if let player, player.isPlaying {
            PlayerView(player: player)
        } else {
            BrowserView(player: $player)
        }
    }
}

#Preview {
    ContentView()
}
