import SwiftUI
import SlyKit

struct ArtworkCoverView: View {
    let system: System
    let rom: URL

    @State private var cover: Image?
    @State private var isLoading = true

    @AppStorage(AppSettings.artworkRegionKey)
    private var artworkRegion = AppSettings.defaultArtworkRegion

    var body: some View {
        Color.clear
            .aspectRatio(1, contentMode: .fit)
            .overlay {
                if let cover {
                    cover
                        .resizable()
                        .scaledToFit()
                } else if isLoading {
                    ProgressView()
                } else {
                    Image(systemName: "gamecontroller")
                        .font(.title)
                        .foregroundStyle(.secondary)
                }
            }
            .clipped()
            .accessibilityHidden(true)
            .task(id: artworkRegion) {
                cover = nil
                isLoading = true
                do {
                    let loaded = try await ArtworkService.shared.cover(
                        system: system,
                        romURL: rom,
                        preferredRegion: artworkRegion
                    )
                    try Task.checkCancellation()
                    cover = loaded
                        .flatMap { UIImage(data: $0.data) }
                        .map(Image.init(uiImage:))
                    isLoading = false
                } catch is CancellationError {
                    // The row left the screen, ignore..
                } catch {
                    isLoading = false
                    print("Failed to load artwork for \(rom.lastPathComponent):", error)
                }
            }
    }
}
