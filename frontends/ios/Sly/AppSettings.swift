import Foundation

enum AppSettings {
    static let imageFilteringKey = "imageFiltering"
    static let showFileExtensionsKey = "showFileExtensions"
    static let autoLoadQuickSaveKey = "autoLoadQuickSave"

    static let defaultImageFiltering = ImageFiltering.sharpPixels
    static let defaultShowFileExtensions = false
    static let defaultAutoLoadQuickSave = true
}

enum ImageFiltering: String, CaseIterable, Identifiable {
    case sharpPixels
    case smooth

    var id: Self { self }

    var displayName: String {
        switch self {
        case .sharpPixels:
            "Sharp Pixels"
        case .smooth:
            "Smooth"
        }
    }
}
