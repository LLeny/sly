import Foundation
import SlyKit

public final class Player {
    let currentSystem: System
    let currentRom: URL
    let core: EmulatorCore

    init(system: System, rom: URL) throws {
        let buffer = try Data(contentsOf: rom)

        currentSystem = system
        currentRom = rom
        core = try EmulatorCore(system: system, rom: buffer)
    }
    
    func reset() {
        core.reset()
    }

    func saveState() throws {
        try AppFiles.saveState(
            core.saveState(),
            romCRC32: core.cartridge.romCRC32
        )
    }

    func loadState() throws {
        guard let state = try AppFiles.loadState(
            romCRC32: core.cartridge.romCRC32
        ) else {
            return
        }

        try core.loadState(state)
    }

    var hasState: Bool {
        AppFiles.hasState(romCRC32: core.cartridge.romCRC32)
    }

    func runFrame() -> Frame? {
        core.runFrame()
        return core.frame()
    }
}
