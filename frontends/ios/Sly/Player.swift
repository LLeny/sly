import Foundation
import SlyKit

public final class Player: Identifiable {
    public let id = UUID()

    private static let saveRAMFlushInterval: TimeInterval = 60

    let currentSystem: System
    let currentRom: URL
    let core: EmulatorCore

    private let statePath: String
    private let saveRAMPath: String
    private let backupCapacity: Int
    private var lastSaveRAMFlush: TimeInterval
    private var lastPersistedSaveRAM: Data?

    init(system: System, rom: URL) throws {
        let buffer = try Data(contentsOf: rom)

        currentSystem = system
        currentRom = rom
        core = try EmulatorCore(system: system, rom: buffer)
        statePath = try core.statePath()
        saveRAMPath = try core.saveRAMPath()
        backupCapacity = core.cartridge.backupCapacity
        lastSaveRAMFlush = ProcessInfo.processInfo.systemUptime
        lastPersistedSaveRAM = core.saveRAM()

        do {
            try restoreSaveRAM()
        } catch {
            core.inhibitSaveRAMPersistence()
            print("Failed to restore save RAM:", error)
        }
    }

    func reset() {
        core.reset()
    }

    func saveState() throws {
        try persistSaveRAM()
        try AppFiles.saveState(
            core.saveState(),
            path: statePath
        )
    }

    func loadState() throws {
        guard let state = try AppFiles.loadState(path: statePath) else {
            return
        }

        try core.loadState(state)
    }

    var hasState: Bool {
        AppFiles.hasState(path: statePath)
    }

    func persistSaveRAM() throws {
        guard let saveRAM = core.persistableSaveRAM(),
            saveRAM != lastPersistedSaveRAM
        else {
            return
        }

        try AppFiles.saveRAM(
            saveRAM,
            path: saveRAMPath
        )
        lastPersistedSaveRAM = saveRAM
    }

    func runFrame() -> Frame? {
        core.runFrame()
        flushSaveRAMIfNeeded()
        return core.frame()
    }

    private func restoreSaveRAM() throws {
        guard lastPersistedSaveRAM != nil,
            let saveRAM = try AppFiles.loadSaveRAM(path: saveRAMPath)
        else {
            return
        }

        guard core.loadSaveRAM(saveRAM) else {
            throw PlayerError.backupCapacityMismatch(
                expected: backupCapacity,
                actual: saveRAM.count
            )
        }
        lastPersistedSaveRAM = saveRAM
    }

    private func flushSaveRAMIfNeeded() {
        let now = ProcessInfo.processInfo.systemUptime
        guard now - lastSaveRAMFlush >= Self.saveRAMFlushInterval else {
            return
        }
        lastSaveRAMFlush = now

        do {
            try persistSaveRAM()
        } catch {
            print("Failed to periodically persist save RAM:", error)
        }
    }
}

private enum PlayerError: LocalizedError {
    case backupCapacityMismatch(expected: Int, actual: Int)

    var errorDescription: String? {
        switch self {
        case .backupCapacityMismatch(let expected, let actual):
            "Stored save RAM is \(actual) bytes; this cartridge expects \(expected) bytes."
        }
    }
}
