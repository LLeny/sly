use chapa::BitEnum;

use crate::Snes;
use crate::macros::{register_mmio, register_mmio_raw};
use crate::mmio::{MemoryMap, MmioReg};

pub(crate) const CGADD: u16 = 0x2121;
pub(crate) const CGDATA: u16 = 0x2122;
pub(crate) const RDCGRAM: u16 = 0x213B;

// TODO
pub(crate) const CGWSEL: u16 = 0x2130;
pub(crate) const SETINI: u16 = 0x2133;

pub(crate) const VMAIN: u16 = 0x2115;
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
    VMAIN => "VMAIN",
    VMADDL => "VMADDL",
    VMADDH => "VMADDH",
    VMDATAL => "VMDATAL",
    VMDATAH => "VMDATAH",
    RDVRAML => "RDVRAML",
    RDVRAMH => "RDVRAMH",
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
