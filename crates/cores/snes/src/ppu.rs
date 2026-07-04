pub(crate) mod bg;
pub mod cgram;
pub(crate) mod regs;
pub mod tile;
pub mod vram;

use crate::ppu::cgram::Cgram;
use crate::ppu::regs::{BackgroundMode, Priority};
use crate::ppu::tile::Tile;
use crate::ppu::vram::Vram;
use crate::scheduler::{MASTER_CLOCKS_PER_SCANLINE, Scheduler, VBLANK_START_SCANLINE};
use crate::{MemoryMap, Snes};

const HBLANK_START_CLOCK: u64 = 274 * 4;

pub struct Ppu {
    pub(crate) cgram: Cgram,
    pub(crate) vram: Vram,
    pub(crate) bg_layers: [bg::BackgroundLayer; 4],
    pub(crate) bg_mode: BackgroundMode,
    pub(crate) bg3_priority: Priority,
}

impl Ppu {
    pub fn new() -> Self {
        Self {
            cgram: Cgram::new(),
            vram: Vram::new(),
            bg_layers: [bg::BackgroundLayer::default(); 4],
            bg_mode: BackgroundMode::Mode0,
            bg3_priority: Priority::Normal,
        }
    }
}

pub struct TilesetData {
    pub bpp: u8,
    pub base_addr: u32,
    pub tiles: Vec<Tile>,
}

impl TilesetData {
    pub fn tile_addr(&self, index: usize) -> u32 {
        self.base_addr + index as u32 * self.bpp as u32 * 4
    }
}

impl Snes {
    pub fn palette(&self, index: usize) -> [u16; 16] {
        std::array::from_fn(|i| self.ppu.cgram.word((index * 16 + i) as u8))
    }

    pub fn tileset(&self, layer: usize) -> Option<TilesetData> {
        let bpp = self.ppu.bg_mode.bpp(layer)?;
        let bytes_per_tile = bpp as u32 * 8;

        let base_addr = self.ppu.bg_layers[layer].tileset_addr;
        let base = base_addr * 2;

        let tiles = (0..256)
            .map(|i| {
                let addr = base + i * bytes_per_tile;

                let mut tile = Tile::default();
                for (j, byte) in tile.data[..bytes_per_tile as usize].iter_mut().enumerate() {
                    *byte = self.ppu.vram.byte(addr + j as u32);
                }

                tile
            })
            .collect();

        Some(TilesetData {
            bpp,
            base_addr,
            tiles,
        })
    }

    pub fn bg_palette(&self, layer: usize, bpp: u8, palette: usize) -> (usize, usize) {
        match bpp {
            2 => {
                let base = match self.ppu.bg_mode {
                    BackgroundMode::Mode0 => layer * 32,
                    _ => 0,
                };
                (base + palette * 4, 4)
            }
            4 => (palette * 16, 16),
            _ => (0, 256),
        }
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
