pub(crate) mod bg;
pub mod cgram;
pub(crate) mod latch;
pub(crate) mod regs;
pub mod tile;
pub mod vram;

use crate::ppu::cgram::Cgram;
use crate::ppu::latch::ByteLatch;
use crate::ppu::regs::{BackgroundMode, Priority};
use crate::ppu::tile::Tile;
use crate::ppu::vram::Vram;
use crate::scheduler::{MASTER_CLOCKS_PER_SCANLINE, Scheduler, VBLANK_START_SCANLINE};
use crate::{MemoryMap, Snes};

const HBLANK_START_CLOCK: u64 = 274 * 4;

// TODO
pub const SCREEN_WIDTH: u32 = 256;
pub const SCREEN_HEIGHT: u32 = 224;

pub struct Ppu {
    pub(crate) cgram: Cgram,
    pub(crate) vram: Vram,
    pub(crate) bg_layers: [bg::BackgroundLayer; 4],
    pub(crate) bg_mode: BackgroundMode,
    pub(crate) bg3_priority: Priority,
    pub(crate) bgofs_latch: ByteLatch,
    pub(crate) hofs_latch: ByteLatch,
    pub(crate) frame_buffer: Vec<(u8, u8, u8)>,
}

impl Ppu {
    pub fn new() -> Self {
        Self {
            cgram: Cgram::new(),
            vram: Vram::new(),
            bg_layers: [bg::BackgroundLayer::default(); 4],
            bg_mode: BackgroundMode::Mode0,
            bg3_priority: Priority::Normal,
            bgofs_latch: ByteLatch::default(),
            hofs_latch: ByteLatch::default(),
            frame_buffer: vec![(0, 0, 0); SCREEN_WIDTH as usize * SCREEN_HEIGHT as usize],
        }
    }

    // Hacked for SMW
    pub fn render_scanline(&mut self, scanline: usize) {
        let order = match (self.bg_mode, self.bg3_priority) {
            (BackgroundMode::Mode1, Priority::High) => [2, 0, 1],
            _ => [0, 1, 2],
        };

        let mut line = [None; SCREEN_WIDTH as usize];
        for layer in order {
            self.render_bg_scanline(layer, scanline as u32, &mut line);
        }

        let backdrop = cgram::rgb(self.cgram.word(0));
        let row =
            &mut self.frame_buffer[scanline * SCREEN_WIDTH as usize..][..SCREEN_WIDTH as usize];
        for (dst, pixel) in row.iter_mut().zip(line) {
            *dst = pixel.unwrap_or(backdrop);
        }
    }

    fn render_bg_scanline(&self, layer: usize, y: u32, line: &mut [Option<(u8, u8, u8)>]) {
        let (Some(bpp), Some(map)) = (self.bg_mode.bpp(layer), self.tilemap_header(layer)) else {
            return;
        };

        let sy = (y + map.scroll_y) % (map.height * map.tile_px);
        let (ty, in_y) = (sy / map.tile_px, sy % map.tile_px);

        let mut cached = None;
        let mut pixels = [[0u8; 8]; 8];

        let mut x = 0;
        while x < line.len() as u32 {
            let sx = (x + map.scroll_x) % (map.width * map.tile_px);
            let (tx, in_x) = (sx / map.tile_px, sx % map.tile_px);
            let run = (map.tile_px - in_x).min(line.len() as u32 - x);

            let entry = TilemapEntry::from(self.vram.word(map.entry_addr(tx, ty) as u16));
            let py = if entry.flip_v {
                map.tile_px - 1 - in_y
            } else {
                in_y
            };
            let (base, _) = self.bg_palette(layer, bpp, entry.palette_selection as usize);

            for i in 0..run {
                let slot = &mut line[(x + i) as usize];
                if slot.is_some() {
                    continue;
                }

                let px = if entry.flip_h {
                    map.tile_px - 1 - (in_x + i)
                } else {
                    in_x + i
                };
                let index = entry.tile_index + (px / 8) as u16 + (py / 8) as u16 * 16;
                if cached != Some(index) {
                    pixels = self.bg_tile(layer, index).unwrap_or_default().decode(bpp);
                    cached = Some(index);
                }

                let color = pixels[py as usize % 8][px as usize % 8];
                if color != 0 {
                    *slot = Some(cgram::rgb(self.cgram.word((base + color as usize) as u8)));
                }
            }

            x += run;
        }
    }

    pub fn palette(&self, index: usize) -> [u16; 16] {
        std::array::from_fn(|i| self.cgram.word((index * 16 + i) as u8))
    }

    pub fn tileset(&self, layer: usize) -> Option<TilesetData> {
        let bpp = self.bg_mode.bpp(layer)?;
        let base_addr = self.bg_layers[layer].tileset_addr;
        let tiles = (0..256).filter_map(|i| self.bg_tile(layer, i)).collect();

        Some(TilesetData {
            bpp,
            base_addr,
            tiles,
        })
    }

    pub fn bg_tile(&self, layer: usize, index: u16) -> Option<Tile> {
        let bpp = self.bg_mode.bpp(layer)?;
        let bytes_per_tile = bpp as usize * 8;
        let base = self.bg_layers[layer].tileset_addr * 2 + index as u32 * bytes_per_tile as u32;

        let mut tile = Tile::default();
        for (j, byte) in tile.data[..bytes_per_tile].iter_mut().enumerate() {
            *byte = self.vram.byte(base + j as u32);
        }

        Some(tile)
    }

    pub fn tilemap(&self, layer: usize) -> Option<TilemapData> {
        let mut data = self.tilemap_header(layer)?;

        data.entries.reserve((data.width * data.height) as usize);
        for y in 0..data.height {
            for x in 0..data.width {
                let word = self.vram.word(data.entry_addr(x, y) as u16);
                data.entries.push(word.into());
            }
        }

        Some(data)
    }

    fn tilemap_header(&self, layer: usize) -> Option<TilemapData> {
        self.bg_mode.bpp(layer)?;
        
        let bg = &self.bg_layers[layer];

        Some(TilemapData {
            base_addr: bg.tilemap_addr,
            width: bg.horizontal_tilemap_count.count() as u32 * 32,
            height: bg.vertical_tilemap_count.count() as u32 * 32,
            tile_px: bg.tile_size.pixels(),
            scroll_x: bg.scroll_x(),
            scroll_y: bg.scroll_y(),
            entries: Vec::new(),
        })
    }

    pub fn bg_palette(&self, layer: usize, bpp: u8, palette: usize) -> (usize, usize) {
        match bpp {
            2 => {
                let base = match self.bg_mode {
                    BackgroundMode::Mode0 => layer * 32,
                    _ => 0,
                };
                (base + palette * 4, 4)
            }
            4 => (palette * 16, 16),
            _ => (0, 256),
        }
    }

    pub fn bg_palette_index(&self, layer: usize, bpp: u8, color: usize) -> usize {
        let (base, size) = self.bg_palette(layer, bpp, 0);
        color.saturating_sub(base) / size
    }

    pub fn frame_buffer(&self) -> &[(u8, u8, u8)] {
        &self.frame_buffer
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

    pub fn tile_bytes(&self) -> u32 {
        self.bpp as u32 * 8
    }
}

pub struct TilemapEntry {
    pub raw: u16,
    pub tile_index: u16,
    pub palette_selection: u8,
    pub priority: bool,
    pub flip_h: bool,
    pub flip_v: bool,
}

impl From<u16> for TilemapEntry {
    fn from(raw: u16) -> Self {
        Self {
            raw,
            tile_index: raw & 0b11_1111_1111,
            palette_selection: (raw >> 10) as u8 & 0b111,
            priority: raw & 1 << 13 != 0,
            flip_h: raw & 1 << 14 != 0,
            flip_v: raw & 1 << 15 != 0,
        }
    }
}

pub struct TilemapData {
    pub base_addr: u32,
    pub width: u32,
    pub height: u32,
    pub tile_px: u32,
    pub scroll_x: u32,
    pub scroll_y: u32,
    pub entries: Vec<TilemapEntry>,
}

impl TilemapData {
    pub fn entry_addr(&self, x: u32, y: u32) -> u32 {
        let screen = y / 32 * (self.width / 32) + x / 32;
        self.base_addr + screen * 0x400 + y % 32 * 32 + x % 32
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

pub(crate) fn draw_scanline<M: MemoryMap>(snes: &mut Snes<M>, deadline: u64) {
    let lines_per_frame = snes.scheduler.cycles_per_frame() / MASTER_CLOCKS_PER_SCANLINE;
    let scanline = (deadline / MASTER_CLOCKS_PER_SCANLINE) % lines_per_frame;
    if (1..=SCREEN_HEIGHT as u64).contains(&scanline) {
        snes.ppu.render_scanline(scanline as usize - 1);
    }
}
