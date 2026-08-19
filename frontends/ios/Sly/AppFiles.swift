import Foundation
import SlyKit

struct AppFiles {
    static var romsFolder: URL {
        FileManager.default
            .urls(for: .documentDirectory, in: .userDomainMask)[0]
            .appendingPathComponent("roms", isDirectory: true)
    }
    
    static var extensionsBySystem: [System: [String]] = [
        .snes: ["sfc", "smc"],
    ]
    
    static func prepare() throws {
        try FileManager.default.createDirectory(
            at: romsFolder,
            withIntermediateDirectories: true
        )
    }

    static func roms() throws -> [System: [URL]] {
        guard let files = FileManager.default.enumerator(
            at: romsFolder,
            includingPropertiesForKeys: nil,
            options: [.skipsHiddenFiles]
        ) else {
            return [:]
        }

        var result: [System: [URL]] = [:]
        
        for case let url as URL in files {
            guard let system = extensionsBySystem.first(
                where: { $0.value.contains(url.pathExtension.lowercased()) }
            )?.key else {
                continue
            }
            
            result[system, default: []].append(url)
        }
        
        return result.mapValues { files in
            files.sorted {
                $0.lastPathComponent.localizedStandardCompare(
                    $1.lastPathComponent
                ) == .orderedAscending
            }
        }
    }
}
