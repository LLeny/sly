mod cartridge;
mod input;
mod metadata;
pub mod state;

use std::sync::Arc;
use std::time::Duration;

pub use cartridge::{Cartridge, RomFile};
pub use input::ControllerDescriptor;
pub use metadata::{
    BackupKind, BackupMemory, CartridgeInfo, DisplayRotation, GameId, RomIdentity, format_byte_size,
};
pub use state::StateError;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CartridgeError {
    #[error("invalid ROM: {0}")]
    InvalidRom(String),
    #[error("cartridge is not compatible with this core")]
    Incompatible,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CoreError {
    #[error(transparent)]
    Cartridge(#[from] CartridgeError),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AudioSpec {
    pub sample_rate: u32,
    pub channels: u8,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum System {
    Snes,
    Gba,
    Lynx,
}

impl System {
    pub const ALL: [System; 3] = [System::Snes, System::Gba, System::Lynx];

    pub const fn slug(self) -> &'static str {
        match self {
            System::Snes => "snes",
            System::Gba => "gba",
            System::Lynx => "lynx",
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            System::Snes => "SNES",
            System::Gba => "GBA",
            System::Lynx => "Lynx",
        }
    }

    pub const fn qualified_name(self) -> &'static str {
        match self {
            System::Snes => "Super Nintendo Entertainment System",
            System::Gba => "Game Boy Advance",
            System::Lynx => "Atari Lynx",
        }
    }

    pub const fn rom_extensions(self) -> &'static [&'static str] {
        match self {
            System::Snes => &["sfc", "smc"],
            System::Gba => &["gba"],
            System::Lynx => &["lnx", "o"],
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Button {
    Up,
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

impl Button {
    /// This order is part of the C ABI. New buttons can only be appended to `ALL`.
    pub const CLASSIC: [Button; 12] = [
        Button::Up,
        Button::Down,
        Button::Left,
        Button::Right,
        Button::A,
        Button::B,
        Button::X,
        Button::Y,
        Button::L,
        Button::R,
        Button::Start,
        Button::Select,
    ];

    pub const ALL: [Button; 12] = Self::CLASSIC;
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ExecutionState {
    #[default]
    Paused,
    Running,
}

impl ExecutionState {
    pub const fn is_running(self) -> bool {
        matches!(self, Self::Running)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TvSystem {
    Ntsc,
    Pal,
}

impl std::fmt::Display for TvSystem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            TvSystem::Ntsc => "NTSC",
            TvSystem::Pal => "PAL",
        })
    }
}

/// The market a game was released for, as declared by its ROM/disc metadata.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Region {
    Japan,
    NorthAmerica,
    Europe,
    Scandinavia,
    Finland,
    Denmark,
    France,
    Netherlands,
    Spain,
    Germany,
    Italy,
    China,
    Indonesia,
    SouthKorea,
    Common,
    Canada,
    Brazil,
    Australia,
    Other(u8),
}

impl std::fmt::Display for Region {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = match self {
            Region::Japan => "Japan",
            Region::NorthAmerica => "North America",
            Region::Europe => "Europe",
            Region::Scandinavia => "Scandinavia",
            Region::Finland => "Finland",
            Region::Denmark => "Denmark",
            Region::France => "France",
            Region::Netherlands => "Netherlands",
            Region::Spain => "Spain",
            Region::Germany => "Germany",
            Region::Italy => "Italy",
            Region::China => "China",
            Region::Indonesia => "Indonesia",
            Region::SouthKorea => "South Korea",
            Region::Common => "Common",
            Region::Canada => "Canada",
            Region::Brazil => "Brazil",
            Region::Australia => "Australia",
            Region::Other(code) => return write!(f, "Region ${code:02X}"),
        };

        f.write_str(name)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SessionInfo {
    pub system: Arc<str>,
    pub session: Arc<str>,
    pub segments: Vec<Arc<str>>,
}

#[derive(Clone, Copy, Debug)]
pub struct SoftwareFrame<'a> {
    pub width: u32,
    pub height: u32,
    pub pixels: &'a [[u8; 4]],
}

impl<'a> SoftwareFrame<'a> {
    pub fn is_valid(&self) -> bool {
        self.width > 0
            && self.height > 0
            && self.pixels.len() == self.width as usize * self.height as usize
    }

    pub fn bytes(self) -> &'a [u8] {
        self.pixels.as_flattened()
    }

    pub fn write_rgb8(self, out: &mut Vec<u8>) {
        out.clear();
        out.reserve(self.pixels.len() * 3);
        for &[r, g, b, _] in self.pixels {
            out.extend_from_slice(&[r, g, b]);
        }
    }
}

pub trait Core {
    fn load_cartridge(&mut self, rom: RomFile) -> Result<(), CoreError>;

    fn reset(&mut self);
    fn step(&mut self);
    fn run_frame(&mut self);

    fn audio_spec(&self) -> Option<AudioSpec> {
        None
    }

    fn drain_audio(&mut self, _out: &mut Vec<i16>) {}

    fn set_button(&mut self, _player: usize, _button: Button, _pressed: bool) {}

    fn controllers(&self) -> Vec<ControllerDescriptor> {
        Vec::new()
    }

    fn cartridge_info(&self) -> Option<CartridgeInfo> {
        None
    }

    fn session_info(&self) -> SessionInfo {
        SessionInfo::default()
    }

    fn software_frame(&self) -> Option<SoftwareFrame<'_>> {
        None
    }

    fn display_aspect(&self) -> Option<f32> {
        None
    }

    fn frame_interval(&self) -> Option<Duration> {
        None
    }

    fn save_state(&self) -> Result<Vec<u8>, StateError> {
        Err(StateError::Unsupported)
    }

    fn load_state(&mut self, _bytes: &[u8]) -> Result<(), StateError> {
        Err(StateError::Unsupported)
    }

    fn save_ram(&self) -> Option<&[u8]> {
        None
    }

    fn load_save_ram(&mut self, _bytes: &[u8]) -> bool {
        false
    }
}
