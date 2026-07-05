pub(crate) mod bg;
pub mod cgram;
pub(crate) mod color_math;
pub(crate) mod latch;
pub mod oam;
pub(crate) mod obj;
pub(crate) mod regs;
pub mod tile;
pub mod vram;

use crate::ppu::cgram::Cgram;
use crate::ppu::color_math::ColorMath;
use crate::ppu::latch::ByteLatch;
use crate::ppu::oam::Oam;
use crate::ppu::obj::ObjLayer;
use crate::ppu::regs::{BackgroundMode, LayerMask, Priority};
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
    pub(crate) oam: Oam,
    pub(crate) bg_layers: [bg::BackgroundLayer; 4],
    pub(crate) bg_mode: BackgroundMode,
    pub(crate) bg3_priority: Priority,
    pub(crate) bgofs_latch: ByteLatch,
    pub(crate) hofs_latch: ByteLatch,
    pub(crate) main_screen: LayerMask,
    pub(crate) sub_screen: LayerMask,
    pub(crate) color_math: ColorMath,
    pub(crate) obj: ObjLayer,
    pub(crate) frame_buffer: Vec<(u8, u8, u8)>,
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
            bgofs_latch: ByteLatch::default(),
            hofs_latch: ByteLatch::default(),
            main_screen: LayerMask::default(),
            sub_screen: LayerMask::default(),
            color_math: ColorMath::default(),
            obj: ObjLayer::default(),
            frame_buffer: vec![(0, 0, 0); SCREEN_WIDTH as usize * SCREEN_HEIGHT as usize],
        }
    }

    pub fn render_scanline(&mut self, scanline: usize) {
        let mut line = [None; SCREEN_WIDTH as usize];
        let obj = if self.main_screen.obj() {
            self.render_obj_scanline(scanline as u32)
        } else {
            [None; SCREEN_WIDTH as usize]
        };

        for &layer in self.layer_order() {
            match layer {
                Layer::Bg(layer, priority) => {
                    if self.main_screen.bg(layer) {
                        self.render_bg_scanline(layer, priority, scanline as u32, &mut line);
                    }
                }
                Layer::Obj(priority) => {
                    for (slot, &pixel) in line.iter_mut().zip(obj.iter()) {
                        if slot.is_some() {
                            continue;
                        }
                        if let Some((p, color)) = pixel
                            && p == priority
                        {
                            *slot = Some(cgram::rgb(self.cgram.word(color)));
                        }
                    }
                }
            }
        }

        let mut backdrop = self.cgram.word(0);
        if self.color_math.layers.backdrop() {
            backdrop = self.color_math.apply(backdrop);
        }

        let backdrop = cgram::rgb(backdrop);
        let row =
            &mut self.frame_buffer[scanline * SCREEN_WIDTH as usize..][..SCREEN_WIDTH as usize];
        for (dst, pixel) in row.iter_mut().zip(line) {
            *dst = pixel.unwrap_or(backdrop);
        }
    }

    fn layer_order(&self) -> &'static [Layer] {
        use Layer::{Bg, Obj};
        
        const H: bool = true;
        const L: bool = false;

        match (self.bg_mode, self.bg3_priority) {
            (BackgroundMode::Mode0, _) => &[
                Obj(3), Bg(0, H), Bg(1, H),
                Obj(2), Bg(0, L), Bg(1, L),
                Obj(1), Bg(2, H), Bg(3, H),
                Obj(0), Bg(2, L), Bg(3, L),
            ],
            (BackgroundMode::Mode1, Priority::High) => &[
                Bg(2, H),
                Obj(3), Bg(0, H), Bg(1, H),
                Obj(2), Bg(0, L), Bg(1, L),
                Obj(1),
                Obj(0), Bg(2, L),
            ],
            (BackgroundMode::Mode1, Priority::Normal) => &[
                Obj(3), Bg(0, H), Bg(1, H),
                Obj(2), Bg(0, L), Bg(1, L),
                Obj(1), Bg(2, H),
                Obj(0), Bg(2, L),
            ],
            // TODO: modes 2-7
            _ => &[
                Obj(3), Bg(0, H),
                Obj(2), Bg(1, H),
                Obj(1), Bg(0, L),
                Obj(0), Bg(1, L),
            ],
        }
    }

    fn render_obj_scanline(&self, y: u32) -> [Option<(u8, u8)>; SCREEN_WIDTH as usize] {
        let mut line = [None; SCREEN_WIDTH as usize];

        // TODO: priority rotation
        let mut in_range = [None; 32];
        let mut count = 0;
        for index in 0..128 {
            let sprite = self.sprite(index);
            if (y + 256 - sprite.y as u32) & 0xFF >= sprite.height
                || sprite.x <= -(sprite.width as i32)
            {
                continue;
            }
            if count == in_range.len() {
                // TODO: range over flag
                break;
            }
            in_range[count] = Some(sprite);
            count += 1;
        }

        let mut tiles = 0;
        for sprite in in_range[..count].iter().rev().flatten() {
            let mut row = (y + 256 - sprite.y as u32) & 0xFF;
            if sprite.flip_v {
                row = sprite.height - 1 - row;
            }

            for sliver in 0..sprite.width / 8 {
                let base = sprite.x + sliver as i32 * 8;
                if base <= -8 || base >= SCREEN_WIDTH as i32 {
                    continue;
                }
                if tiles == 34 {
                    // TODO: time over flag
                    return line;
                }
                tiles += 1;

                let tx = if sprite.flip_h {
                    sprite.width / 8 - 1 - sliver
                } else {
                    sliver
                };
                let pixels = self
                    .obj_tile(sprite.sub_tile(tx, row / 8))
                    .decode_row(row as usize % 8, 4);

                for i in 0..8u32 {
                    let x = base + i as i32;
                    if !(0..SCREEN_WIDTH as i32).contains(&x) {
                        continue;
                    }

                    let px = if sprite.flip_h { 7 - i } else { i };
                    let color = pixels[px as usize];
                    if color != 0 {
                        line[x as usize] =
                            Some((sprite.priority, Self::obj_palette_addr(sprite.palette, color)));
                    }
                }
            }
        }

        line
    }

    fn render_bg_scanline(
        &self,
        layer: usize,
        priority: bool,
        y: u32,
        line: &mut [Option<(u8, u8, u8)>],
    ) {
        let (Some(bpp), Some(map)) = (self.bg_mode.bpp(layer), self.tilemap_header(layer)) else {
            return;
        };

        let sy = (y + map.scroll_y) % (map.height * map.tile_px);
        let (ty, in_y) = (sy / map.tile_px, sy % map.tile_px);

        let mut cached = None;
        let mut pixels = [0u8; 8];

        let mut x = 0;
        while x < line.len() as u32 {
            let sx = (x + map.scroll_x) % (map.width * map.tile_px);
            let (tx, in_x) = (sx / map.tile_px, sx % map.tile_px);
            let run = (map.tile_px - in_x).min(line.len() as u32 - x);

            let entry = TilemapEntry::from(self.vram.word(map.entry_addr(tx, ty) as u16));
            if entry.priority != priority {
                x += run;
                continue;
            }

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
                if cached != Some((index, py)) {
                    pixels = self
                        .bg_tile(layer, index)
                        .unwrap_or_default()
                        .decode_row(py as usize % 8, bpp);
                    cached = Some((index, py));
                }

                let color = pixels[px as usize % 8];
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

    pub fn sprites(&self) -> Vec<Sprite> {
        (0..128).map(|i| self.sprite(i)).collect()
    }

    pub fn sprite(&self, index: u8) -> Sprite {
        let base = index as usize * 4;
        let attr = self.oam.byte(base + 3);
        let extra = self.oam.byte(512 + index as usize / 4) >> ((index as usize % 4) * 2);

        let large = extra & 2 != 0;
        let x = self.oam.byte(base) as u16 | ((extra as u16 & 1) << 8);
        let tile = self.oam.byte(base + 2) as u16 | ((attr as u16 & 1) << 8);
        let (width, height) = self.obj.sprite_size.dimensions(large);

        Sprite {
            x: if x & 0x100 != 0 {
                x as i32 - 512
            } else {
                x as i32
            },
            y: self.oam.byte(base + 1),
            tile,
            tile_addr: self.obj.tile_addr(tile),
            palette: (attr >> 1) & 7,
            priority: (attr >> 4) & 3,
            flip_h: attr & 0x40 != 0,
            flip_v: attr & 0x80 != 0,
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

#[derive(Clone, Copy)]
enum Layer {
    Bg(usize, bool),
    Obj(u8),
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
