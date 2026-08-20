import SwiftUI

struct SettingsView: View {
    @AppStorage(AppSettings.imageFilteringKey)
    private var imageFiltering = AppSettings.defaultImageFiltering

    @AppStorage(AppSettings.showFileExtensionsKey)
    private var showFileExtensions = AppSettings.defaultShowFileExtensions

    @AppStorage(AppSettings.autoLoadQuickSaveKey)
    private var autoLoadQuickSave = AppSettings.defaultAutoLoadQuickSave

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

                Section("Library") {
                    Toggle(
                        "Show File Extensions",
                        isOn: $showFileExtensions
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
                        "Automatically resumes a ROM from its quick save " +
                        "when one is available."
                    )
                }
            }
            .navigationTitle("Settings")
        }
    }
}

#Preview {
    SettingsView()
}
