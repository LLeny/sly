import SwiftUI
import SlyKit

struct PlayerView: View {
    let player: Player

    var body: some View {
        if let frameInterval = player.core.frameIntervalNanoseconds {
            VStack(spacing: 0) {
                MetalPlayerView(
                    player: player,
                    framesPerSecond: Int(
                        (1_000_000_000 / Double(frameInterval)).rounded()
                    )
                )

                Button("Back to ROMs") {
                    player.pause()
                }
                .padding()
            }
            .background(.black)
        } else {
            ContentUnavailableView(
                "Unable to Play",
                systemImage: "exclamationmark.triangle",
                description: Text("The emulator core has no frame timing.")
            )
        }
    }
}
