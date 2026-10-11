use crate::ppu::tile::{ColorDepth, Size, TextMapEntry};

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

impl BackgroundKind {
    /// Tiles a map entry can address: 10-bit text entries, 8-bit affine entries.
    pub fn tile_count(self) -> Option<usize> {
        match self {
            Self::Text => Some(1024),
            Self::Affine => Some(256),
            Self::Bitmap | Self::Unavailable => None,
        }
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
#[derive(PartialEq, Eq)]
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
    pub bg0_enabled: bool,

    #[bits(9)]
    pub bg1_enabled: bool,

    #[bits(10)]
    pub bg2_enabled: bool,

    #[bits(11)]
    pub bg3_enabled: bool,

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
    pub fn bg_enabled(self, index: usize) -> bool {
        match index {
            0 => self.bg0_enabled(),
            1 => self.bg1_enabled(),
            2 => self.bg2_enabled(),
            3 => self.bg3_enabled(),
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
        super::VRAM_BASE + self.tileset_offset() as u32
    }

    pub fn tileset_offset(self) -> usize {
        usize::from(self.tileset_base()) * 0x4000
    }

    pub fn tilemap_address(self) -> u32 {
        super::VRAM_BASE + self.tilemap_offset() as u32
    }

    pub fn tilemap_offset(self) -> usize {
        usize::from(self.tilemap_base()) * 0x800
    }

    pub fn text_entry_offset(self, tile_x: usize, tile_y: usize, width: usize) -> usize {
        let block = ((tile_y / 32) * (width / 256)) + (tile_x / 32);
        let index = ((tile_y % 32) * 32) + (tile_x % 32);
        self.tilemap_offset() + (block * 0x800) + (index * 2)
    }

    pub fn map_entry(
        self,
        kind: BackgroundKind,
        vram: &[u8],
        tile_x: usize,
        tile_y: usize,
        width: usize,
    ) -> Option<(usize, TextMapEntry)> {
        match kind {
            BackgroundKind::Text => {
                let addr = self.text_entry_offset(tile_x, tile_y, width);
                let entry = TextMapEntry::from_le_bytes([vram[addr], vram[addr + 1]]);
                Some((addr, entry))
            }
            BackgroundKind::Affine => {
                let addr = self.tilemap_offset() + (tile_y * (width / 8)) + tile_x;
                let entry = TextMapEntry::zeroed().with_tile_number(u16::from(vram[addr]));
                Some((addr, entry))
            }
            BackgroundKind::Bitmap | BackgroundKind::Unavailable => None,
        }
    }

    pub fn tile_offset(self, tile_number: usize, depth: ColorDepth) -> usize {
        self.tileset_offset() + (tile_number * depth.tile_size())
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

#[chapa::bitfield(u16, order = lsb0)]
pub struct BackgroundOffset {
    #[bits(0..=8)]
    pub offset: u16,
}

#[chapa::bitfield(u16, order = lsb0)]
#[derive(PartialEq, Eq)]
pub struct WindowXOffset {
    #[bits(0..=7)]
    pub x2: u16,
    #[bits(8..=15)]
    pub x1: u16,
}

#[chapa::bitfield(u16, order = lsb0)]
#[derive(PartialEq, Eq)]
pub struct WindowYOffset {
    #[bits(0..=7)]
    pub y2: u16,
    #[bits(8..=15)]
    pub y1: u16,
}

#[chapa::bitfield(u16, order = lsb0)]
#[derive(PartialEq, Eq)]
pub struct WindowControl {
    #[bits(0, overlay = "winin")]
    pub win0_bg0_enable: bool,
    #[bits(1, overlay = "winin")]
    pub win0_bg1_enable: bool,
    #[bits(2, overlay = "winin")]
    pub win0_bg2_enable: bool,
    #[bits(3, overlay = "winin")]
    pub win0_bg3_enable: bool,
    #[bits(4, overlay = "winin")]
    pub win0_obj_enable: bool,
    #[bits(5, overlay = "winin")]
    pub win0_sfx_enable: bool,

    #[bits(8, overlay = "winin")]
    pub win1_bg0_enable: bool,
    #[bits(9, overlay = "winin")]
    pub win1_bg1_enable: bool,
    #[bits(10, overlay = "winin")]
    pub win1_bg2_enable: bool,
    #[bits(11, overlay = "winin")]
    pub win1_bg3_enable: bool,
    #[bits(12, overlay = "winin")]
    pub win1_obj_enable: bool,
    #[bits(13, overlay = "winin")]
    pub win1_sfx_enable: bool,

    #[bits(0, overlay = "winout")]
    pub outside_bg0_enable: bool,
    #[bits(1, overlay = "winout")]
    pub outside_bg1_enable: bool,
    #[bits(2, overlay = "winout")]
    pub outside_bg2_enable: bool,
    #[bits(3, overlay = "winout")]
    pub outside_bg3_enable: bool,
    #[bits(4, overlay = "winout")]
    pub outside_obj_enable: bool,
    #[bits(5, overlay = "winout")]
    pub outside_sfx_enable: bool,

    #[bits(8, overlay = "winout")]
    pub obj_win_bg0_enable: bool,
    #[bits(9, overlay = "winout")]
    pub obj_win_bg1_enable: bool,
    #[bits(10, overlay = "winout")]
    pub obj_win_bg2_enable: bool,
    #[bits(11, overlay = "winout")]
    pub obj_win_bg3_enable: bool,
    #[bits(12, overlay = "winout")]
    pub obj_win_obj_enable: bool,
    #[bits(13, overlay = "winout")]
    pub obj_win_sfx_enable: bool,
}

#[chapa::bitenum]
pub enum SpecialEffect {
    #[fallback]
    None = 0,
    AlphaBlend = 1,
    BrightnessIncrease = 2,
    BrightnessDecrease = 3,
}

#[chapa::bitfield(u16, order = lsb0)]
pub struct BlendControl {
    #[bits(0)]
    pub bg0_first_target: bool,

    #[bits(1)]
    pub bg1_first_target: bool,

    #[bits(2)]
    pub bg2_first_target: bool,

    #[bits(3)]
    pub bg3_first_target: bool,

    #[bits(4)]
    pub obj_first_target: bool,

    #[bits(5)]
    pub backdrop_first_target: bool,

    #[bits(6..=7)]
    pub special_effect: SpecialEffect,

    #[bits(8)]
    pub bg0_second_target: bool,

    #[bits(9)]
    pub bg1_second_target: bool,

    #[bits(10)]
    pub bg2_second_target: bool,

    #[bits(11)]
    pub bg3_second_target: bool,

    #[bits(12)]
    pub obj_second_target: bool,

    #[bits(13)]
    pub backdrop_second_target: bool,
}

pub(crate) const OBJ_LAYER: usize = 4;
pub(crate) const BACKDROP_LAYER: usize = 5;

impl BlendControl {
    pub const WRITE_MASK: u16 = 0x3FFF;

    pub(crate) fn first_target(self, layer: usize) -> bool {
        layer <= BACKDROP_LAYER && (self.raw() >> layer) & 1 != 0
    }

    pub(crate) fn second_target(self, layer: usize) -> bool {
        layer <= BACKDROP_LAYER && (self.raw() >> (8 + layer)) & 1 != 0
    }
}

#[chapa::bitfield(u16, order = lsb0)]
pub struct BlendAlpha {
    #[bits(0..=4)]
    pub eva: u8,

    #[bits(8..=12)]
    pub evb: u8,
}

impl BlendAlpha {
    pub const WRITE_MASK: u16 = Self::EVA_MASK | Self::EVB_MASK;
}

#[chapa::bitfield(u16, order = lsb0)]
pub struct BlendBrightness {
    #[bits(0..=4)]
    pub ev: u8,
}

impl BlendBrightness {
    pub const WRITE_MASK: u16 = Self::EV_MASK;
}
