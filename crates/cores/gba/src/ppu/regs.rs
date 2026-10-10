use crate::ppu::tile::{ColorDepth, Size};

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
#[derive(Debug, PartialEq, Eq)]
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

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum BackgroundKind {
    Text,
    Affine,
    Bitmap,
    Unavailable,
}

impl std::fmt::Display for BackgroundKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Text => "Text",
            Self::Affine => "Affine",
            Self::Bitmap => "Bitmap",
            Self::Unavailable => "Unavailable",
        })
    }
}

impl BackgroundMode {
    pub fn background_kind(self, index: usize) -> BackgroundKind {
        match (self, index) {
            (Self::Mode0, 0..=3) | (Self::Mode1, 0 | 1) => BackgroundKind::Text,
            (Self::Mode1, 2) | (Self::Mode2, 2 | 3) => BackgroundKind::Affine,
            (Self::Mode3 | Self::Mode4 | Self::Mode5, 2) => BackgroundKind::Bitmap,
            _ => BackgroundKind::Unavailable,
        }
    }
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
    TwoDimensional = 0,
    OneDimensional = 1,
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

#[chapa::bitfield(u16, order = lsb0)]
pub struct BackgroundControl {
    #[bits(0..=1)]
    pub priority: u8,

    #[bits(2..=3)]
    pub tileset_base: u8,

    #[bits(4..=5)]
    pub reserved: u8,

    #[bits(6)]
    pub mosaic: bool,

    #[bits(7)]
    pub color_depth: ColorDepth,

    #[bits(8..=12)]
    pub tilemap_base: u8,

    #[bits(13)]
    pub wraparound: bool,

    #[bits(14..=15)]
    pub screen_size: Size,
}

impl DisplayControl {
    pub fn enable_bg(self, index: usize) -> bool {
        match index {
            0 => self.enable_bg0(),
            1 => self.enable_bg1(),
            2 => self.enable_bg2(),
            3 => self.enable_bg3(),
            _ => false,
        }
    }
}

impl BackgroundControl {
    pub const WRITE_MASK: u16 = Self::PRIORITY_MASK
        | Self::TILESET_BASE_MASK
        | Self::MOSAIC_MASK
        | Self::COLOR_DEPTH_MASK
        | Self::TILEMAP_BASE_MASK
        | Self::WRAPAROUND_MASK
        | Self::SCREEN_SIZE_MASK;

    pub fn tileset_address(self) -> u32 {
        super::VRAM_BASE + u32::from(self.tileset_base()) * 0x4000
    }

    pub fn tilemap_address(self) -> u32 {
        super::VRAM_BASE + u32::from(self.tilemap_base()) * 0x800
    }

    pub fn dimensions(self, kind: BackgroundKind) -> Option<(usize, usize)> {
        match kind {
            BackgroundKind::Text => self.screen_size().text_dimensions(),
            BackgroundKind::Affine => self.screen_size().affine_dimensions(),
            BackgroundKind::Bitmap | BackgroundKind::Unavailable => None,
        }
    }

    pub fn effective_color_depth(self, kind: BackgroundKind) -> ColorDepth {
        match kind {
            BackgroundKind::Affine => ColorDepth::Bpp8,
            _ => self.color_depth(),
        }
    }
}
