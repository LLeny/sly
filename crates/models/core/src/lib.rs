use std::sync::Arc;
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AudioSpec {
    pub sample_rate: u32,
    pub channels: u8,
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
    pub const ALL: [Button; 12] = [
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
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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

impl Region {
    pub fn tv_system(self) -> TvSystem {
        match self {
            Region::Japan
            | Region::NorthAmerica
            | Region::SouthKorea
            | Region::Canada
            | Region::Common
            | Region::Other(_) => TvSystem::Ntsc,
            _ => TvSystem::Pal,
        }
    }
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
        write!(f, "{name} ({})", self.tv_system())
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
    pub pixels: &'a [(u8, u8, u8)],
}

impl SoftwareFrame<'_> {
    pub fn is_valid(&self) -> bool {
        self.width > 0
            && self.height > 0
            && self.pixels.len() == self.width as usize * self.height as usize
    }
}

pub trait Core {
    fn reset(&mut self);
    fn step(&mut self);
    fn run_frame(&mut self);

    fn audio_spec(&self) -> Option<AudioSpec> {
        None
    }

    fn drain_audio(&mut self, _out: &mut Vec<i16>) {}

    fn set_button(&mut self, _player: usize, _button: Button, _pressed: bool) {}

    fn session_info(&self) -> SessionInfo {
        SessionInfo::default()
    }

    fn software_frame(&self) -> Option<SoftwareFrame<'_>> {
        None
    }

    fn frame_interval(&self) -> Option<Duration> {
        None
    }
}
