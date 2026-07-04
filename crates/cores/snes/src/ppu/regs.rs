use crate::Snes;
use crate::macros::{register_mmio, register_mmio_raw};
use crate::mmio::{MemoryMap, MmioReg};

pub(crate) const CGADD: u16 = 0x2121;
pub(crate) const CGDATA: u16 = 0x2122;
pub(crate) const RDCGRAM: u16 = 0x213B;

register_mmio_raw! {
    CGADD => "CGADD",
    CGDATA => "CGDATA",
    RDCGRAM => "RDCGRAM",
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
