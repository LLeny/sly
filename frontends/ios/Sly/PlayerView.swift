import SwiftUI
import SlyKit

struct PlayerView: View {
    let player: Player

    var body: some View {
        VStack {
            Text(player.core.cartridge.title)

            Button("Back to ROMs") {
                player.pause()
            }
        }
    }
}
