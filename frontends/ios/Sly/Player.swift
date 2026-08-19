import Foundation
import Observation
import SlyKit

@Observable
public final class Player {
    let currentSystem: System
    let currentRom: URL
    let core: EmulatorCore
    
    public private(set) var isPlaying = true
    
    init(system: System, rom: URL) throws {
        let buffer = try Data(contentsOf: rom)

        currentSystem = system
        currentRom = rom
        core = try EmulatorCore(system: system, rom: buffer)
    }
    
    func reset() {
        core.reset()
    }
    
    func pause() {
        isPlaying = false
    }
    
    func play() {
        isPlaying = true
    }
    
    func runToFrame() -> Frame? {
        guard isPlaying else {
            return nil
        }
        
        core.runFrame()
        return core.frame()
    }
}
