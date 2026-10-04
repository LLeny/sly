#![allow(clippy::missing_safety_doc)]

use std::cell::RefCell;
use std::ffi::{CString, c_char};
use std::sync::OnceLock;

use core_model::{BackupKind, CartridgeInfo, StateError, System, TvSystem};

use crate::runtime::{Runtime, button_from_index, inspect_cartridge, system_from_index};

pub const SLY_ABI_VERSION: u32 = 3;

/// Anything but `Ok` leaves a description in `sly_last_error()`.
#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SlyStatus {
    Ok = 0,
    InvalidArg,
    BadRom,
    StateUnsupported,
    StateBadMagic,
    StateVersion,
    StateSystemMismatch,
    StateRomMismatch,
    StateTruncated,
    StateCorrupt,
    ArtworkMetadata,
}

#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SlySystem {
    Snes = 0,
    Gba = 1,
}

#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SlyButton {
    Up = 0,
    Down,
    Left,
    Right,
    A,
    B,
    X,
    Y,
    L,
    R,
    Start,
    Select,
}

crate::assert_button_order!(SlyButton, |button| button as usize);

#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SlyTvSystem {
    Ntsc = 0,
    Pal,
    Unspecified,
}

#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SlyBackupKind {
    None = 0,
    Sram,
    Fram,
    Eeprom,
    Flash,
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct SlyAudioSpec {
    pub sample_rate: u32,
    pub channels: u8,
}

/// `pixels` is tightly packed RGBA8, `pixels_len == width * height * 4`, and is
/// valid only until the next call that mutates or destroys this core.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct SlyFrame {
    pub width: u32,
    pub height: u32,
    pub pixels: *const u8,
    pub pixels_len: usize,
}

/// Rust-allocated. Free exactly once with `sly_buffer_free`.
#[repr(C)]
#[derive(Debug)]
pub struct SlyBuffer {
    pub data: *mut u8,
    pub len: usize,
}

/// Rust-allocated cartridge metadata. Free exactly once with
/// `sly_cartridge_info_free`.
#[repr(C)]
#[derive(Debug)]
pub struct SlyCartridgeInfo {
    pub title: SlyBuffer,
    pub system: SlyBuffer,
    pub region_name: SlyBuffer,
    pub tv_system: SlyTvSystem,
    pub rom_crc32: u32,
    pub rom_sha1: SlyBuffer,
    pub rom_size: usize,
    pub backup_kind: SlyBackupKind,
    pub backup_capacity: usize,
    pub backup_battery_backed: bool,
}

struct SystemMetadata {
    slug: CString,
    name: CString,
    qualified_name: CString,
    extensions: Vec<CString>,
}

static SYSTEM_METADATA: OnceLock<Vec<SystemMetadata>> = OnceLock::new();

pub struct SlyCore {
    rt: Runtime,
    title: CString,
    system: CString,
    region_name: CString,
    rom_sha1: CString,
    controller_names: Vec<CString>,
}

thread_local! {
    static LAST_ERROR: RefCell<CString> = RefCell::new(CString::default());
}

pub(crate) fn set_last_error(msg: &str) {
    let cstring = cstring_lossy(msg);
    LAST_ERROR.with(|slot| *slot.borrow_mut() = cstring);
}

fn state_status(err: &StateError) -> SlyStatus {
    match err {
        StateError::Unsupported => SlyStatus::StateUnsupported,
        StateError::BadMagic => SlyStatus::StateBadMagic,
        StateError::Version { .. } | StateError::PayloadVersion { .. } => SlyStatus::StateVersion,
        StateError::SystemMismatch => SlyStatus::StateSystemMismatch,
        StateError::RomMismatch => SlyStatus::StateRomMismatch,
        StateError::Truncated => SlyStatus::StateTruncated,
        StateError::Corrupt(_) => SlyStatus::StateCorrupt,
    }
}

fn cstring_lossy(text: &str) -> CString {
    CString::new(text.replace('\0', " ")).unwrap_or_default()
}

fn binding_system(system: System) -> SlySystem {
    match system {
        System::Snes => SlySystem::Snes,
        System::Gba => SlySystem::Gba,
    }
}

pub fn binding_tv(system: Option<TvSystem>) -> SlyTvSystem {
    match system {
        Some(TvSystem::Ntsc) => SlyTvSystem::Ntsc,
        Some(TvSystem::Pal) => SlyTvSystem::Pal,
        None => SlyTvSystem::Unspecified,
    }
}

pub fn binding_backup(kind: Option<BackupKind>) -> SlyBackupKind {
    match kind {
        None => SlyBackupKind::None,
        Some(BackupKind::Sram) => SlyBackupKind::Sram,
        Some(BackupKind::Fram) => SlyBackupKind::Fram,
        Some(BackupKind::Eeprom) => SlyBackupKind::Eeprom,
        Some(BackupKind::Flash) => SlyBackupKind::Flash,
    }
}

fn binding_cartridge(meta: &CartridgeInfo) -> SlyCartridgeInfo {
    SlyCartridgeInfo {
        title: buffer_from_vec(meta.title.as_bytes().to_vec()),
        system: buffer_from_vec(meta.system.slug().as_bytes().to_vec()),
        region_name: buffer_from_vec(crate::runtime::region_name(meta).into_bytes()),
        tv_system: binding_tv(meta.tv_system),
        rom_crc32: meta.rom.crc32,
        rom_sha1: buffer_from_vec(meta.rom.sha1.as_bytes().to_vec()),
        rom_size: meta.rom.len,
        backup_kind: binding_backup(meta.backup.map(|backup| backup.kind)),
        backup_capacity: meta.backup.map_or(0, |backup| backup.capacity),
        backup_battery_backed: meta.backup.is_some_and(|backup| backup.battery_backed),
    }
}

fn system_metadata() -> &'static [SystemMetadata] {
    SYSTEM_METADATA.get_or_init(|| {
        System::ALL
            .iter()
            .map(|system| SystemMetadata {
                slug: cstring_lossy(system.slug()),
                name: cstring_lossy(system.name()),
                qualified_name: cstring_lossy(system.qualified_name()),
                extensions: system
                    .rom_extensions()
                    .iter()
                    .map(|extension| cstring_lossy(extension))
                    .collect(),
            })
            .collect()
    })
}

unsafe fn write_out<T>(out: *mut T, value: Option<T>) -> bool {
    match value {
        Some(value) if !out.is_null() => {
            unsafe { out.write(value) };
            true
        }
        _ => false,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn sly_abi_version() -> u32 {
    SLY_ABI_VERSION
}

#[unsafe(no_mangle)]
pub extern "C" fn sly_system_count() -> usize {
    System::ALL.len()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn sly_system_at(index: usize, out_system: *mut SlySystem) -> bool {
    let system = system_from_index(index).map(binding_system);
    unsafe { write_out(out_system, system) }
}

/// Never NULL for a valid system. Static UTF-8; do not free.
#[unsafe(no_mangle)]
pub extern "C" fn sly_system_slug(system: SlySystem) -> *const c_char {
    system_metadata()
        .get(system as usize)
        .map_or(std::ptr::null(), |metadata| metadata.slug.as_ptr())
}

/// Never NULL for a valid system. Static UTF-8; do not free.
#[unsafe(no_mangle)]
pub extern "C" fn sly_system_name(system: SlySystem) -> *const c_char {
    system_metadata()
        .get(system as usize)
        .map_or(std::ptr::null(), |metadata| metadata.name.as_ptr())
}

/// Never NULL for a valid system. Static UTF-8; do not free.
#[unsafe(no_mangle)]
pub extern "C" fn sly_system_qualified_name(system: SlySystem) -> *const c_char {
    system_metadata()
        .get(system as usize)
        .map_or(std::ptr::null(), |metadata| {
            metadata.qualified_name.as_ptr()
        })
}

#[unsafe(no_mangle)]
pub extern "C" fn sly_system_extension_count(system: SlySystem) -> usize {
    system_metadata()
        .get(system as usize)
        .map_or(0, |metadata| metadata.extensions.len())
}

/// Never NULL for a valid system and extension index. Static UTF-8; do not free.
#[unsafe(no_mangle)]
pub extern "C" fn sly_system_extension(system: SlySystem, index: usize) -> *const c_char {
    system_metadata()
        .get(system as usize)
        .and_then(|metadata| metadata.extensions.get(index))
        .map_or(std::ptr::null(), |extension| extension.as_ptr())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn sly_cartridge_inspect(
    system: SlySystem,
    rom: *const u8,
    rom_len: usize,
    out_info: *mut SlyCartridgeInfo,
) -> SlyStatus {
    if out_info.is_null() || (rom.is_null() && rom_len > 0) {
        set_last_error("NULL argument to sly_cartridge_inspect");
        return SlyStatus::InvalidArg;
    }

    let Some(core_system) = system_from_index(system as usize) else {
        set_last_error("unknown core system");
        return SlyStatus::InvalidArg;
    };
    let bytes = unsafe { slice_from_raw(rom, rom_len) }.to_vec();

    match inspect_cartridge(core_system, bytes) {
        Ok(meta) => {
            unsafe {
                out_info.write(binding_cartridge(&meta));
            }

            SlyStatus::Ok
        }
        Err(error) => {
            set_last_error(&error);
            SlyStatus::BadRom
        }
    }
}

/// NULL is a no-op.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sly_cartridge_info_free(info: *mut SlyCartridgeInfo) {
    let Some(info) = (unsafe { info.as_mut() }) else {
        return;
    };
    unsafe {
        sly_buffer_free(&mut info.title);
        sly_buffer_free(&mut info.system);
        sly_buffer_free(&mut info.region_name);
        sly_buffer_free(&mut info.rom_sha1);
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn sly_core_controller_count(core: *const SlyCore) -> usize {
    unsafe { core.as_ref() }.map_or(0, |core| core.rt.controllers().len())
}

/// The descriptor's port can differ from `index`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sly_core_controller_port(
    core: *const SlyCore,
    index: usize,
    out_port: *mut u32,
) -> bool {
    let port = unsafe { core.as_ref() }
        .and_then(|core| core.rt.controllers().get(index))
        .and_then(|controller| u32::try_from(controller.port).ok());
    unsafe { write_out(out_port, port) }
}

/// Owned by the core, don't free it.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sly_core_controller_name(
    core: *const SlyCore,
    index: usize,
) -> *const c_char {
    unsafe { core.as_ref() }
        .and_then(|core| core.controller_names.get(index))
        .map_or(std::ptr::null(), |name| name.as_ptr())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn sly_core_supports_button(
    core: *const SlyCore,
    port: u32,
    button: SlyButton,
) -> bool {
    let Some(button) = button_from_index(button as usize) else {
        return false;
    };

    unsafe { core.as_ref() }.is_some_and(|core| {
        core.rt
            .controllers()
            .iter()
            .any(|controller| controller.port == port as usize && controller.supports(button))
    })
}

/// Owned by the core, don't free it.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sly_core_rom_sha1(core: *const SlyCore) -> *const c_char {
    unsafe { core.as_ref() }.map_or(std::ptr::null(), |core| core.rom_sha1.as_ptr())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn sly_core_rom_size(core: *const SlyCore) -> usize {
    unsafe { core.as_ref() }.map_or(0, |core| core.rt.meta().rom.len)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn sly_core_backup_kind(core: *const SlyCore) -> SlyBackupKind {
    binding_backup(
        unsafe { core.as_ref() }
            .and_then(|core| core.rt.meta().backup)
            .map(|backup| backup.kind),
    )
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn sly_core_inhibit_save_ram_persistence(core: *const SlyCore) {
    if let Some(core) = unsafe { core.as_ref() } {
        core.rt.inhibit_save_ram_persistence();
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn sly_core_persistable_save_ram(
    core: *const SlyCore,
    out_data: *mut *const u8,
    out_len: *mut usize,
) -> bool {
    if out_data.is_null() || out_len.is_null() {
        return false;
    }

    let Some(bytes) = unsafe { core.as_ref() }.and_then(|core| core.rt.persistable_save_ram())
    else {
        return false;
    };

    unsafe {
        out_data.write(bytes.as_ptr());
        out_len.write(bytes.len());
    }

    true
}

/// Never NULL; empty when no error has occurred. Valid until the next failing
/// `sly_*` call on the same thread. Do not free.
#[unsafe(no_mangle)]
pub extern "C" fn sly_last_error() -> *const c_char {
    LAST_ERROR.with(|slot| slot.borrow().as_ptr())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn sly_core_create(
    system: SlySystem,
    rom: *const u8,
    rom_len: usize,
    out_core: *mut *mut SlyCore,
) -> SlyStatus {
    if out_core.is_null() || (rom.is_null() && rom_len > 0) {
        set_last_error("NULL argument to sly_core_create");
        return SlyStatus::InvalidArg;
    }

    let Some(system) = system_from_index(system as usize) else {
        set_last_error("unknown core system");
        return SlyStatus::InvalidArg;
    };

    let bytes = unsafe { slice_from_raw(rom, rom_len) }.to_vec();

    match Runtime::new(system, bytes) {
        Ok(rt) => {
            let meta = rt.meta();
            let core = Box::new(SlyCore {
                title: cstring_lossy(&meta.title),
                system: cstring_lossy(rt.meta().system.slug()),
                region_name: cstring_lossy(&crate::runtime::region_name(meta)),
                rom_sha1: cstring_lossy(&meta.rom.sha1),
                controller_names: rt
                    .controllers()
                    .iter()
                    .map(|controller| cstring_lossy(&controller.name))
                    .collect(),
                rt,
            });

            unsafe { out_core.write(Box::into_raw(core)) };
            SlyStatus::Ok
        }
        Err(err) => {
            set_last_error(&err.to_string());
            SlyStatus::BadRom
        }
    }
}

/// NULL is a no-op. Invalidates every pointer borrowed from this core.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sly_core_destroy(core: *mut SlyCore) {
    if !core.is_null() {
        drop(unsafe { Box::from_raw(core) });
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn sly_core_reset(core: *mut SlyCore) {
    if let Some(core) = unsafe { core.as_mut() } {
        core.rt.reset();
    }
}

/// Execute a single CPU instruction.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sly_core_step(core: *mut SlyCore) {
    if let Some(core) = unsafe { core.as_mut() } {
        core.rt.step();
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn sly_core_run_frame(core: *mut SlyCore) {
    if let Some(core) = unsafe { core.as_mut() } {
        core.rt.run_frame();
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn sly_core_frame_interval_ns(
    core: *const SlyCore,
    out_ns: *mut u64,
) -> bool {
    let ns = unsafe { core.as_ref() }.and_then(|core| core.rt.frame_interval_ns());
    unsafe { write_out(out_ns, ns) }
}

/// `player` is 0-based.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sly_core_set_button(
    core: *mut SlyCore,
    player: u32,
    button: SlyButton,
    pressed: bool,
) {
    if let Some(core) = unsafe { core.as_mut() }
        && let Some(button) = button_from_index(button as usize)
    {
        core.rt.set_button(player as usize, button, pressed);
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn sly_core_audio_spec(
    core: *const SlyCore,
    out_spec: *mut SlyAudioSpec,
) -> bool {
    let spec = unsafe { core.as_ref() }
        .and_then(|core| core.rt.audio_spec())
        .map(|spec| SlyAudioSpec {
            sample_rate: spec.sample_rate,
            channels: spec.channels,
        });
    unsafe { write_out(out_spec, spec) }
}

/// Move up to `max_samples` interleaved i16 samples into `out` and return the
/// number written. An undrained remainder is retained and returned first by the
/// next call, so size `out` above `sample_rate * channels / fps`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sly_core_drain_audio(
    core: *mut SlyCore,
    out: *mut i16,
    max_samples: usize,
) -> usize {
    let Some(core) = (unsafe { core.as_mut() }) else {
        return 0;
    };

    if out.is_null() || max_samples == 0 {
        return 0;
    }

    let out = unsafe { std::slice::from_raw_parts_mut(out, max_samples) };
    core.rt.drain_audio_into(out)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn sly_core_frame(core: *const SlyCore, out_frame: *mut SlyFrame) -> bool {
    let frame = unsafe { core.as_ref() }
        .and_then(|core| core.rt.frame_rgba8())
        .map(|(width, height, pixels)| SlyFrame {
            width,
            height,
            pixels: pixels.as_ptr(),
            pixels_len: pixels.len(),
        });
    unsafe { write_out(out_frame, frame) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn sly_core_save_state(
    core: *const SlyCore,
    out_state: *mut SlyBuffer,
) -> SlyStatus {
    let Some(core) = (unsafe { core.as_ref() }) else {
        set_last_error("NULL core in sly_core_save_state");
        return SlyStatus::InvalidArg;
    };

    if out_state.is_null() {
        set_last_error("NULL out_state in sly_core_save_state");
        return SlyStatus::InvalidArg;
    }

    match core.rt.save_state() {
        Ok(bytes) => {
            unsafe { out_state.write(buffer_from_vec(bytes)) };
            SlyStatus::Ok
        }
        Err(err) => {
            set_last_error(&err.to_string());
            state_status(&err)
        }
    }
}

/// Invalidates every pointer borrowed from this core.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sly_core_load_state(
    core: *mut SlyCore,
    data: *const u8,
    len: usize,
) -> SlyStatus {
    let Some(core) = (unsafe { core.as_mut() }) else {
        set_last_error("NULL core in sly_core_load_state");
        return SlyStatus::InvalidArg;
    };

    if data.is_null() && len > 0 {
        set_last_error("NULL data in sly_core_load_state");
        return SlyStatus::InvalidArg;
    }

    let bytes = unsafe { slice_from_raw(data, len) };

    match core.rt.load_state(bytes) {
        Ok(()) => SlyStatus::Ok,
        Err(err) => {
            set_last_error(&err.to_string());
            state_status(&err)
        }
    }
}

/// Index 0 is the quick slot, 1 to 10 are the numbered slots. Free with sly_buffer_free.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sly_core_state_path(
    core: *const SlyCore,
    index: u8,
    out_path: *mut SlyBuffer,
) -> SlyStatus {
    let Some(core) = (unsafe { core.as_ref() }) else {
        set_last_error("NULL core in sly_core_state_path");
        return SlyStatus::InvalidArg;
    };

    if out_path.is_null() {
        set_last_error("NULL out_path in sly_core_state_path");
        return SlyStatus::InvalidArg;
    }

    match core.rt.state_path(index) {
        Ok(path) => {
            unsafe { out_path.write(buffer_from_vec(path.into_bytes())) };
            SlyStatus::Ok
        }
        Err(error) => {
            set_last_error(&error);
            SlyStatus::InvalidArg
        }
    }
}

/// Free with sly_buffer_free.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sly_core_save_ram_path(
    core: *const SlyCore,
    out_path: *mut SlyBuffer,
) -> SlyStatus {
    let Some(core) = (unsafe { core.as_ref() }) else {
        set_last_error("NULL core in sly_core_save_ram_path");
        return SlyStatus::InvalidArg;
    };

    if out_path.is_null() {
        set_last_error("NULL out_path in sly_core_save_ram_path");
        return SlyStatus::InvalidArg;
    }

    unsafe { out_path.write(buffer_from_vec(core.rt.save_ram_path().into_bytes())) };
    SlyStatus::Ok
}

/// Safe to call with NULL or an already-freed buffer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sly_buffer_free(buf: *mut SlyBuffer) {
    let Some(buf) = (unsafe { buf.as_mut() }) else {
        return;
    };
    if !buf.data.is_null() {
        let slice = std::ptr::slice_from_raw_parts_mut(buf.data, buf.len);
        drop(unsafe { Box::from_raw(slice) });
    }
    buf.data = std::ptr::null_mut();
    buf.len = 0;
}

/// The pointer is valid until the next mutating call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sly_core_save_ram(
    core: *const SlyCore,
    out_data: *mut *const u8,
    out_len: *mut usize,
) -> bool {
    let Some(core) = (unsafe { core.as_ref() }) else {
        return false;
    };
    if out_data.is_null() || out_len.is_null() {
        return false;
    }
    match core.rt.save_ram() {
        Some(sram) => {
            unsafe {
                out_data.write(sram.as_ptr());
                out_len.write(sram.len());
            }
            true
        }
        None => false,
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn sly_core_load_save_ram(
    core: *mut SlyCore,
    data: *const u8,
    len: usize,
) -> bool {
    let Some(core) = (unsafe { core.as_mut() }) else {
        return false;
    };
    if data.is_null() && len > 0 {
        return false;
    }
    let bytes = unsafe { slice_from_raw(data, len) };
    core.rt.load_save_ram(bytes)
}

/// UTF-8, from the ROM header. Owned by the core; do not free.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sly_core_title(core: *const SlyCore) -> *const c_char {
    match unsafe { core.as_ref() } {
        Some(core) => core.title.as_ptr(),
        None => std::ptr::null(),
    }
}

/// System slug, e.g. "snes". Owned by the core; do not free.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sly_core_system(core: *const SlyCore) -> *const c_char {
    match unsafe { core.as_ref() } {
        Some(core) => core.system.as_ptr(),
        None => std::ptr::null(),
    }
}

/// Owned by the core, don't free it.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sly_core_region_name(core: *const SlyCore) -> *const c_char {
    match unsafe { core.as_ref() } {
        Some(core) => core.region_name.as_ptr(),
        None => std::ptr::null(),
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn sly_core_tv_system(core: *const SlyCore) -> SlyTvSystem {
    binding_tv(unsafe { core.as_ref() }.and_then(|core| core.rt.meta().tv_system))
}

/// Copier header stripped.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sly_core_rom_crc32(core: *const SlyCore) -> u32 {
    unsafe { core.as_ref() }.map_or(0, |core| core.rt.meta().rom.crc32)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn sly_core_backup_battery_backed(core: *const SlyCore) -> bool {
    unsafe { core.as_ref() }.is_some_and(|core| {
        core.rt
            .meta()
            .backup
            .is_some_and(|backup| backup.battery_backed)
    })
}

/// Declared size in bytes, 0 when none.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sly_core_backup_capacity(core: *const SlyCore) -> usize {
    unsafe { core.as_ref() }.map_or(0, |core| {
        core.rt.meta().backup.map_or(0, |backup| backup.capacity)
    })
}

pub(crate) unsafe fn slice_from_raw<'a>(ptr: *const u8, len: usize) -> &'a [u8] {
    if len == 0 {
        &[]
    } else {
        unsafe { std::slice::from_raw_parts(ptr, len) }
    }
}

pub(crate) fn buffer_from_vec(bytes: Vec<u8>) -> SlyBuffer {
    let slice = Box::into_raw(bytes.into_boxed_slice());
    SlyBuffer {
        data: slice.cast(),
        len: slice.len(),
    }
}
