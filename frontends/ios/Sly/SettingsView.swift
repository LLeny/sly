import SlyKit
import SwiftUI

struct SettingsView: View {
    @AppStorage(AppSettings.imageFilteringKey)
    private var imageFiltering = AppSettings.defaultImageFiltering

    @AppStorage(AppSettings.showFileExtensionsKey)
    private var showFileExtensions = AppSettings.defaultShowFileExtensions

    @AppStorage(AppSettings.autoLoadQuickSaveKey)
    private var autoLoadQuickSave = AppSettings.defaultAutoLoadQuickSave

    @AppStorage(AppSettings.artworkRegionKey)
    private var artworkRegion = AppSettings.defaultArtworkRegion

    @AppStorage(AppSettings.artworkAPIKeyKey)
    private var artworkAPIKey = ""

    @State private var apiKeyDraft = ""
    @State private var revealKey = false

    private static let accountURL = URL(string: "https://art.layle.dev/account")!

    var body: some View {
        NavigationStack {
            Form {
                Section("Video") {
                    VStack(alignment: .leading, spacing: 12) {
                        Text("Image Filtering")

                        Picker(
                            "Image Filtering",
                            selection: $imageFiltering
                        ) {
                            ForEach(ImageFiltering.allCases) { filtering in
                                Text(filtering.displayName).tag(filtering)
                            }
                        }
                        .labelsHidden()
                        .pickerStyle(.segmented)
                    }
                    .padding(.vertical, 4)
                }

                Section {
                    Picker("Preferred Cover Region", selection: $artworkRegion) {
                        ForEach(ArtworkRegion.allCases) { region in
                            Text(region.displayName).tag(region)
                        }
                    }

                    Toggle(
                        "Show File Extensions",
                        isOn: $showFileExtensions
                    )
                } header: {
                    Text("Library")
                } footer: {
                    Text(
                        "Automatic uses the ROMs detected region and falls back to another available cover when necessary."
                    )
                }

                Section {
                    Toggle(
                        "Auto-load Quick Save",
                        isOn: $autoLoadQuickSave
                    )
                } header: {
                    Text("Save States")
                } footer: {
                    Text(
                        "Automatically resumes a ROM from its quick save when one is available."
                    )
                }

                Section("Artwork") {
                    HStack {
                        Group {
                            if revealKey {
                                TextField("Anonymous", text: $apiKeyDraft)
                            } else {
                                SecureField("Anonymous", text: $apiKeyDraft)
                            }
                        }
                        .textInputAutocapitalization(.never)
                        .autocorrectionDisabled()

                        Button {
                            revealKey.toggle()
                        } label: {
                            Image(systemName: revealKey ? "eye.slash" : "eye")
                        }
                    }

                    HStack {
                        Button("Save") {
                            artworkAPIKey = apiKeyDraft.trimmingCharacters(in: .whitespaces)
                        }

                        Button("Clear") {
                            apiKeyDraft = ""
                        }

                        Spacer()

                        Link("Get Key", destination: Self.accountURL)
                    }
                }
                .buttonStyle(.borderless)
            }
            .navigationTitle("Settings")
            .onAppear {
                apiKeyDraft = artworkAPIKey
            }
        }
    }
}

#Preview {
    SettingsView()
}
