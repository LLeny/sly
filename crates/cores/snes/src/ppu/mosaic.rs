use crate::Snes;
use crate::macros::register_mmio;
use crate::mmio::{MemoryMap, MmioReg};
use serde::{Deserialize, Serialize};

register_mmio! {
    /// MOSAIC
    pub(crate) struct MosaicRegister @ 0x2106 {
        #[bits(0)]
        pub enable_bg1: bool,

        #[bits(1)]
        pub enable_bg2: bool,

        #[bits(2)]
        pub enable_bg3: bool,

        #[bits(3)]
        pub enable_bg4: bool,

        #[bits(4..=7)]
        pub size: u8,
    }
}

#[derive(Clone, Copy, Serialize, Deserialize)]
pub(crate) struct Mosaic {
    layers: u8,
    size: u8,
}

impl Default for Mosaic {
    fn default() -> Self {
        Self { layers: 0, size: 1 }
    }
}

impl Mosaic {
    fn write(&mut self, value: u8) {
        self.layers = value & 0x0F;
        self.size = (value >> 4) + 1;
    }

    fn raw(self) -> u8 {
        (self.size - 1) << 4 | self.layers
    }

    pub(crate) fn size_for_layer(self, layer: usize) -> u32 {
        if self.layers & (1 << layer) != 0 {
            self.size as u32
        } else {
            1
        }
    }
}

impl MmioReg for MosaicRegister {
    fn write<M: MemoryMap>(snes: &mut Snes<M>, value: u8) {
        snes.ppu.mosaic.write(value);
    }

    fn peek<M: MemoryMap>(snes: &Snes<M>) -> u8 {
        snes.ppu.mosaic.raw()
    }
}
