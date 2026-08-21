import Foundation
import SlyKit

public final class Player: Identifiable {
    public let id = UUID()

    private static let saveRAMFlushInterval: TimeInterval = 60

    let currentSystem: System
    let currentRom: URL
    let core: EmulatorCore

    private let romCRC32: UInt32
    private let saveRAMSize: Int
    private var lastSaveRAMFlush: TimeInterval
    private var lastPersistedSaveRAM: Data?
    private var canPersistSaveRAM = true

    init(system: System, rom: URL) throws {
        let buffer = try Data(contentsOf: rom)

        currentSystem = system
        currentRom = rom
        core = try EmulatorCore(system: system, rom: buffer)
        romCRC32 = core.cartridge.romCRC32
        saveRAMSize = core.cartridge.saveRAMSize
        lastSaveRAMFlush = ProcessInfo.processInfo.systemUptime
        lastPersistedSaveRAM = core.saveRAM()

        do {
            try restoreSaveRAM()
        } catch {
            canPersistSaveRAM = false
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
            romCRC32: romCRC32
        )
    }

    func loadState() throws {
        guard let state = try AppFiles.loadState(
            romCRC32: romCRC32
        ) else {
            return
        }

        try core.loadState(state)
    }

    var hasState: Bool {
        AppFiles.hasState(romCRC32: romCRC32)
    }

    func persistSaveRAM() throws {
        guard canPersistSaveRAM,
              let saveRAM = core.saveRAM(),
              saveRAM != lastPersistedSaveRAM
        else {
            return
        }

        try AppFiles.saveRAM(
            saveRAM,
            romCRC32: romCRC32
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
              let saveRAM = try AppFiles.loadSaveRAM(
                romCRC32: romCRC32
              ) else {
            return
        }

        guard core.loadSaveRAM(saveRAM) else {
            throw PlayerError.saveRAMSizeMismatch(
                expected: saveRAMSize,
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
    case saveRAMSizeMismatch(expected: Int, actual: Int)

    var errorDescription: String? {
        switch self {
        case let .saveRAMSizeMismatch(expected, actual):
            "Stored save RAM is \(actual) bytes; this cartridge expects \(expected) bytes."
        }
    }
}
