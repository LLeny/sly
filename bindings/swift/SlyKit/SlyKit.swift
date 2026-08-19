import Foundation

@_implementationOnly import SlyFFI

public enum SlyError: Error, Equatable {
    case incompatibleABI(expected: UInt32, actual: UInt32)
    case native(status: Int, message: String)
}

public enum System {
    case snes

    public var displayName: String {
        switch self {
        case .snes:
            "Super Nintendo Entertainment System"
        }
    }
    
    fileprivate var native: SlySystem {
        switch self {
        case .snes: SLY_SYSTEM_SNES
        }
    }
}

public enum Button {
    case up, down, left, right
    case a, b, x, y
    case l, r, start, select

    fileprivate var native: SlyButton {
        switch self {
        case .up: SLY_BUTTON_UP
        case .down: SLY_BUTTON_DOWN
        case .left: SLY_BUTTON_LEFT
        case .right: SLY_BUTTON_RIGHT
        case .a: SLY_BUTTON_A
        case .b: SLY_BUTTON_B
        case .x: SLY_BUTTON_X
        case .y: SLY_BUTTON_Y
        case .l: SLY_BUTTON_L
        case .r: SLY_BUTTON_R
        case .start: SLY_BUTTON_START
        case .select: SLY_BUTTON_SELECT
        }
    }
}

public enum TelevisionSystem {
    case ntsc
    case pal
}

public struct Frame {
    public let width: Int
    public let height: Int
    public let rgba: Data
}

public struct AudioSpec {
    public let sampleRate: UInt32
    public let channels: UInt8
}

public struct Cartridge {
    public let title: String
    public let system: String
    public let region: String
    public let televisionSystem: TelevisionSystem
    public let romCRC32: UInt32
    public let hasBattery: Bool
    public let saveRAMSize: Int
}

/// Owns one native emulator instance. Use it from one execution context at a
/// time; the underlying core is intentionally not thread-safe.
public final class EmulatorCore {
    public static let expectedABIVersion: UInt32 = 1

    private let handle: OpaquePointer

    public init(system: System = .snes, rom: Data) throws {
        let actualABI = sly_abi_version()
        guard actualABI == Self.expectedABIVersion else {
            throw SlyError.incompatibleABI(
                expected: Self.expectedABIVersion,
                actual: actualABI
            )
        }

        var core: OpaquePointer?
        let status = rom.withUnsafeBytes { bytes in
            sly_core_create(
                system.native,
                bytes.baseAddress?.assumingMemoryBound(to: UInt8.self),
                bytes.count,
                &core
            )
        }
        try Self.check(status)

        guard let core else {
            throw SlyError.native(
                status: Int(SLY_STATUS_INVALID_ARG.rawValue),
                message: "Native core creation returned no handle"
            )
        }
        handle = core
    }

    deinit {
        sly_core_destroy(handle)
    }

    public func reset() {
        sly_core_reset(handle)
    }

    public func step() {
        sly_core_step(handle)
    }

    public func runFrame() {
        sly_core_run_frame(handle)
    }

    public func setButton(_ button: Button, pressed: Bool, player: UInt32 = 0) {
        sly_core_set_button(handle, player, button.native, pressed)
    }

    public var frameIntervalNanoseconds: UInt64? {
        var interval: UInt64 = 0
        return sly_core_frame_interval_ns(handle, &interval) ? interval : nil
    }

    public var audioSpec: AudioSpec? {
        var spec = SlyAudioSpec(sample_rate: 0, channels: 0)
        guard sly_core_audio_spec(handle, &spec) else { return nil }
        return AudioSpec(sampleRate: spec.sample_rate, channels: spec.channels)
    }

    public func drainAudio(maxSamples: Int) -> [Int16] {
        guard maxSamples > 0 else { return [] }

        var samples = [Int16](repeating: 0, count: maxSamples)
        let written = samples.withUnsafeMutableBufferPointer { buffer in
            sly_core_drain_audio(handle, buffer.baseAddress, buffer.count)
        }
        samples.removeSubrange(Int(written)..<samples.count)
        return samples
    }

    /// Returns an owned copy because the native frame pointer is only valid
    /// until the next operation that mutates the core.
    public func frame() -> Frame? {
        var frame = SlyFrame(width: 0, height: 0, pixels: nil, pixels_len: 0)
        guard sly_core_frame(handle, &frame), let pixels = frame.pixels else {
            return nil
        }
        return Frame(
            width: Int(frame.width),
            height: Int(frame.height),
            rgba: Data(bytes: pixels, count: frame.pixels_len)
        )
    }

    public func saveState() throws -> Data {
        var buffer = SlyBuffer(data: nil, len: 0)
        try Self.check(sly_core_save_state(handle, &buffer))
        defer { sly_buffer_free(&buffer) }

        guard let data = buffer.data else { return Data() }
        return Data(bytes: data, count: buffer.len)
    }

    public func loadState(_ state: Data) throws {
        let status = state.withUnsafeBytes { bytes in
            sly_core_load_state(
                handle,
                bytes.baseAddress?.assumingMemoryBound(to: UInt8.self),
                bytes.count
            )
        }
        try Self.check(status)
    }

    public func saveRAM() -> Data? {
        var data: UnsafePointer<UInt8>?
        var count = 0
        guard sly_core_save_ram(handle, &data, &count), let data else {
            return nil
        }
        return Data(bytes: data, count: count)
    }

    @discardableResult
    public func loadSaveRAM(_ data: Data) -> Bool {
        data.withUnsafeBytes { bytes in
            sly_core_load_save_ram(
                handle,
                bytes.baseAddress?.assumingMemoryBound(to: UInt8.self),
                bytes.count
            )
        }
    }

    public var cartridge: Cartridge {
        Cartridge(
            title: Self.string(sly_core_title(handle)),
            system: Self.string(sly_core_system(handle)),
            region: Self.string(sly_core_region_name(handle)),
            televisionSystem: sly_core_tv_system(handle) == SLY_TV_SYSTEM_PAL
                ? .pal
                : .ntsc,
            romCRC32: sly_core_rom_crc32(handle),
            hasBattery: sly_core_has_battery(handle),
            saveRAMSize: sly_core_sram_size(handle)
        )
    }

    private static func check(_ status: SlyStatus) throws {
        guard status == SLY_STATUS_OK else {
            throw SlyError.native(
                status: Int(status.rawValue),
                message: string(sly_last_error())
            )
        }
    }

    private static func string(_ pointer: UnsafePointer<CChar>?) -> String {
        pointer.map(String.init(cString:)) ?? ""
    }
}
