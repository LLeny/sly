pub(crate) mod bg;
pub mod cgram;
pub(crate) mod color_math;
pub mod io;
pub(crate) mod latch;
mod layer_order;
pub(crate) mod mode7;
pub(crate) mod mosaic;
pub mod oam;
pub(crate) mod obj;
mod opt;
pub(crate) mod regs;
mod render;
pub mod tile;
pub mod vram;
pub(crate) mod window;

use crate::ppu::cgram::Cgram;
use crate::ppu::color_math::ColorMath;
use crate::ppu::latch::{ByteLatch, WordLatch};
use crate::ppu::mode7::Matrix;
use crate::ppu::mosaic::Mosaic;
use crate::ppu::oam::Oam;
use crate::ppu::obj::ObjLayer;
use crate::ppu::regs::{BackgroundMode, DisplayControl, LayerMask, LayerWindowEnable, Priority};
use crate::ppu::tile::Tile;
use crate::ppu::vram::Vram;
#[cfg(feature = "debug")]
use crate::ppu::window::WindowTrace;
use crate::ppu::window::Windows;
use crate::scheduler::{MASTER_CLOCKS_PER_SCANLINE, Scheduler, VBLANK_START_SCANLINE};
use crate::{MemoryMap, SnesBus};
use serde::{Deserialize, Serialize};

const HBLANK_START_CLOCK: u64 = 274 * 4;

// TODO
pub const SCREEN_WIDTH: u32 = 256;
pub const SCREEN_HEIGHT: u32 = 224;

#[derive(Serialize, Deserialize)]
pub struct Ppu {
    pub(crate) cgram: Cgram,
    pub(crate) vram: Vram,
    pub(crate) oam: Oam,
    pub(crate) bg_layers: [bg::BackgroundLayer; 4],
    pub(crate) bg_mode: BackgroundMode,
    pub(crate) bg3_priority: Priority,
    pub(crate) mosaic: Mosaic,
    pub(crate) bgofs_latch: ByteLatch,
    pub(crate) hofs_latch: ByteLatch,
    pub(crate) main_screen: LayerMask,
    pub(crate) sub_screen: LayerMask,
    pub(crate) main_screen_window: LayerWindowEnable,
    pub(crate) sub_screen_window: LayerWindowEnable,
    pub(crate) windows: Windows,
    #[cfg(feature = "debug")]
    #[serde(skip)]
    pub(crate) window_trace: WindowTrace,
    pub(crate) color_math: ColorMath,
    pub(crate) obj: ObjLayer,
    pub(crate) matrix: Matrix,
    pub(crate) force_blank: bool,
    pub(crate) brightness: u8,
    pub(crate) pseudo_hires: bool,
    pub(crate) extbg: bool,
    #[serde(skip)]
    pub(crate) frame_buffer: Vec<[u8; 4]>,
    pub(crate) counter_latch: bool,
    latched_h_counter: u16,
    latched_v_counter: u16,
    ophct_latch: WordLatch,
    opvct_latch: WordLatch,
}

impl Ppu {
    pub fn new() -> Self {
        Self {
            cgram: Cgram::new(),
            vram: Vram::new(),
            oam: Oam::new(),
            bg_layers: [bg::BackgroundLayer::default(); 4],
            bg_mode: BackgroundMode::Mode0,
            bg3_priority: Priority::Normal,
            mosaic: Mosaic::default(),
            bgofs_latch: ByteLatch::default(),
            hofs_latch: ByteLatch::default(),
            main_screen: LayerMask::default(),
            sub_screen: LayerMask::default(),
            main_screen_window: LayerWindowEnable::default(),
            sub_screen_window: LayerWindowEnable::default(),
            windows: Windows::default(),
            #[cfg(feature = "debug")]
            window_trace: WindowTrace::default(),
            color_math: ColorMath::default(),
            obj: ObjLayer::default(),
            matrix: Matrix::default(),
            force_blank: true,
            brightness: 0,
            pseudo_hires: false,
            extbg: false,
            frame_buffer: vec![[0, 0, 0, 0xff]; SCREEN_WIDTH as usize * SCREEN_HEIGHT as usize],
            counter_latch: false,
            latched_h_counter: 0,
            latched_v_counter: 0,
            ophct_latch: WordLatch::default(),
            opvct_latch: WordLatch::default(),
        }
    }

    fn latch_counters(&mut self, master_clock: u64, scanline: u16) {
        let mut hclock = master_clock % MASTER_CLOCKS_PER_SCANLINE;
        if hclock >= 1292 {
            hclock -= 2;
        }
        if hclock >= 1308 {
            hclock -= 2;
        }

        self.latched_h_counter = (hclock / 4) as u16 & 0x01FF;
        self.latched_v_counter = scanline & 0x01FF;
        self.counter_latch = true;
    }

    fn counter_byte(counter: u16, high_byte: bool) -> u8 {
        if high_byte {
            (counter >> 8) as u8 & 1
        } else {
            counter as u8
        }
    }

    pub(crate) fn peek_h_counter(&self) -> u8 {
        Self::counter_byte(self.latched_h_counter, self.ophct_latch.is_high())
    }

    pub(crate) fn read_h_counter(&mut self) -> u8 {
        Self::counter_byte(self.latched_h_counter, self.ophct_latch.toggle())
    }

    pub(crate) fn peek_v_counter(&self) -> u8 {
        Self::counter_byte(self.latched_v_counter, self.opvct_latch.is_high())
    }

    pub(crate) fn read_v_counter(&mut self) -> u8 {
        Self::counter_byte(self.latched_v_counter, self.opvct_latch.toggle())
    }

    pub(crate) fn reset_counter_reads(&mut self) {
        self.ophct_latch.reset();
        self.opvct_latch.reset();
    }

    pub(crate) fn set_display_control(&mut self, control: DisplayControl) {
        self.force_blank = control.force_blank();
        self.brightness = control.brightness();
    }

    pub fn palette(&self, index: usize) -> [u16; 16] {
        std::array::from_fn(|i| self.cgram.word((index * 16 + i) as u8))
    }

    pub fn obj_palette_addr(palette: u8, color: u8) -> u8 {
        128 + palette * 16 + color
    }

    pub fn obj_palette(&self, palette: u8) -> [u16; 16] {
        std::array::from_fn(|i| self.cgram.word(Self::obj_palette_addr(palette, i as u8)))
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

        Some(self.read_tile(base, bytes_per_tile))
    }

    pub fn tilemap(&self, layer: usize) -> Option<TilemapData> {
        let mut data = self.tilemap_header(layer)?;

        data.entries.reserve((data.width * data.height) as usize);
        for y in 0..data.height {
            for x in 0..data.width {
                let word = data.entry(&self.vram, x, y);
                data.entries.push(word.into());
            }
        }

        Some(data)
    }

    pub(crate) fn tilemap_header(&self, layer: usize) -> Option<TilemapData> {
        self.bg_mode.bpp(layer)?;
        Some(self.tilemap_geometry(layer))
    }

    pub(crate) fn tilemap_geometry(&self, layer: usize) -> TilemapData {
        let bg = &self.bg_layers[layer];

        TilemapData {
            base_addr: bg.tilemap_addr,
            width: bg.horizontal_tilemap_count.count() as u32 * 32,
            height: bg.vertical_tilemap_count.count() as u32 * 32,
            tile_px: bg.tile_size.pixels(),
            scroll_x: bg.scroll_x(),
            scroll_y: bg.scroll_y(),
            entries: Vec::new(),
        }
    }

    pub fn sprites(&self) -> Vec<Sprite> {
        (0..128).map(|i| self.sprite(i)).collect()
    }

    pub fn sprite(&self, index: u8) -> Sprite {
        let base = index as usize * 4;
        let attr = self.oam.attributes(index);
        let high = self.oam.high_bits(index);

        let x = self.oam.byte(base) as u16 | ((high.x_high() as u16) << 8);
        let tile = self.oam.byte(base + 2) as u16 | ((attr.tile_high() as u16) << 8);
        let (width, height) = self.obj.sprite_size.dimensions(high.large());

        Sprite {
            x: if x & 0x100 != 0 {
                x as i32 - 512
            } else {
                x as i32
            },
            y: self.oam.byte(base + 1),
            tile,
            tile_addr: self.obj.tile_addr(tile),
            palette: attr.palette(),
            priority: attr.priority(),
            flip_h: attr.flip_h(),
            flip_v: attr.flip_v(),
            width,
            height,
        }
    }

    pub fn obj_tile(&self, tile: u16) -> Tile {
        self.read_tile(self.obj.tile_addr(tile) * 2, 32)
    }

    fn read_tile(&self, base: u32, bytes: usize) -> Tile {
        let mut tile = Tile::default();
        for (j, byte) in tile.data[..bytes].iter_mut().enumerate() {
            *byte = self.vram.byte(base + j as u32);
        }
        tile
    }

    pub(crate) fn tile_row(&self, base: u32, y: usize, bpp: u8) -> [u8; 8] {
        let mut row = [0u8; 8];

        for pair in 0..(bpp as usize / 2) {
            let plane = base + (pair * 16 + y * 2) as u32;
            let low = self.vram.byte(plane);
            let high = self.vram.byte(plane + 1);

            for (x, pixel) in row.iter_mut().enumerate() {
                let bit0 = (low >> (7 - x)) & 1;
                let bit1 = (high >> (7 - x)) & 1;
                *pixel |= (bit1 << 1 | bit0) << (pair * 2);
            }
        }

        row
    }

    pub(crate) fn sprite_covers_line(&self, index: u8, y: u32) -> bool {
        let large = self.oam.high_bits(index).large();
        let (_, height) = self.obj.sprite_size.dimensions(large);
        let sprite_y = self.oam.byte(index as usize * 4 + 1);
        (y + 256 - sprite_y as u32) & 0xFF < height
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

    pub fn frame_buffer(&self) -> &[[u8; 4]] {
        &self.frame_buffer
    }

    pub(crate) fn frame_buffer_rgba(&self) -> &[u8] {
        self.frame_buffer.as_flattened()
    }

    pub(crate) fn restore_frame_buffer(&mut self, rgba: Option<&[u8]>) {
        self.frame_buffer.clear();
        self.frame_buffer.resize(
            SCREEN_WIDTH as usize * SCREEN_HEIGHT as usize,
            [0, 0, 0, 0xff],
        );

        let Some(rgba) = rgba else {
            return;
        };

        for (pixel, chunk) in self.frame_buffer.iter_mut().zip(rgba.as_chunks::<4>().0) {
            *pixel = *chunk;
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

    pub fn tile_bytes(&self) -> u32 {
        self.bpp as u32 * 8
    }
}

pub struct TilemapEntry {
    pub raw: u16,
    pub tile_index: u16,
    pub palette_selection: u8,
    pub priority: Priority,
    pub flip_h: bool,
    pub flip_v: bool,
}

impl From<u16> for TilemapEntry {
    fn from(raw: u16) -> Self {
        Self {
            raw,
            tile_index: raw & 0b11_1111_1111,
            palette_selection: (raw >> 10) as u8 & 0b111,
            priority: if raw & 1 << 13 != 0 {
                Priority::High
            } else {
                Priority::Normal
            },
            flip_h: raw & 1 << 14 != 0,
            flip_v: raw & 1 << 15 != 0,
        }
    }
}

impl TilemapEntry {
    pub fn texel(&self, x: u32, y: u32, tile_w: u32, tile_h: u32) -> (u32, u32) {
        (
            if self.flip_h { tile_w - 1 - x } else { x },
            if self.flip_v { tile_h - 1 - y } else { y },
        )
    }

    pub fn char_index(&self, px: u32, py: u32) -> u16 {
        self.tile_index + (px / 8) as u16 + (py / 8) as u16 * 16
    }
}

#[derive(Clone, Copy)]
pub struct Sprite {
    pub x: i32,
    pub y: u8,
    pub tile: u16,
    pub tile_addr: u32,
    pub palette: u8,
    pub priority: u8,
    pub flip_h: bool,
    pub flip_v: bool,
    pub width: u32,
    pub height: u32,
}

impl Sprite {
    pub fn sub_tile(&self, tx: u32, ty: u32) -> u16 {
        let col = (self.tile as u32 + tx) & 0x0F;
        let row = ((self.tile as u32 >> 4) + ty) & 0x0F;
        (self.tile & 0x100) | (row << 4) as u16 | col as u16
    }

    pub fn row(&self, y: u32) -> u32 {
        (y + 256 - self.y as u32) & 0xFF
    }

    pub fn in_range(&self, y: u32) -> bool {
        self.row(y) < self.height && self.x > -(self.width as i32)
    }

    pub fn texel(&self, x: u32, y: u32) -> (u32, u32) {
        (
            if self.flip_h { self.width - 1 - x } else { x },
            if self.flip_v { self.height - 1 - y } else { y },
        )
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

    pub(crate) fn entry(&self, vram: &Vram, x: u32, y: u32) -> u16 {
        vram.word(self.entry_addr(x, y) as u16)
    }
}

pub(crate) fn vblank_start<M: MemoryMap>(snes: &mut SnesBus<M>, _deadline: u64) {
    #[cfg(feature = "debug")]
    snes.ppu.window_trace.finish_frame();

    snes.irq.set_vblank(true);

    if !snes.ppu.force_blank {
        snes.ppu.oam.reload();
    }

    if snes.irq.joypad_autoread_enabled() {
        snes.joypads.autoread();
    }
}

pub(crate) fn vblank_end<M: MemoryMap>(snes: &mut SnesBus<M>, _deadline: u64) {
    snes.irq.set_vblank(false);
}

pub(crate) fn in_vblank(scheduler: &Scheduler) -> bool {
    scheduler.cycles() % scheduler.cycles_per_frame()
        >= VBLANK_START_SCANLINE * MASTER_CLOCKS_PER_SCANLINE
}

pub(crate) fn in_hblank(scheduler: &Scheduler) -> bool {
    scheduler.cycles() % MASTER_CLOCKS_PER_SCANLINE >= HBLANK_START_CLOCK
}

pub(crate) fn latch_hv_counters<M: MemoryMap>(snes: &mut SnesBus<M>) {
    let master_clock = snes.scheduler.cycles();
    let scanline = snes.scheduler.scanline_at(master_clock) as u16;
    snes.ppu.latch_counters(master_clock, scanline);
}

pub(crate) fn draw_scanline<M: MemoryMap>(snes: &mut SnesBus<M>, deadline: u64) {
    let scanline = snes.scheduler.scanline_at(deadline);
    if (1..=SCREEN_HEIGHT as u64).contains(&scanline) {
        snes.ppu.render_scanline(scanline as usize - 1);
    }
}
