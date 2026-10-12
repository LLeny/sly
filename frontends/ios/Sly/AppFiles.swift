import Foundation
import SlyKit

struct AppFiles {
    private static var documentsFolder: URL {
        FileManager.default
            .urls(for: .documentDirectory, in: .userDomainMask)[0]
    }

    static var romsFolder: URL {
        documentsFolder
            .appendingPathComponent("roms", isDirectory: true)
    }

    static func prepare() throws {
        let systemFolders = System.all.map {
            romsFolder.appendingPathComponent($0.slug, isDirectory: true)
        }
        for folder in [romsFolder] + systemFolders {
            try FileManager.default.createDirectory(
                at: folder,
                withIntermediateDirectories: true
            )
        }
    }

    static func roms() throws -> [System: [URL]] {
        var result: [System: [URL]] = [:]

        for system in System.all {
            let folder = romsFolder.appendingPathComponent(
                system.slug,
                isDirectory: true
            )
            guard
                let files = FileManager.default.enumerator(
                    at: folder,
                    includingPropertiesForKeys: [.isRegularFileKey],
                    options: [.skipsHiddenFiles]
                )
            else {
                continue
            }

            let extensions = Set(system.romExtensions)
            for case let url as URL in files
            where
                extensions.contains(url.pathExtension.lowercased())
            {
                result[system, default: []].append(url)
            }
        }

        return result.mapValues { files in
            files.sorted {
                $0.lastPathComponent.localizedStandardCompare(
                    $1.lastPathComponent
                ) == .orderedAscending
            }
        }
    }

    static func saveState(_ state: Data, path: String) throws {
        try writeData(state, path: path)
    }

    static func loadState(path: String) throws -> Data? {
        try loadData(path: path)
    }

    static func hasState(path: String) -> Bool {
        FileManager.default.fileExists(
            atPath: documentsFolder.appendingPathComponent(path).path
        )
    }

    static func saveRAM(_ data: Data, path: String) throws {
        try writeData(data, path: path)
    }

    static func loadSaveRAM(path: String) throws -> Data? {
        try loadData(path: path)
    }

    private static func writeData(_ data: Data, path: String) throws {
        let url = documentsFolder.appendingPathComponent(path)
        try FileManager.default.createDirectory(
            at: url.deletingLastPathComponent(),
            withIntermediateDirectories: true
        )
        try data.write(to: url, options: .atomic)
    }

    private static func loadData(path: String) throws -> Data? {
        let url = documentsFolder.appendingPathComponent(path)
        guard FileManager.default.fileExists(atPath: url.path) else { return nil }
        return try Data(contentsOf: url)
    }
}
