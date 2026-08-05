use core_model::{AudioSpec, Core, StateError, System, TvSystem};
use snes::{Cartridge, Snes, SnesError};

pub use core_model::Button;

const _: () = assert!(size_of::<(u8, u8, u8)>() == 3 && align_of::<(u8, u8, u8)>() == 1);

/// Captured at load time, before the `Cartridge` is consumed by the core.
pub struct CoreMeta {
    pub title: String,
    pub region_name: String,
    pub tv_system: TvSystem,
    pub rom_crc32: u32,
    pub has_battery: bool,
    pub sram_size: usize,
}

/// The stable index shared by the C and C++ APIs.
pub fn button_from_index(index: usize) -> Option<Button> {
    Button::ALL.get(index).copied()
}

/// The stable index shared by the C and C++ APIs.
pub fn system_from_index(index: usize) -> Option<System> {
    match index {
        0 => Some(System::Snes),
        _ => None,
    }
}

/// Assert at compile time that a binding's button enum matches the index
/// contract [`button_from_index`] relies on: variant `n` of `$ty` must name
/// the same button as `Button::ALL[n]`.
#[macro_export]
macro_rules! assert_button_order {
    ($ty:ty, |$value:ident| $repr:expr) => {
        const _: () = {
            use $crate::runtime::Button as Core;

            const BUTTONS: [($ty, Core); Core::ALL.len()] = [
                (<$ty>::Up, Core::Up),
                (<$ty>::Down, Core::Down),
                (<$ty>::Left, Core::Left),
                (<$ty>::Right, Core::Right),
                (<$ty>::A, Core::A),
                (<$ty>::B, Core::B),
                (<$ty>::X, Core::X),
                (<$ty>::Y, Core::Y),
                (<$ty>::L, Core::L),
                (<$ty>::R, Core::R),
                (<$ty>::Start, Core::Start),
                (<$ty>::Select, Core::Select),
            ];

            let mut index = 0;
            while index < BUTTONS.len() {
                let $value = BUTTONS[index].0;
                assert!($repr == index);
                assert!(BUTTONS[index].1 as usize == Core::ALL[index] as usize);
                index += 1;
            }
        };
    };
}

fn rgb8_bytes(pixels: &[(u8, u8, u8)]) -> &[u8] {
    // SAFETY: the const assert above pins the tuple to 3 bytes, alignment 1.
    unsafe { std::slice::from_raw_parts(pixels.as_ptr().cast(), pixels.len() * 3) }
}

/// Not thread-safe: use a handle from one thread at a time. The handle itself
/// may be moved between threads.
pub struct Runtime {
    system: System,
    core: Box<dyn Core + Send>,
    audio_scratch: Vec<i16>,
    audio_cursor: usize,
    meta: CoreMeta,
}

impl Runtime {
    pub fn new(system: System, rom: Vec<u8>) -> Result<Self, String> {
        let (core, meta) = match system {
            System::Snes => Self::load_snes(rom).map_err(|error| error.to_string())?,
        };
        Ok(Self {
            system,
            core,
            audio_scratch: Vec::new(),
            audio_cursor: 0,
            meta,
        })
    }

    fn load_snes(rom: Vec<u8>) -> Result<(Box<dyn Core + Send>, CoreMeta), SnesError> {
        let cartridge = Cartridge::new(rom)?;
        let region = cartridge.region();
        let meta = CoreMeta {
            title: cartridge.title(),
            region_name: region.to_string(),
            tv_system: region.tv_system(),
            rom_crc32: cartridge.rom_crc32(),
            has_battery: cartridge.has_battery(),
            sram_size: cartridge.sram_size(),
        };
        Ok((Box::new(Snes::new(cartridge)), meta))
    }

    pub fn system_slug(&self) -> &'static str {
        self.system.slug()
    }

    pub fn meta(&self) -> &CoreMeta {
        &self.meta
    }

    pub fn reset(&mut self) {
        self.core.reset();
    }

    pub fn step(&mut self) {
        self.core.step();
    }

    pub fn run_frame(&mut self) {
        self.core.run_frame();
    }

    pub fn set_button(&mut self, player: usize, button: Button, pressed: bool) {
        self.core.set_button(player, button, pressed);
    }

    pub fn audio_spec(&self) -> Option<AudioSpec> {
        self.core.audio_spec()
    }

    /// Everything the core has produced plus any remainder a previous
    /// [`Self::drain_audio_into`] left behind, interleaved. Valid until the
    /// next `&mut self` call.
    pub fn drain_audio(&mut self) -> &[i16] {
        self.refill_audio();
        self.audio_cursor = self.audio_scratch.len();
        &self.audio_scratch
    }

    /// Copies up to `out.len()` samples, retaining any remainder for the next
    /// drain call.
    pub fn drain_audio_into(&mut self, out: &mut [i16]) -> usize {
        self.refill_audio();
        let n = out.len().min(self.audio_scratch.len());
        out[..n].copy_from_slice(&self.audio_scratch[..n]);
        self.audio_cursor = n;
        n
    }

    fn refill_audio(&mut self) {
        if self.audio_cursor > 0 {
            self.audio_scratch.drain(..self.audio_cursor);
            self.audio_cursor = 0;
        }
        self.core.drain_audio(&mut self.audio_scratch);
    }

    /// `(width, height, pixels)` with `pixels` tightly packed RGB8. Invalidated
    /// by the next mutating call.
    pub fn frame_rgb8(&self) -> Option<(u32, u32, &[u8])> {
        let frame = self.core.software_frame()?;
        frame
            .is_valid()
            .then(|| (frame.width, frame.height, rgb8_bytes(frame.pixels)))
    }

    pub fn frame_interval_ns(&self) -> Option<u64> {
        self.core
            .frame_interval()
            .map(|interval| interval.as_nanos().min(u64::MAX as u128) as u64)
    }

    pub fn save_state(&self) -> Result<Vec<u8>, StateError> {
        self.core.save_state()
    }

    pub fn load_state(&mut self, bytes: &[u8]) -> Result<(), StateError> {
        self.core.load_state(bytes)
    }

    pub fn save_ram(&self) -> Option<&[u8]> {
        self.core.save_ram()
    }

    pub fn load_save_ram(&mut self, bytes: &[u8]) -> bool {
        self.core.load_save_ram(bytes)
    }
}
