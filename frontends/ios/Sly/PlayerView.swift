import SwiftUI
import SlyKit

struct PlayerView: View {
    let player: Player
    let onExit: () -> Void

    @AppStorage(AppSettings.imageFilteringKey)
    private var imageFiltering = AppSettings.defaultImageFiltering

    @State private var isMenuPresented = false
    @State private var errorMessage: String?

    var body: some View {
        if let frameInterval = player.core.frameIntervalNanoseconds {
            MetalPlayerView(
                player: player,
                framesPerSecond: Int(
                    (1_000_000_000 / Double(frameInterval)).rounded()
                ),
                isPaused: isMenuPresented,
                imageFiltering: imageFiltering
            )
            .overlay(alignment: .top) {
                HStack {
                    Button(action: onExit) {
                        Label("Back", systemImage: "chevron.left")
                    }

                    Spacer()

                    Button {
                        isMenuPresented = true
                    } label: {
                        Label("Menu", systemImage: "line.3.horizontal")
                    }
                    .popover(isPresented: $isMenuPresented, arrowEdge: .top) {
                        menu
                            .presentationCompactAdaptation(.popover)
                    }
                }
                .buttonStyle(.glass)
                .buttonBorderShape(.capsule)
                .padding()
            }
            .background(.black)
            .alert(
                "Unable to Complete Action",
                isPresented: Binding(
                    get: { errorMessage != nil },
                    set: { if !$0 { errorMessage = nil } }
                )
            ) {
                Button("OK", role: .cancel) {}
            } message: {
                Text(errorMessage ?? "")
            }
        } else {
            ContentUnavailableView(
                "Unable to Play",
                systemImage: "exclamationmark.triangle",
                description: Text("The emulator core has no frame timing.")
            )
        }
    }

    private var menu: some View {
        VStack(alignment: .leading, spacing: 0) {
            menuButton("Reset", systemImage: "arrow.counterclockwise") {
                player.reset()
            }

            Divider()

            menuButton("Quick Save", systemImage: "square.and.arrow.down") {
                try player.saveState()
            }

            menuButton("Quick Load", systemImage: "square.and.arrow.up") {
                try player.loadState()
            }
            .disabled(!player.hasState)
        }
        .buttonStyle(.plain)
        .padding(8)
        .frame(width: 220)
    }

    private func menuButton(
        _ title: String,
        systemImage: String,
        action: @escaping () throws -> Void
    ) -> some View {
        Button {
            isMenuPresented = false
            do {
                try action()
            } catch {
                errorMessage = error.localizedDescription
            }
        } label: {
            Label(title, systemImage: systemImage)
                .frame(maxWidth: .infinity, alignment: .leading)
                .padding(12)
                .contentShape(.rect)
        }
    }
}
