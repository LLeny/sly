import Foundation

@_implementationOnly import SlyFFI

public enum SlyError: Error, Equatable {
    case incompatibleABI(expected: UInt32, actual: UInt32)
    case native(status: Int, message: String)
}

/// A system supported by the linked native core library.
public struct System: Hashable, Sendable {
    private let nativeValue: UInt32

    // Convenience names.
    public static let snes = System(native: SLY_SYSTEM_SNES)
    public static let gba = System(native: SLY_SYSTEM_GBA)

    public static let all: [System] = (0..<sly_system_count()).compactMap { index in
        var native = SlySystem(rawValue: 0)
        guard sly_system_at(index, &native) else { return nil }
        return System(native: native)
    }

    private init(native: SlySystem) {
        nativeValue = native.rawValue
    }

    public var slug: String {
        nativeString(sly_system_slug(native))
    }

    public var romExtensions: [String] {
        (0..<sly_system_extension_count(native)).map {
            nativeString(sly_system_extension(native, $0))
        }
    }

    public var displayName: String {
        nativeString(sly_system_name(native))
    }

    public var qualifiedName: String {
        nativeString(sly_system_qualified_name(native))
    }

    var native: SlySystem {
        SlySystem(rawValue: nativeValue)
    }
}

public enum Button: Hashable, Sendable, CaseIterable {
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

public enum TelevisionSystem: Sendable {
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

public enum BackupKind: Sendable {
    case sram, fram, eeprom, flash
}

public struct ControllerDescriptor: Sendable {
    public let port: UInt32
    public let name: String
    public let buttons: [Button]
}

public struct Cartridge {
    public let title: String
    public let system: String
    public let region: String
    public let televisionSystem: TelevisionSystem?
    public let romCRC32: UInt32
    public let romSHA1: String
    public let romSize: Int
    public let backupKind: BackupKind?
    public let backupBatteryBacked: Bool
    public let backupCapacity: Int

    public static func inspect(
        system: System = .snes,
        rom: Data
    ) throws -> Cartridge {
        try checkABI()
        var info = SlyCartridgeInfo()
        let status = rom.withUnsafeBytes { bytes in
            sly_cartridge_inspect(
                system.native,
                bytes.baseAddress?.assumingMemoryBound(to: UInt8.self),
                bytes.count,
                &info
            )
        }
        try checkNativeStatus(status)
        defer { sly_cartridge_info_free(&info) }
        return Cartridge(
            title: nativeString(info.title),
            system: nativeString(info.system),
            region: nativeString(info.region_name),
            televisionSystem: decodeTelevisionSystem(info.tv_system),
            romCRC32: info.rom_crc32,
            romSHA1: nativeString(info.rom_sha1),
            romSize: info.rom_size,
            backupKind: decodeBackupKind(info.backup_kind),
            backupBatteryBacked: info.backup_battery_backed,
            backupCapacity: info.backup_capacity
        )
    }
}

private func decodeTelevisionSystem(_ value: SlyTvSystem) -> TelevisionSystem? {
    switch value {
    case SLY_TV_SYSTEM_NTSC: .ntsc
    case SLY_TV_SYSTEM_PAL: .pal
    default: nil
    }
}

private func decodeBackupKind(_ value: SlyBackupKind) -> BackupKind? {
    switch value {
    case SLY_BACKUP_KIND_SRAM: .sram
    case SLY_BACKUP_KIND_FRAM: .fram
    case SLY_BACKUP_KIND_EEPROM: .eeprom
    case SLY_BACKUP_KIND_FLASH: .flash
    default: nil
    }
}

/// Owns one native emulator instance. Use it from one execution context at a
/// time; the underlying core is intentionally not thread-safe.
public final class EmulatorCore {
    public static let expectedABIVersion: UInt32 = 3

    private let handle: OpaquePointer

    public init(system: System = .snes, rom: Data) throws {
        try checkABI()

        var core: OpaquePointer?
        let status = rom.withUnsafeBytes { bytes in
            sly_core_create(
                system.native,
                bytes.baseAddress?.assumingMemoryBound(to: UInt8.self),
                bytes.count,
                &core
            )
        }
        try checkNativeStatus(status)

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

    public var controllers: [ControllerDescriptor] {
        (0..<sly_core_controller_count(handle)).compactMap { index in
            var port: UInt32 = 0
            guard sly_core_controller_port(handle, index, &port) else { return nil }
            return ControllerDescriptor(
                port: port,
                name: nativeString(sly_core_controller_name(handle, index)),
                buttons: Button.allCases.filter {
                    sly_core_supports_button(handle, port, $0.native)
                }
            )
        }
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
        try checkNativeStatus(sly_core_save_state(handle, &buffer))
        defer { sly_buffer_free(&buffer) }

        guard let data = buffer.data else { return Data() }
        return Data(bytes: data, count: buffer.len)
    }

    /// 0 is the quick slot, 1...10 are the numbered slots.
    public func statePath(_ index: UInt8 = 0) throws -> String {
        var buffer = SlyBuffer(data: nil, len: 0)
        try checkNativeStatus(sly_core_state_path(handle, index, &buffer))
        defer { sly_buffer_free(&buffer) }
        return nativeString(buffer)
    }

    public func saveRAMPath() throws -> String {
        var buffer = SlyBuffer(data: nil, len: 0)
        try checkNativeStatus(sly_core_save_ram_path(handle, &buffer))
        defer { sly_buffer_free(&buffer) }
        return nativeString(buffer)
    }

    public func loadState(_ state: Data) throws {
        let status = state.withUnsafeBytes { bytes in
            sly_core_load_state(
                handle,
                bytes.baseAddress?.assumingMemoryBound(to: UInt8.self),
                bytes.count
            )
        }
        try checkNativeStatus(status)
    }

    public func saveRAM() -> Data? {
        var data: UnsafePointer<UInt8>?
        var count = 0
        guard sly_core_save_ram(handle, &data, &count), let data else {
            return nil
        }
        return Data(bytes: data, count: count)
    }

    public func persistableSaveRAM() -> Data? {
        var data: UnsafePointer<UInt8>?
        var count = 0
        guard sly_core_persistable_save_ram(handle, &data, &count), let data else { return nil }
        return Data(bytes: data, count: count)
    }

    public func inhibitSaveRAMPersistence() {
        sly_core_inhibit_save_ram_persistence(handle)
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
            title: nativeString(sly_core_title(handle)),
            system: nativeString(sly_core_system(handle)),
            region: nativeString(sly_core_region_name(handle)),
            televisionSystem: decodeTelevisionSystem(sly_core_tv_system(handle)),
            romCRC32: sly_core_rom_crc32(handle),
            romSHA1: nativeString(sly_core_rom_sha1(handle)),
            romSize: sly_core_rom_size(handle),
            backupKind: decodeBackupKind(sly_core_backup_kind(handle)),
            backupBatteryBacked: sly_core_backup_battery_backed(handle),
            backupCapacity: sly_core_backup_capacity(handle)
        )
    }

}

func nativeString(_ pointer: UnsafePointer<CChar>?) -> String {
    pointer.map(String.init(cString:)) ?? ""
}

func nativeString(_ buffer: SlyBuffer) -> String {
    guard let data = buffer.data else { return "" }
    return String(decoding: UnsafeBufferPointer(start: data, count: buffer.len), as: UTF8.self)
}

func checkABI() throws {
    let actualABI = sly_abi_version()
    guard actualABI == EmulatorCore.expectedABIVersion else {
        throw SlyError.incompatibleABI(
            expected: EmulatorCore.expectedABIVersion,
            actual: actualABI
        )
    }
}

func checkNativeStatus(_ status: SlyStatus) throws {
    guard status == SLY_STATUS_OK else {
        throw SlyError.native(
            status: Int(status.rawValue),
            message: nativeString(sly_last_error())
        )
    }
}
