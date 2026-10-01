#[chapa::bitfield(u16, order = lsb0)]
pub struct DisplayStatus {
    #[bits(0)]
    pub vblank: bool,

    #[bits(1)]
    pub hblank: bool,

    #[bits(2)]
    pub vcounter: bool,

    #[bits(3)]
    pub vblank_irq: bool,

    #[bits(4)]
    pub hblank_irq: bool,

    #[bits(5)]
    pub vcounter_irq: bool,

    #[bits(8..=15)]
    pub vcount_compare: u8,
}

impl DisplayStatus {
    pub const WRITE_MASK: u16 = Self::VBLANK_IRQ_MASK
        | Self::HBLANK_IRQ_MASK
        | Self::VCOUNTER_IRQ_MASK
        | Self::VCOUNT_COMPARE_MASK;

    pub const READ_MASK: u16 =
        Self::WRITE_MASK | Self::VBLANK_MASK | Self::HBLANK_MASK | Self::VCOUNTER_MASK;
}

#[chapa::bitenum]
pub enum BackgroundMode {
    Mode0 = 0,
    Mode1 = 1,
    Mode2 = 2,
    Mode3 = 3,
    Mode4 = 4,
    Mode5 = 5,
    #[fallback]
    Unknown = 6,
}

impl std::fmt::Display for BackgroundMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            BackgroundMode::Mode0 => "Mode 0",
            BackgroundMode::Mode1 => "Mode 1",
            BackgroundMode::Mode2 => "Mode 2",
            BackgroundMode::Mode3 => "Mode 3",
            BackgroundMode::Mode4 => "Mode 4",
            BackgroundMode::Mode5 => "Mode 5",
            BackgroundMode::Unknown => "Unknown",
        };
        write!(f, "{}", s)
    }
}

#[chapa::bitenum]
pub enum DisplayFrame {
    #[fallback]
    Frame0 = 0,
    Frame1 = 1,
}

impl DisplayFrame {
    pub(crate) const fn vram_offset(self) -> usize {
        match self {
            Self::Frame0 => 0,
            Self::Frame1 => 0xA000,
        }
    }
}

impl std::fmt::Display for DisplayFrame {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            DisplayFrame::Frame0 => "Frame 0",
            DisplayFrame::Frame1 => "Frame 1",
        };
        write!(f, "{}", s)
    }
}

#[chapa::bitenum]
pub enum SpriteMapping {
    #[fallback]
    OneDimensional = 0,
    TwoDimensional = 1,
}

#[chapa::bitfield(u16, order = lsb0)]
pub struct DisplayControl {
    #[bits(0..=2)]
    pub mode: BackgroundMode,

    #[bits(3)]
    pub cgb_mode: bool,

    #[bits(4)]
    pub frame_select: DisplayFrame,

    #[bits(5)]
    pub hblank_interval_free: bool,

    #[bits(6)]
    pub sprite_mapping: SpriteMapping,

    #[bits(7)]
    pub forced_blank: bool,

    #[bits(8)]
    pub enable_bg0: bool,

    #[bits(9)]
    pub enable_bg1: bool,

    #[bits(10)]
    pub enable_bg2: bool,

    #[bits(11)]
    pub enable_bg3: bool,

    #[bits(12)]
    pub enable_obj: bool,

    #[bits(13)]
    pub enable_window0: bool,

    #[bits(14)]
    pub enable_window1: bool,

    #[bits(15)]
    pub enable_obj_window: bool,
}

#[chapa::bitfield(u16, order = lsb0)]
pub struct VerticalCount {
    #[bits(0..=7)]
    pub scanline: u8,
}
