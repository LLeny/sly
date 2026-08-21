import Foundation
import SlyKit

enum AppSettings {
    static let imageFilteringKey = "imageFiltering"
    static let showFileExtensionsKey = "showFileExtensions"
    static let autoLoadQuickSaveKey = "autoLoadQuickSave"
    static let artworkRegionKey = "artworkRegion"

    static let defaultImageFiltering = ImageFiltering.sharpPixels
    static let defaultShowFileExtensions = false
    static let defaultAutoLoadQuickSave = true
    static let defaultArtworkRegion = ArtworkRegion.automatic

    static func displayName(for rom: URL, showingFileExtensions: Bool) -> String {
        showingFileExtensions
            ? rom.lastPathComponent
            : rom.deletingPathExtension().lastPathComponent
    }
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
