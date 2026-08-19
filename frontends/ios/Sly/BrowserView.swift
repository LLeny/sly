import SwiftUI
import SlyKit

struct BrowserView: View {
    @Binding var player: Player?
    @State private var roms: [System: [URL]] = [:]

    var body: some View {
        NavigationStack {
            List {
                ForEach(Array(roms.keys), id: \.self) { system in
                    Section(system.displayName) {
                        ForEach(roms[system, default: []], id: \.self) { rom in
                            Button {
                                do {
                                    player = try Player(system: system, rom: rom)
                                } catch {
                                    print("Failed to load ROM:", error)
                                }
                            } label: {
                                Text(rom.deletingPathExtension().lastPathComponent)
                                    .foregroundStyle(.primary)
                            }
                        }
                    }
                }
            }
        }
        .task {
            do {
                try AppFiles.prepare()
                roms = try AppFiles.roms()
            } catch {
                print("Failed to load ROM list:", error)
            }
        }
    }
}
