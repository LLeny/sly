use sly::runtime::{Runtime, button_from_index, system_from_index};

#[cxx::bridge(namespace = "sly")]
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
        pal: bool,
        rom_crc32: u32,
        has_battery: bool,
        sram_size: usize,
    }

    #[derive(Clone, Copy, Debug)]
    enum System {
        Snes = 0,
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

        fn create_core(system: System, rom: &[u8]) -> Result<Box<SlyCore>>;

        fn reset(self: &mut SlyCore);
        /// Execute a single CPU instruction.
        fn step(self: &mut SlyCore);
        fn run_frame(self: &mut SlyCore);
        /// 0 if the core does not pace.
        fn frame_interval_ns(self: &SlyCore) -> u64;
        /// `player` is 0-based.
        fn set_button(self: &mut SlyCore, player: u32, button: Button, pressed: bool);

        fn audio_spec(self: &SlyCore) -> AudioSpec;
        /// Pending interleaved i16 samples, borrowed from the core's buffer:
        /// consume before the next non-const call.
        fn drain_audio(self: &mut SlyCore) -> &[i16];

        fn frame_info(self: &SlyCore) -> FrameInfo;
        /// Tightly packed RGB8 borrowed from the core's framebuffer, empty if
        /// no frame is available. Upload before the next non-const call, or
        /// use `copy_frame_rgb` to cross threads.
        fn frame_rgb(self: &SlyCore) -> &[u8];
        /// `out` must be exactly `width * height * 3` bytes. The copy is safe
        /// to hand to another thread.
        fn copy_frame_rgb(self: &SlyCore, out: &mut [u8]) -> bool;

        fn save_state(self: &SlyCore) -> Result<Vec<u8>>;
        fn load_state(self: &mut SlyCore, bytes: &[u8]) -> Result<()>;
        /// Battery-backed SRAM borrowed from the core, empty when there is
        /// none. Valid until the next non-const call.
        fn save_ram(self: &SlyCore) -> &[u8];
        /// False when the cartridge has no battery or the size does not match.
        fn load_save_ram(self: &mut SlyCore, bytes: &[u8]) -> bool;

        fn cartridge_info(self: &SlyCore) -> CartridgeInfo;
    }
}

sly::assert_button_order!(ffi::Button, |button| button.repr as usize);

pub struct SlyCore {
    rt: Runtime,
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
        match self.rt.frame_rgb8() {
            Some((width, height, _)) => ffi::FrameInfo { width, height },
            None => ffi::FrameInfo {
                width: 0,
                height: 0,
            },
        }
    }

    fn frame_rgb(&self) -> &[u8] {
        self.rt.frame_rgb8().map_or(&[], |(_, _, pixels)| pixels)
    }

    fn copy_frame_rgb(&self, out: &mut [u8]) -> bool {
        let Some((_, _, bytes)) = self.rt.frame_rgb8() else {
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

    fn load_save_ram(&mut self, bytes: &[u8]) -> bool {
        self.rt.load_save_ram(bytes)
    }

    fn cartridge_info(&self) -> ffi::CartridgeInfo {
        let meta = self.rt.meta();
        ffi::CartridgeInfo {
            title: meta.title.clone(),
            system: self.rt.system_slug().to_owned(),
            region_name: meta.region_name.clone(),
            pal: matches!(meta.tv_system, core_model::TvSystem::Pal),
            rom_crc32: meta.rom_crc32,
            has_battery: meta.has_battery,
            sram_size: meta.sram_size,
        }
    }
}
