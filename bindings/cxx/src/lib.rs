use core_model::System as CoreSystem;
use settings::{api_key, frontend_json, save_frontend_json, set_api_key};
use sly::runtime::{
    Runtime, button_from_index, inspect_cartridge as inspect_runtime_cartridge, system_from_index,
};

#[cxx::bridge(namespace = "sly::ffi")]
mod ffi {
    /// `channels == 0` means the core has no audio output.
    #[derive(Clone, Copy, Debug)]
    struct AudioSpec {
        sample_rate: u32,
        channels: u8,
    }

    /// Zeroed when no frame exists.
    #[derive(Clone, Copy, Debug)]
    struct FrameInfo {
        width: u32,
        height: u32,
    }

    #[derive(Clone, Debug)]
    struct CartridgeInfo {
        title: String,
        system: String,
        region_name: String,
        rom_crc32: u32,
        backup_battery_backed: bool,
        backup_capacity: usize,
        rom_sha1: String,
        rom_size: usize,
        /// 0 = NTSC, 1 = PAL, 2 = none.
        tv_system: u8,
        /// 0 = none, 1 = SRAM, 2 = FRAM, 3 = EEPROM, 4 = flash.
        backup_kind: u8,
    }

    #[derive(Clone, Debug)]
    struct ControllerInfo {
        port: u32,
        name: String,
        buttons: Vec<u8>,
    }

    #[derive(Clone, Copy, Debug)]
    enum System {
        Snes = 0,
        Gba = 1,
    }

    /// Values match `SlyButton` in the C API.
    #[derive(Clone, Copy, Debug)]
    enum Button {
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

    extern "Rust" {
        type SlyCore;

        fn api_key() -> Result<String>;
        fn set_api_key(key: &str) -> Result<()>;
        fn frontend_json(name: &str) -> Result<String>;
        fn save_frontend_json(name: &str, json: &str) -> Result<()>;

        fn system_count() -> usize;
        fn system_at(index: usize) -> Result<System>;
        fn system_slug(system: System) -> Result<String>;
        fn system_name(system: System) -> Result<String>;
        fn system_qualified_name(system: System) -> Result<String>;
        fn system_extensions(system: System) -> Result<Vec<String>>;
        fn rom_sha1(system: System, rom: &[u8]) -> Result<String>;
        fn inspect_cartridge(system: System, rom: &[u8]) -> Result<CartridgeInfo>;
        fn fetch_wheel_artwork(system: System, rom: &[u8]) -> Result<Vec<u8>>;

        fn create_core(system: System, rom: &[u8]) -> Result<Box<SlyCore>>;

        fn reset(self: &mut SlyCore);
        /// Execute a single CPU instruction.
        fn step(self: &mut SlyCore);
        fn run_frame(self: &mut SlyCore);
        /// 0 if the core does not pace.
        fn frame_interval_ns(self: &SlyCore) -> u64;
        /// `player` is 0-based.
        fn set_button(self: &mut SlyCore, player: u32, button: Button, pressed: bool);

        fn controllers(self: &SlyCore) -> Vec<ControllerInfo>;

        fn audio_spec(self: &SlyCore) -> AudioSpec;
        /// Pending interleaved i16 samples, borrowed from the core's buffer:
        /// consume before the next non-const call.
        fn drain_audio(self: &mut SlyCore) -> &[i16];

        fn frame_info(self: &SlyCore) -> FrameInfo;
        /// Tightly packed RGBA8 borrowed from the core's framebuffer, empty if
        /// no frame is available. Upload before the next non-const call, or
        /// use `copy_frame_rgba` to cross threads.
        fn frame_rgba(self: &SlyCore) -> &[u8];
        /// `out` must be exactly `width * height * 4` bytes. The copy is safe
        /// to hand to another thread.
        fn copy_frame_rgba(self: &SlyCore, out: &mut [u8]) -> bool;

        fn save_state(self: &SlyCore) -> Result<Vec<u8>>;

        fn load_state(self: &mut SlyCore, bytes: &[u8]) -> Result<()>;
        /// Only valid until the next non-const call.
        fn save_ram(self: &SlyCore) -> &[u8];
        fn load_save_ram(self: &mut SlyCore, bytes: &[u8]) -> bool;

        fn persistable_save_ram(self: &SlyCore) -> &[u8];

        fn inhibit_save_ram_persistence(self: &SlyCore);

        fn state_path(self: &SlyCore, index: u8) -> Result<String>;

        fn save_ram_path(self: &SlyCore) -> String;

        fn cartridge_info(self: &SlyCore) -> CartridgeInfo;
    }
}

sly::assert_button_order!(ffi::Button, |button| button.repr as usize);

pub struct SlyCore {
    rt: Runtime,
}

fn binding_system(system: CoreSystem) -> ffi::System {
    match system {
        CoreSystem::Snes => ffi::System::Snes,
        CoreSystem::Gba => ffi::System::Gba,
    }
}

fn core_system(system: ffi::System) -> Result<CoreSystem, String> {
    system_from_index(system.repr as usize)
        .ok_or_else(|| format!("unknown core system {}", system.repr))
}

fn system_count() -> usize {
    CoreSystem::ALL.len()
}

fn system_at(index: usize) -> Result<ffi::System, String> {
    system_from_index(index)
        .map(binding_system)
        .ok_or_else(|| format!("unknown core system index {index}"))
}

fn system_slug(system: ffi::System) -> Result<String, String> {
    Ok(core_system(system)?.slug().to_owned())
}

fn system_name(system: ffi::System) -> Result<String, String> {
    Ok(core_system(system)?.name().to_owned())
}

fn system_qualified_name(system: ffi::System) -> Result<String, String> {
    Ok(core_system(system)?.qualified_name().to_owned())
}

fn system_extensions(system: ffi::System) -> Result<Vec<String>, String> {
    Ok(core_system(system)?
        .rom_extensions()
        .iter()
        .map(|extension| (*extension).to_owned())
        .collect())
}

fn rom_sha1(system: ffi::System, rom: &[u8]) -> Result<String, String> {
    Ok(core_loader::identity(core_system(system)?, rom).sha1)
}

fn binding_cartridge_info(meta: &core_model::CartridgeInfo) -> ffi::CartridgeInfo {
    ffi::CartridgeInfo {
        title: meta.title.clone(),
        system: meta.system.slug().to_owned(),
        region_name: sly::runtime::region_name(meta),
        rom_crc32: meta.rom.crc32,
        backup_battery_backed: meta.backup.is_some_and(|backup| backup.battery_backed),
        backup_capacity: meta.backup.map_or(0, |backup| backup.capacity),
        rom_sha1: meta.rom.sha1.clone(),
        rom_size: meta.rom.len,
        tv_system: sly::capi::binding_tv(meta.tv_system) as u8,
        backup_kind: sly::capi::binding_backup(meta.backup.map(|backup| backup.kind)) as u8,
    }
}

fn inspect_cartridge(system: ffi::System, rom: &[u8]) -> Result<ffi::CartridgeInfo, String> {
    let system = core_system(system)?;
    let meta = inspect_runtime_cartridge(system, rom.to_vec())?;
    Ok(binding_cartridge_info(&meta))
}

fn fetch_wheel_artwork(system: ffi::System, rom: &[u8]) -> Result<Vec<u8>, String> {
    let system = core_system(system)?;
    let meta = inspect_runtime_cartridge(system, rom.to_vec())?;
    Ok(artwork::layle::fetch_wheel(&meta.game_id(), meta.region)?.unwrap_or_default())
}

fn create_core(system: ffi::System, rom: &[u8]) -> Result<Box<SlyCore>, String> {
    let system = system_from_index(system.repr as usize)
        .ok_or_else(|| format!("unknown core system {}", system.repr))?;
    Runtime::new(system, rom.to_vec()).map(|rt| Box::new(SlyCore { rt }))
}

impl SlyCore {
    fn reset(&mut self) {
        self.rt.reset();
    }

    fn step(&mut self) {
        self.rt.step();
    }

    fn run_frame(&mut self) {
        self.rt.run_frame();
    }

    fn frame_interval_ns(&self) -> u64 {
        self.rt.frame_interval_ns().unwrap_or(0)
    }

    fn set_button(&mut self, player: u32, button: ffi::Button, pressed: bool) {
        if let Some(button) = button_from_index(button.repr as usize) {
            self.rt.set_button(player as usize, button, pressed);
        }
    }

    fn controllers(&self) -> Vec<ffi::ControllerInfo> {
        self.rt
            .controllers()
            .iter()
            .map(|controller| ffi::ControllerInfo {
                port: controller.port as u32,
                name: controller.name.to_string(),
                buttons: controller
                    .buttons
                    .iter()
                    .map(|button| *button as u8)
                    .collect(),
            })
            .collect()
    }

    fn audio_spec(&self) -> ffi::AudioSpec {
        match self.rt.audio_spec() {
            Some(spec) => ffi::AudioSpec {
                sample_rate: spec.sample_rate,
                channels: spec.channels,
            },
            None => ffi::AudioSpec {
                sample_rate: 0,
                channels: 0,
            },
        }
    }

    fn drain_audio(&mut self) -> &[i16] {
        self.rt.drain_audio()
    }

    fn frame_info(&self) -> ffi::FrameInfo {
        match self.rt.frame_rgba8() {
            Some((width, height, _)) => ffi::FrameInfo { width, height },
            None => ffi::FrameInfo {
                width: 0,
                height: 0,
            },
        }
    }

    fn frame_rgba(&self) -> &[u8] {
        self.rt.frame_rgba8().map_or(&[], |(_, _, pixels)| pixels)
    }

    fn copy_frame_rgba(&self, out: &mut [u8]) -> bool {
        let Some((_, _, bytes)) = self.rt.frame_rgba8() else {
            return false;
        };
        if out.len() != bytes.len() {
            return false;
        }
        out.copy_from_slice(bytes);
        true
    }

    fn save_state(&self) -> Result<Vec<u8>, core_model::StateError> {
        self.rt.save_state()
    }

    fn load_state(&mut self, bytes: &[u8]) -> Result<(), core_model::StateError> {
        self.rt.load_state(bytes)
    }

    fn save_ram(&self) -> &[u8] {
        self.rt.save_ram().unwrap_or_default()
    }

    fn state_path(&self, index: u8) -> Result<String, String> {
        self.rt.state_path(index)
    }

    fn save_ram_path(&self) -> String {
        self.rt.save_ram_path()
    }

    fn load_save_ram(&mut self, bytes: &[u8]) -> bool {
        self.rt.load_save_ram(bytes)
    }

    fn persistable_save_ram(&self) -> &[u8] {
        self.rt.persistable_save_ram().unwrap_or_default()
    }

    fn inhibit_save_ram_persistence(&self) {
        self.rt.inhibit_save_ram_persistence();
    }

    fn cartridge_info(&self) -> ffi::CartridgeInfo {
        binding_cartridge_info(self.rt.meta())
    }
}
