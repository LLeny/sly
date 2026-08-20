import SwiftUI
import SlyKit

struct BrowserView: View {
    @Binding var player: Player?
    @State private var roms: [System: [URL]] = [:]

    @AppStorage(AppSettings.showFileExtensionsKey)
    private var showFileExtensions = AppSettings.defaultShowFileExtensions

    @AppStorage(AppSettings.autoLoadQuickSaveKey)
    private var autoLoadQuickSave = AppSettings.defaultAutoLoadQuickSave

    var body: some View {
        NavigationStack {
            List {
                ForEach(Array(roms.keys), id: \.self) { system in
                    Section(system.displayName) {
                        ForEach(roms[system, default: []], id: \.self) { rom in
                            Button {
                                do {
                                    let newPlayer = try Player(
                                        system: system,
                                        rom: rom
                                    )
                                    if autoLoadQuickSave {
                                        do {
                                            try newPlayer.loadState()
                                        } catch {
                                            print(
                                                "Failed to auto-load quick save:",
                                                error
                                            )
                                        }
                                    }
                                    player = newPlayer
                                } catch {
                                    print("Failed to load ROM:", error)
                                }
                            } label: {
                                Text(displayName(for: rom))
                                    .foregroundStyle(.primary)
                            }
                        }
                    }
                }
            }
            .refreshable {
                loadRoms()
            }
            .navigationTitle("Games")
        }
        .task {
            loadRoms()
        }
    }

    private func loadRoms() {
        do {
            try AppFiles.prepare()
            roms = try AppFiles.roms()
        } catch {
            print("Failed to load ROM list:", error)
        }
    }

    private func displayName(for rom: URL) -> String {
        if showFileExtensions {
            rom.lastPathComponent
        } else {
            rom.deletingPathExtension().lastPathComponent
        }
    }
}
