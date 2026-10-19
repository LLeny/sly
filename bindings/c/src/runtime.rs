use core_model::{
    AudioSpec, CartridgeInfo, ControllerDescriptor, Core, RomFile, StateError, System,
};
use storage::BackupPolicy;

pub use core_model::Button;

/// The stable index shared by the C and C++ APIs.
pub fn button_from_index(index: usize) -> Option<Button> {
    Button::ALL.get(index).copied()
}

pub fn region_name(meta: &CartridgeInfo) -> String {
    meta.region
        .map(|region| region.to_string())
        .unwrap_or_default()
}

/// The stable index shared by the C and C++ APIs.
pub fn system_from_index(index: usize) -> Option<System> {
    System::ALL.get(index).copied()
}

pub fn inspect_cartridge(system: System, rom: Vec<u8>) -> Result<CartridgeInfo, String> {
    core_loader::inspect(system, rom).map_err(|error| error.to_string())
}

/// Assert at compile time that a binding's button enum matches the index
/// contract [`button_from_index`] relies on: variant `n` of `$ty` must name
/// the same button as `Button::ALL[n]`.
#[macro_export]
macro_rules! assert_button_order {
    ($ty:ty, |$value:ident| $repr:expr) => {
        const _: () = {
            use $crate::runtime::Button as Core;

            const BUTTONS: [($ty, Core); Core::CLASSIC.len()] = [
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

/// Not thread-safe: use a handle from one thread at a time. The handle itself
/// may be moved between threads.
pub struct Runtime {
    core: Box<dyn Core + Send>,
    audio_scratch: Vec<i16>,
    audio_cursor: usize,
    meta: CartridgeInfo,
    controllers: Vec<ControllerDescriptor>,
    backup_policy: BackupPolicy,
}

impl Runtime {
    pub fn new(system: System, rom: Vec<u8>) -> Result<Self, String> {
        let cartridge = RomFile::new().data(rom);
        let core = core_loader::load(system, cartridge).map_err(|error| error.to_string())?;
        let meta = core
            .cartridge_info()
            .ok_or("loaded core did not report cartridge metadata")?;
        let controllers = core.controllers();
        Ok(Self {
            core,
            audio_scratch: Vec::new(),
            audio_cursor: 0,
            meta,
            controllers,
            backup_policy: BackupPolicy::new(),
        })
    }

    pub fn meta(&self) -> &CartridgeInfo {
        &self.meta
    }

    pub fn reset(&mut self) {
        self.core.reset();
        self.audio_scratch.clear();
        self.audio_cursor = 0;
    }

    pub fn step(&mut self) {
        self.core.step();
    }

    pub fn run_frame(&mut self) {
        self.core.run_frame();
    }

    pub fn set_button(&mut self, player: usize, button: Button, pressed: bool) {
        if self
            .controllers
            .iter()
            .any(|controller| controller.port == player && controller.supports(button))
        {
            self.core.set_button(player, button, pressed);
        }
    }

    pub fn controllers(&self) -> &[ControllerDescriptor] {
        &self.controllers
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

    /// `(width, height, pixels)` with `pixels` tightly packed RGBA8.
    /// Invalidated by the next mutating call.
    pub fn frame_rgba8(&self) -> Option<(u32, u32, &[u8])> {
        let frame = self.core.software_frame()?;
        frame
            .is_valid()
            .then(|| (frame.width, frame.height, frame.bytes()))
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
        self.core.load_state(bytes)?;
        self.audio_scratch.clear();
        self.audio_cursor = 0;
        Ok(())
    }

    /// Index 0 is the quick slot, 1 to 10 are the numbered slots.
    pub fn state_path(&self, index: u8) -> Result<String, String> {
        let slot = storage::Slot::ALL
            .get(index as usize)
            .copied()
            .ok_or_else(|| format!("invalid state slot index {index}"))?;

        Ok(storage::entry_path(
            &self.meta.game_id(),
            storage::Entry::State(slot),
        ))
    }

    pub fn save_ram_path(&self) -> String {
        storage::entry_path(&self.meta.game_id(), storage::Entry::SaveRam)
    }

    pub fn save_ram(&self) -> Option<&[u8]> {
        self.core.save_ram()
    }

    pub fn load_save_ram(&mut self, bytes: &[u8]) -> bool {
        let restored = self.core.load_save_ram(bytes);
        self.backup_policy.restored(restored);
        restored
    }

    pub fn inhibit_save_ram_persistence(&self) {
        self.backup_policy.inhibit();
    }

    pub fn persistable_save_ram(&self) -> Option<&[u8]> {
        self.backup_policy
            .allow_persist()
            .then(|| self.core.save_ram())
            .flatten()
    }
}
