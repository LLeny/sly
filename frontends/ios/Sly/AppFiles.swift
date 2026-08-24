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

    private static var statesFolder: URL {
        documentsFolder.appendingPathComponent("states", isDirectory: true)
    }

    private static var savesFolder: URL {
        documentsFolder.appendingPathComponent("saves", isDirectory: true)
    }
    
    static func prepare() throws {
        let systemFolders = System.all.map {
            romsFolder.appendingPathComponent($0.slug, isDirectory: true)
        }
        for folder in [romsFolder, statesFolder, savesFolder] + systemFolders {
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
            guard let files = FileManager.default.enumerator(
                at: folder,
                includingPropertiesForKeys: [.isRegularFileKey],
                options: [.skipsHiddenFiles]
            ) else {
                continue
            }

            let extensions = Set(system.romExtensions)
            for case let url as URL in files where
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

    static func saveState(_ state: Data, romCRC32: UInt32) throws {
        try state.write(to: quickStateURL(romCRC32: romCRC32), options: .atomic)
    }

    static func loadState(romCRC32: UInt32) throws -> Data? {
        try loadData(at: quickStateURL(romCRC32: romCRC32))
    }

    static func hasState(romCRC32: UInt32) -> Bool {
        FileManager.default.fileExists(
            atPath: quickStateURL(romCRC32: romCRC32).path
        )
    }

    static func saveRAM(_ data: Data, romCRC32: UInt32) throws {
        try data.write(to: saveRAMURL(romCRC32: romCRC32), options: .atomic)
    }

    static func loadSaveRAM(romCRC32: UInt32) throws -> Data? {
        try loadData(at: saveRAMURL(romCRC32: romCRC32))
    }

    private static func loadData(at url: URL) throws -> Data? {
        guard FileManager.default.fileExists(atPath: url.path) else {
            return nil
        }
        return try Data(contentsOf: url)
    }

    private static func quickStateURL(romCRC32: UInt32) -> URL {
        statesFolder.appendingPathComponent(
            String(format: "%08x.quick.sav", romCRC32)
        )
    }

    private static func saveRAMURL(romCRC32: UInt32) -> URL {
        savesFolder.appendingPathComponent(
            String(format: "%08x.srm", romCRC32)
        )
    }
}
