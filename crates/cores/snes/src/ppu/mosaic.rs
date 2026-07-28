use crate::Snes;
use crate::macros::register_mmio;
use crate::mmio::{MemoryMap, MmioReg};
use serde::{Deserialize, Serialize};

register_mmio! {
    /// MOSAIC
    pub(crate) struct MosaicRegister @ 0x2106 {
        #[bits(0, overlay = "layers")]
        pub enable_bg1: bool,

        #[bits(1, overlay = "layers")]
        pub enable_bg2: bool,

        #[bits(2, overlay = "layers")]
        pub enable_bg3: bool,

        #[bits(3, overlay = "layers")]
        pub enable_bg4: bool,

        #[bits(0..=3, overlay = "mask")]
        pub enabled_mask: u8,

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
        let reg = MosaicRegister::from_raw(value);
        self.layers = reg.enabled_mask();
        self.size = reg.size() + 1;
    }

    fn raw(self) -> u8 {
        MosaicRegister::default()
            .with_enabled_mask(self.layers)
            .with_size(self.size - 1)
            .raw()
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
