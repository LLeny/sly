import SwiftUI
import SlyKit

struct BrowserView: View {
    @Binding var player: Player?
    @State private var roms: [System: [URL]] = [:]
    @State private var selectedPlatform = LibraryPlatform.snes
    @State private var searchText = ""

    private let columns = [
        GridItem(
            .adaptive(minimum: 96, maximum: 120),
            spacing: 12,
            alignment: .top
        ),
    ]

    @AppStorage(AppSettings.showFileExtensionsKey)
    private var showFileExtensions = AppSettings.defaultShowFileExtensions

    @AppStorage(AppSettings.autoLoadQuickSaveKey)
    private var autoLoadQuickSave = AppSettings.defaultAutoLoadQuickSave

    var body: some View {
        NavigationStack {
            ScrollView {
                if let system = selectedPlatform.system {
                    let visibleRoms = filteredRoms(for: system)
                    if visibleRoms.isEmpty {
                        placeholder { emptyLibrary }
                    } else {
                        LazyVGrid(
                            columns: columns,
                            alignment: .leading,
                            spacing: 16
                        ) {
                            ForEach(visibleRoms, id: \.self) { rom in
                                Button {
                                    play(system: system, rom: rom)
                                } label: {
                                    VStack(alignment: .leading, spacing: 8) {
                                        ArtworkCoverView(system: system, rom: rom)
                                        Text(displayName(for: rom))
                                            .font(.subheadline.weight(.medium))
                                            .foregroundStyle(.primary)
                                            .lineLimit(2)
                                            .multilineTextAlignment(.leading)
                                            .frame(
                                                height: 40,
                                                alignment: .topLeading
                                            )
                                    }
                                    .frame(
                                        maxWidth: .infinity,
                                        alignment: .leading
                                    )
                                    .contentShape(Rectangle())
                                }
                                .buttonStyle(.plain)
                            }
                        }
                        .padding(16)
                    }
                } else {
                    placeholder {
                        ContentUnavailableView(
                            "GBA Core Not Available",
                            systemImage: "gamecontroller",
                            description: Text(
                                "Game Boy Advance support has not been added yet."
                            )
                        )
                    }
                }
            }
            .refreshable {
                loadRoms()
            }
            .navigationTitle("Games")
            .searchable(
                text: $searchText,
                prompt: "Search Games"
            )
            .toolbar {
                ToolbarItem(placement: .topBarTrailing) {
                    Menu {
                        Picker("Core", selection: $selectedPlatform) {
                            ForEach(LibraryPlatform.allCases) { platform in
                                Text(platform.shortName).tag(platform)
                            }
                        }
                    } label: {
                        Label(selectedPlatform.shortName, systemImage: "cpu")
                    }
                    .accessibilityLabel("Core")
                }
            }
            .fullScreenCover(item: $player) { player in
                NavigationStack {
                    PlayerView(player: player)
                        .toolbar {
                            ToolbarItem(placement: .topBarLeading) {
                                Button {
                                    self.player = nil
                                } label: {
                                    Label("Back", systemImage: "chevron.down")
                                }
                            }
                        }
                }
            }
        }
        .task {
            loadRoms()
        }
    }

    private func play(system: System, rom: URL) {
        do {
            let newPlayer = try Player(system: system, rom: rom)
            if autoLoadQuickSave {
                do {
                    try newPlayer.loadState()
                } catch {
                    print("Failed to auto-load quick save:", error)
                }
            }
            player = newPlayer
        } catch {
            print("Failed to load ROM:", error)
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
        AppSettings.displayName(for: rom, showingFileExtensions: showFileExtensions)
    }

    private func filteredRoms(for system: System) -> [URL] {
        let systemRoms = roms[system, default: []]
        guard !searchText.isEmpty else { return systemRoms }
        return systemRoms.filter {
            displayName(for: $0).localizedCaseInsensitiveContains(searchText)
        }
    }

    @ViewBuilder
    private var emptyLibrary: some View {
        if searchText.isEmpty {
            ContentUnavailableView(
                "No Games",
                systemImage: "gamecontroller",
                description: Text("Add ROMs to the Sly \"roms\" folder.")
            )
        } else {
            ContentUnavailableView.search(text: searchText)
        }
    }

    private func placeholder(@ViewBuilder _ content: () -> some View) -> some View {
        content()
            .frame(maxWidth: .infinity)
            .padding(.top, 80)
    }
}

private enum LibraryPlatform: String, CaseIterable, Identifiable {
    case snes
    case gba

    var id: Self { self }

    var shortName: String {
        switch self {
        case .snes: "SNES"
        case .gba: "GBA"
        }
    }

    var system: System? {
        switch self {
        case .snes: .snes
        case .gba: nil
        }
    }
}
