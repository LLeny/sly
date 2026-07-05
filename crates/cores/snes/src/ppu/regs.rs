use chapa::BitEnum;

use crate::Snes;
use crate::macros::{register_mmio, register_mmio_family, register_mmio_raw};
use crate::mmio::{MemoryMap, MmioReg};

pub(crate) const CGADD: u16 = 0x2121;
pub(crate) const CGDATA: u16 = 0x2122;
pub(crate) const RDCGRAM: u16 = 0x213B;

// TODO
pub(crate) const CGWSEL: u16 = 0x2130;
pub(crate) const SETINI: u16 = 0x2133;

pub(crate) const VMADDL: u16 = 0x2116;
pub(crate) const VMADDH: u16 = 0x2117;
pub(crate) const VMDATAL: u16 = 0x2118;
pub(crate) const VMDATAH: u16 = 0x2119;
pub(crate) const RDVRAML: u16 = 0x2139;
pub(crate) const RDVRAMH: u16 = 0x213A;

register_mmio_raw! {
    CGADD => "CGADD",
    CGDATA => "CGDATA",
    RDCGRAM => "RDCGRAM",
    VMADDL => "VMADDL",
    VMADDH => "VMADDH",
    VMDATAL => "VMDATAL",
    VMDATAH => "VMDATAH",
    RDVRAML => "RDVRAML",
    RDVRAMH => "RDVRAMH",
    0x210D => "BG1HOFS",
    0x210E => "BG1VOFS",
    0x210F => "BG2HOFS",
    0x2110 => "BG2VOFS",
    0x2111 => "BG3HOFS",
    0x2112 => "BG3VOFS",
    0x2113 => "BG4HOFS",
    0x2114 => "BG4VOFS",
}

pub(crate) struct BgScrollOffset;

impl BgScrollOffset {
    pub(crate) const START: u16 = 0x210D; // BG1HOFS
    pub(crate) const END: u16 = 0x2114; // BG4VOFS

    pub(crate) fn write<M: MemoryMap>(snes: &mut Snes<M>, reg: u16, value: u8) {
        let index = (reg - Self::START) as usize;
        let layer = &mut snes.ppu.bg_layers[index / 2];

        let prev = snes.ppu.bgofs_latch.exchange(value);
        if index % 2 == 0 {
            let prev_h = snes.ppu.hofs_latch.exchange(value);
            layer.hofs = (value as u16) << 8 | (prev & !7) as u16 | (prev_h & 7) as u16;
        } else {
            layer.vofs = (value as u16) << 8 | prev as u16;
        }
    }
}

#[derive(BitEnum, Clone, Copy, Default)]
pub enum IncrementMode {
    #[fallback]
    #[default]
    AfterLow = 0,
    AfterHigh = 1,
}

#[derive(BitEnum, Clone, Copy, Default)]
pub enum IncrementAmount {
    #[default]
    One = 0,
    ThirtyTwo = 1,
    #[fallback]
    OneTwentyEight = 2,
}

impl IncrementAmount {
    pub fn words(self) -> u16 {
        match self {
            Self::One => 1,
            Self::ThirtyTwo => 32,
            Self::OneTwentyEight => 128,
        }
    }
}

#[derive(BitEnum, Clone, Copy, Default)]
pub enum AddressRemapping {
    #[fallback]
    #[default]
    None = 0,
    Bpp2 = 1,
    Bpp4 = 2,
    Bpp8 = 3,
}

impl AddressRemapping {
    pub fn apply(self, addr: u16) -> u16 {
        match self {
            Self::None => addr,
            Self::Bpp2 => (addr & 0xFF00) | ((addr << 3) & 0x00F8) | ((addr >> 5) & 0x0007),
            Self::Bpp4 => (addr & 0xFE00) | ((addr << 3) & 0x01F8) | ((addr >> 6) & 0x0007),
            Self::Bpp8 => (addr & 0xFC00) | ((addr << 3) & 0x03F8) | ((addr >> 7) & 0x0007),
        }
    }
}

register_mmio! {
    /// VMAIN
    pub struct VramControl @ 0x2115 {
        #[bits(0..=1)]
        pub increment_amount: IncrementAmount,

        #[bits(2..=3)]
        pub address_remapping: AddressRemapping,

        #[bits(7)]
        pub increment_mode: IncrementMode,
    }
}

register_mmio! {
    /// BG12NBA
    pub struct Bg12ChrBase @ 0x210B {
        #[bits(0..=3)]
        pub bg1: u8,

        #[bits(4..=7)]
        pub bg2: u8,
    }
}

register_mmio! {
    /// BG34NBA
    pub struct Bg34ChrBase @ 0x210C {
        #[bits(0..=3)]
        pub bg3: u8,

        #[bits(4..=7)]
        pub bg4: u8,
    }
}

#[derive(BitEnum, Clone, Copy)]
pub enum BackgroundMode {
    Mode0 = 0,
    Mode1 = 1,
    Mode2 = 2,
    Mode3 = 3,
    Mode4 = 4,
    Mode5 = 5,
    Mode6 = 6,
    Mode7 = 7,
    #[fallback]
    Invalid,
}

impl BackgroundMode {
    pub fn layer_bpp(self) -> [Option<u8>; 4] {
        match self {
            Self::Mode0 => [Some(2), Some(2), Some(2), Some(2)],
            Self::Mode1 => [Some(4), Some(4), Some(2), None],
            Self::Mode2 => [Some(4), Some(4), None, None],
            Self::Mode3 => [Some(8), Some(4), None, None],
            Self::Mode4 => [Some(8), Some(2), None, None],
            Self::Mode5 => [Some(4), Some(2), None, None],
            Self::Mode6 => [Some(4), None, None, None],
            Self::Mode7 => [Some(8), None, None, None],
            Self::Invalid => [None; 4],
        }
    }

    pub fn bpp(self, layer: usize) -> Option<u8> {
        self.layer_bpp().get(layer).copied().flatten()
    }
}

#[derive(BitEnum, Clone, Copy, Default)]
pub enum Priority {
    #[fallback]
    #[default]
    Normal = 0,
    High = 1,
}

#[derive(BitEnum, Clone, Copy, Default)]
pub enum TileSize {
    #[fallback]
    #[default]
    Size8x8 = 0,
    Size16x16 = 1,
}

impl TileSize {
    pub fn pixels(self) -> u32 {
        match self {
            Self::Size8x8 => 8,
            Self::Size16x16 => 16,
        }
    }
}

register_mmio! {
    /// BGMODE
    pub struct BgMode @ 0x2105 {
        #[bits(0..=2)]
        pub mode: BackgroundMode,

        #[bits(3)]
        pub mode1_bg3_priority: Priority,

        #[bits(4)]
        pub bg1_tile_size: TileSize,

        #[bits(5)]
        pub bg2_tile_size: TileSize,

        #[bits(6)]
        pub bg3_tile_size: TileSize,

        #[bits(7)]
        pub bg4_tile_size: TileSize,
    }
}

#[derive(BitEnum, Clone, Copy, Default)]
pub enum TilemapCount {
    #[fallback]
    #[default]
    One = 0,
    Two = 1,
}

impl TilemapCount {
    pub fn count(self) -> u8 {
        match self {
            Self::One => 1,
            Self::Two => 2,
        }
    }
}

register_mmio_family! {
    /// BGnSC
    pub struct TilemapAddrAndSize {
        #[bits(0)]
        pub horizontal_tilemap_count: TilemapCount,

        #[bits(1)]
        pub vertical_tilemap_count: TilemapCount,

        #[bits(2..=7)]
        pub tilemap_addr: u8,
    }
    symbols {
        0x2107 => "BG1SC",
        0x2108 => "BG2SC",
        0x2109 => "BG3SC",
        0x210A => "BG4SC",
    }
}

impl TilemapAddrAndSize {
    pub fn address(&self) -> u32 {
        (self.tilemap_addr() as u32) << 10
    }

    pub(crate) const START: u16 = 0x2107;
    pub(crate) const END: u16 = 0x210A;

    pub(crate) fn write<M: MemoryMap>(snes: &mut Snes<M>, addr: u16, value: u8) {
        let reg = Self::from_raw(value);
        let layer = &mut snes.ppu.bg_layers[(addr - Self::START) as usize];
        layer.tilemap_addr = reg.address();
        layer.horizontal_tilemap_count = reg.horizontal_tilemap_count();
        layer.vertical_tilemap_count = reg.vertical_tilemap_count();
    }

    pub(crate) fn peek<M: MemoryMap>(snes: &Snes<M>, addr: u16) -> u8 {
        let layer = &snes.ppu.bg_layers[(addr - Self::START) as usize];
        let mut reg = Self::from_raw(0);
        reg.set_tilemap_addr((layer.tilemap_addr >> 10) as u8);
        reg.set_horizontal_tilemap_count(layer.horizontal_tilemap_count);
        reg.set_vertical_tilemap_count(layer.vertical_tilemap_count);
        reg.raw()
    }
}

impl MmioReg for BgMode {
    fn write<M: MemoryMap>(snes: &mut Snes<M>, value: u8) {
        let reg = Self::from_raw(value);
        snes.ppu.bg_mode = reg.mode();
        snes.ppu.bg3_priority = reg.mode1_bg3_priority();
        snes.ppu.bg_layers[0].tile_size = reg.bg1_tile_size();
        snes.ppu.bg_layers[1].tile_size = reg.bg2_tile_size();
        snes.ppu.bg_layers[2].tile_size = reg.bg3_tile_size();
        snes.ppu.bg_layers[3].tile_size = reg.bg4_tile_size();
    }

    fn peek<M: MemoryMap>(snes: &Snes<M>) -> u8 {
        let mut reg = Self::default();
        reg.set_mode(snes.ppu.bg_mode);
        reg.set_mode1_bg3_priority(snes.ppu.bg3_priority);
        reg.set_bg1_tile_size(snes.ppu.bg_layers[0].tile_size);
        reg.set_bg2_tile_size(snes.ppu.bg_layers[1].tile_size);
        reg.set_bg3_tile_size(snes.ppu.bg_layers[2].tile_size);
        reg.set_bg4_tile_size(snes.ppu.bg_layers[3].tile_size);
        reg.raw()
    }
}

impl MmioReg for Bg12ChrBase {
    fn write<M: MemoryMap>(snes: &mut Snes<M>, value: u8) {
        let reg = Self::from_raw(value);
        snes.ppu.bg_layers[0].tileset_addr = (reg.bg1() as u32) << 12;
        snes.ppu.bg_layers[1].tileset_addr = (reg.bg2() as u32) << 12;
    }

    fn peek<M: MemoryMap>(snes: &Snes<M>) -> u8 {
        let mut reg = Self::default();
        reg.set_bg1((snes.ppu.bg_layers[0].tileset_addr >> 12) as u8);
        reg.set_bg2((snes.ppu.bg_layers[1].tileset_addr >> 12) as u8);
        reg.raw()
    }
}

impl MmioReg for Bg34ChrBase {
    fn write<M: MemoryMap>(snes: &mut Snes<M>, value: u8) {
        let reg = Self::from_raw(value);
        snes.ppu.bg_layers[2].tileset_addr = (reg.bg3() as u32) << 12;
        snes.ppu.bg_layers[3].tileset_addr = (reg.bg4() as u32) << 12;
    }

    fn peek<M: MemoryMap>(snes: &Snes<M>) -> u8 {
        let mut reg = Self::default();
        reg.set_bg3((snes.ppu.bg_layers[2].tileset_addr >> 12) as u8);
        reg.set_bg4((snes.ppu.bg_layers[3].tileset_addr >> 12) as u8);
        reg.raw()
    }
}
