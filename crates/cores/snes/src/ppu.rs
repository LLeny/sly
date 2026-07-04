mod bg;
pub mod cgram;
pub(crate) mod regs;
pub mod tile;

use crate::ppu::cgram::Cgram;
use crate::ppu::tile::Tile;
use crate::scheduler::{MASTER_CLOCKS_PER_SCANLINE, Scheduler, VBLANK_START_SCANLINE};
use crate::{MemoryMap, Snes};

const HBLANK_START_CLOCK: u64 = 274 * 4;

pub struct Ppu {
    pub(crate) cgram: Cgram,
    bg_layers: [bg::BackgroundLayer; 4],
}

impl Ppu {
    pub fn new() -> Self {
        Self {
            cgram: Cgram::new(),
            bg_layers: [bg::BackgroundLayer::default(); 4],
        }
    }
}

impl Snes {
    pub fn palette(&self, index: usize) -> [u16; 16] {
        std::array::from_fn(|i| self.ppu.cgram.word((index * 16 + i) as u8))
    }

    pub fn tileset(&self, layer: usize) -> [Tile; 256] {
        let mut tiles = [Tile::default(); 256];

        let addr = self.ppu.bg_layers[layer].tileset_addr as u32;

        tiles
    }
}

pub(crate) fn vblank_start<M: MemoryMap + 'static>(snes: &mut Snes<M>, _deadline: u64) {
    snes.irq.set_vblank(true);

    if snes.irq.joypad_autoread_enabled() {
        snes.joypads.autoread();
    }
}

pub(crate) fn vblank_end<M: MemoryMap + 'static>(snes: &mut Snes<M>, _deadline: u64) {
    snes.irq.set_vblank(false);
}

pub(crate) fn in_vblank<M: MemoryMap>(scheduler: &Scheduler<M>) -> bool {
    scheduler.cycles() % scheduler.cycles_per_frame()
        >= VBLANK_START_SCANLINE * MASTER_CLOCKS_PER_SCANLINE
}

pub(crate) fn in_hblank<M: MemoryMap>(scheduler: &Scheduler<M>) -> bool {
    scheduler.cycles() % MASTER_CLOCKS_PER_SCANLINE >= HBLANK_START_CLOCK
}
