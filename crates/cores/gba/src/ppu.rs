pub(crate) mod io;
pub(crate) mod regs;
pub(crate) mod sprite;
pub(crate) mod tile;

use std::cmp::Reverse;

use crate::bus::Width;
use crate::ppu::regs::{BackgroundKind, BackgroundMode};
use crate::ppu::sprite::{Attribute, OBJ_TILE_OFFSET};
use crate::ppu::tile::Tile;
use crate::{SCREEN_HEIGHT, SCREEN_WIDTH};
use regs::{
    BackgroundControl, BackgroundOffset, DisplayControl, DisplayStatus, SpriteMapping,
    VerticalCount,
};

const SCANLINES_PER_FRAME: u8 = 228;
const VBLANK_START_SCANLINE: u8 = 160;
const VBLANK_END_SCANLINE: u8 = 227;

pub(crate) const PALETTE_BASE: u32 = 0x0500_0000;
pub(crate) const PALETTE_END: u32 = 0x05FF_FFFF;
pub(crate) const PALETTE_SIZE: u32 = 0x400;
pub(crate) const VRAM_BASE: u32 = 0x0600_0000;
pub(crate) const VRAM_END: u32 = 0x06FF_FFFF;
pub(crate) const VRAM_SIZE: u32 = 0x18000;
pub(crate) const OAM_BASE: u32 = 0x0700_0000;
pub(crate) const OAM_END: u32 = 0x07FF_FFFF;
pub(crate) const OAM_SIZE: u32 = 0x400;

pub(crate) const SCANLINE_CYCLES: u64 = 1232;
pub(crate) const HBLANK_START_CYCLES: u64 = 1006;
pub(crate) const VBLANK_START_CYCLES: u64 = VBLANK_START_SCANLINE as u64 * SCANLINE_CYCLES;
pub(crate) const VBLANK_END_CYCLES: u64 = VBLANK_END_SCANLINE as u64 * SCANLINE_CYCLES;
pub(crate) const FRAME_CYCLES: u64 = SCANLINES_PER_FRAME as u64 * SCANLINE_CYCLES;

pub(crate) fn bitmap_layout(control: DisplayControl) -> Option<BitmapLayout> {
    let page = control.frame_select().vram_offset();
    let (width, height, bytes_per_pixel, base) = match control.mode() {
        BackgroundMode::Mode3 => (SCREEN_WIDTH as usize, SCREEN_HEIGHT as usize, 2, 0),
        BackgroundMode::Mode4 => (SCREEN_WIDTH as usize, SCREEN_HEIGHT as usize, 1, page),
        BackgroundMode::Mode5 => (160, 128, 2, page),
        _ => return None,
    };

    Some(BitmapLayout {
        width,
        height,
        bytes_per_pixel,
        base,
    })
}

pub(crate) fn render_bitmap_scanline(
    layout: BitmapLayout,
    memory: VideoMemoryView<'_>,
    line: usize,
    output: &mut [[u8; 4]],
) {
    if line >= layout.height {
        return;
    }
    assert!(output.len() >= layout.width);

    let pitch = layout.width * layout.bytes_per_pixel;
    let start = layout.base + (line * pitch);
    let src = &memory.vram[start..start + pitch];
    for (dst, bytes) in output
        .iter_mut()
        .zip(src.chunks_exact(layout.bytes_per_pixel))
    {
        let color = match bytes {
            &[index] => palette_color(memory.palette, index.into()),
            _ => u16::from_le_bytes([bytes[0], bytes[1]]),
        };
        *dst = rgba(color);
    }
}

pub(crate) fn render_bgmap_scanline(
    control: DisplayControl,
    backgrounds: BackgroundView<'_>,
    memory: VideoMemoryView<'_>,
    line: usize,
    output: &mut [[u8; 4]],
) {
    let mut order = [0, 1, 2, 3];
    order.sort_by_key(|&index| Reverse((backgrounds.bgcnt[index].priority(), index)));

    for index in order {
        let bg = backgrounds.bgcnt[index];
        let kind = control.mode().background_kind(index);
        if !control.bg_enabled(index) || kind != BackgroundKind::Text {
            continue;
        }
        let Some((width, height)) = bg.dimensions(kind) else {
            continue;
        };

        let depth = bg.effective_color_depth(kind);
        let hofs = backgrounds.bghofs[index].offset() as usize;
        let vofs = backgrounds.bgvofs[index].offset() as usize;
        let bg_y = (line + vofs) & (height - 1);
        let tile_y = bg_y / 8;
        let pixel_y = bg_y % 8;

        let mut x = 0;
        while x < output.len() {
            let bg_x = (x + hofs) & (width - 1);
            let Some((_, entry)) = bg.map_entry(kind, memory.vram, bg_x / 8, tile_y, width) else {
                break;
            };

            let addr = bg.tile_offset(entry.tile_number() as usize, depth);
            let tile = Tile {
                data: &memory.vram[addr..(addr + depth.tile_size())],
                depth,
            };
            let palette_base = entry.palette_base(depth);

            let first = bg_x % 8;
            let span = (8 - first).min(output.len() - x);
            for (pixel_x, dst) in (first..).zip(&mut output[x..(x + span)]) {
                let color_index = entry.pixel(&tile, pixel_x, pixel_y) as usize;
                if color_index != 0 {
                    *dst = rgba(palette_color(memory.palette, palette_base + color_index));
                }
            }

            x += span;
        }
    }
}

pub(crate) fn render_sprite_scanline(
    control: DisplayControl,
    memory: VideoMemoryView<'_>,
    line: usize,
    output: &mut [[u8; 4]],
) {
    // Lower OAM entries win, so draw them last.
    for attr in sprite::attributes(memory.oam).rev() {
        if attr.affine() {
            // TODO: skipping affine shit for now
            continue;
        }

        if attr.disable_obj() {
            continue;
        }

        let (_, height) = attr.dimensions();
        let row = (line as u8).wrapping_sub(attr.y_coordinate()) as usize;
        if row >= height {
            continue;
        }

        let x_start = attr.x_coordinate() as usize;
        for (col, color) in sprite_row(attr, control.sprite_mapping(), memory, row) {
            let x = (x_start + col) & 0x1FF;
            if x < SCREEN_WIDTH as usize {
                output[x] = color;
            }
        }
    }
}

pub(crate) fn sprite_row<'a>(
    attr: Attribute,
    mapping: SpriteMapping,
    memory: VideoMemoryView<'a>,
    row: usize,
) -> impl Iterator<Item = (usize, [u8; 4])> + 'a {
    let (width, height) = attr.dimensions();
    let flips = !attr.affine();
    let hflip = flips && attr.horizontal_flip();
    let row = if flips && attr.vertical_flip() {
        height - 1 - row
    } else {
        row
    };

    let depth = attr.color_depth();
    let tile_bytes = depth.tile_size();
    let row_addr = attr.tile_addr(mapping, 0, row / 8);
    let palette_base = attr.palette_base();

    (0..width).filter_map(move |x| {
        let col = if hflip { width - 1 - x } else { x };
        let tile_addr = row_addr + ((col / 8) * tile_bytes);
        let data = memory.vram.get(tile_addr..tile_addr + tile_bytes)?;
        let tile = Tile { data, depth };
        let index = tile.pixel(col % 8, row % 8) as usize;
        if index == 0 {
            return None; // transparent
        }

        let color = palette_color(memory.palette, palette_base + index);
        Some((x, rgba(color)))
    })
}

pub struct Ppu {
    #[cfg(feature = "debug")]
    pub(crate) revisions: Revisions,

    pub(crate) dispstat: DisplayStatus,
    pub(crate) dispctrl: DisplayControl,
    pub(crate) vcount: VerticalCount,
    pub(crate) bgcnt: [BackgroundControl; 4],
    pub(crate) bghofs: [BackgroundOffset; 4],
    pub(crate) bgvofs: [BackgroundOffset; 4],

    pub(crate) vram: Vec<u8>,
    pub(crate) palette: Vec<u8>,
    pub(crate) oam: Vec<u8>,

    pub(crate) frame: Vec<[u8; 4]>,
}

impl Ppu {
    pub fn new() -> Self {
        Self {
            #[cfg(feature = "debug")]
            revisions: Revisions {
                palette: 0,
                vram: 0,
                oam: 0,
            },
            dispstat: DisplayStatus::zeroed().with_vcounter(true),
            dispctrl: DisplayControl::zeroed(),
            vcount: VerticalCount::zeroed(),
            bgcnt: [BackgroundControl::zeroed(); 4],
            bghofs: [BackgroundOffset::zeroed(); 4],
            bgvofs: [BackgroundOffset::zeroed(); 4],
            vram: vec![0; VRAM_SIZE as usize],
            palette: vec![0; PALETTE_SIZE as usize],
            oam: vec![0; OAM_SIZE as usize],
            frame: vec![[255; 4]; (SCREEN_WIDTH * SCREEN_HEIGHT) as usize],
        }
    }

    pub(crate) fn reset(&mut self) {
        *self = Self {
            #[cfg(feature = "debug")]
            revisions: {
                let mut revisions = self.revisions;
                revisions.bump();
                revisions
            },
            ..Self::new()
        };
    }

    pub(crate) fn render_scanline(&mut self) {
        let line = self.vcount.scanline() as usize;
        let start = line * SCREEN_WIDTH as usize;
        let output = &mut self.frame[start..start + SCREEN_WIDTH as usize];
        let memory = VideoMemoryView {
            vram: &self.vram,
            palette: &self.palette,
            oam: &self.oam,
        };

        output.fill(rgba(palette_color(memory.palette, 0)));

        match bitmap_layout(self.dispctrl) {
            Some(layout) if self.dispctrl.bg2_enabled() => {
                render_bitmap_scanline(layout, memory, line, output);
            }
            None => {
                let backgrounds = BackgroundView {
                    bgcnt: &self.bgcnt,
                    bghofs: &self.bghofs,
                    bgvofs: &self.bgvofs,
                };
                render_bgmap_scanline(self.dispctrl, backgrounds, memory, line, output);
            }
            _ => {}
        }

        if self.dispctrl.enable_obj() {
            render_sprite_scanline(self.dispctrl, memory, line, output);
        }
    }

    #[cfg(feature = "debug")]
    pub(crate) fn memory(&self) -> VideoMemoryView<'_> {
        VideoMemoryView {
            vram: &self.vram,
            palette: &self.palette,
            oam: &self.oam,
        }
    }

    #[cfg(feature = "debug")]
    pub(crate) fn palette_color(&self, index: usize) -> u16 {
        palette_color(&self.palette, index)
    }

    pub(crate) fn start_hblank(&mut self) {
        self.dispstat.set_hblank(true);

        if self.vcount.scanline() < SCREEN_HEIGHT as u8 {
            self.render_scanline();
        }
    }

    pub(crate) fn start_vblank(&mut self) {
        self.dispstat.set_vblank(true);
    }

    pub(crate) fn end_vblank(&mut self) {
        self.dispstat.set_vblank(false);
    }

    pub(crate) fn start_scanline(&mut self) {
        let line = (self.vcount.scanline() + 1) % SCANLINES_PER_FRAME;
        self.vcount.set_scanline(line);

        self.dispstat.set_hblank(false);
        self.dispstat
            .set_vcounter(self.dispstat.vcount_compare() == line);
    }

    fn read_palette(&self, address: u32, width: Width) -> u32 {
        self::read_memory(&self.palette, address, width, |address| {
            address as usize & (PALETTE_SIZE as usize - 1)
        })
    }

    fn write_palette(&mut self, mut address: u32, width: Width, value: u32) {
        if width == Width::Byte {
            address &= !1;
            self::write_memory(&mut self.palette, address + 1, width, value, |address| {
                address as usize & (PALETTE_SIZE as usize - 1)
            });
        }

        #[cfg(feature = "debug")]
        {
            self.revisions.palette += 1;
        }

        self::write_memory(&mut self.palette, address, width, value, |address| {
            address as usize & (PALETTE_SIZE as usize - 1)
        });
    }

    fn read_vram(&self, address: u32, width: Width) -> u32 {
        self::read_memory(&self.vram, address, width, self::vram_offset)
    }

    fn write_vram(&mut self, mut address: u32, width: Width, value: u32) {
        if width == Width::Byte {
            if self::vram_offset(address) >= self.obj_offset() {
                return;
            }

            address &= !1;
            self::write_memory(&mut self.vram, address + 1, width, value, self::vram_offset);
        }

        #[cfg(feature = "debug")]
        {
            self.revisions.vram += 1;
        }

        self::write_memory(&mut self.vram, address, width, value, self::vram_offset);
    }

    fn read_oam(&self, address: u32, width: Width) -> u32 {
        self::read_memory(&self.oam, address, width, |address| {
            address as usize & (OAM_SIZE as usize - 1)
        })
    }

    fn write_oam(&mut self, address: u32, width: Width, value: u32) {
        if width == Width::Byte {
            tracing::error!("Attempted to write to OAM with byte width");
            return;
        }

        #[cfg(feature = "debug")]
        {
            self.revisions.oam += 1;
        }

        self::write_memory(&mut self.oam, address, width, value, |address| {
            address as usize & (OAM_SIZE as usize - 1)
        });
    }

    fn obj_offset(&self) -> usize {
        match self.dispctrl.mode() {
            BackgroundMode::Mode0 | BackgroundMode::Mode1 | BackgroundMode::Mode2 => {
                OBJ_TILE_OFFSET
            }
            _ => 0x14000,
        }
    }

    pub fn frame(&self) -> &[[u8; 4]] {
        &self.frame
    }
}

fn read_memory(memory: &[u8], address: u32, width: Width, offset: impl Fn(u32) -> usize) -> u32 {
    let mut value = 0;
    for lane in 0..width.bytes() {
        value |= u32::from(memory[offset(address.wrapping_add(lane))]) << (lane * 8);
    }
    value
}

fn write_memory(
    memory: &mut [u8],
    address: u32,
    width: Width,
    value: u32,
    offset: impl Fn(u32) -> usize,
) {
    for lane in 0..width.bytes() {
        memory[offset(address.wrapping_add(lane))] = (value >> (lane * 8)) as u8;
    }
}

fn palette_color(palette: &[u8], index: usize) -> u16 {
    let offset = (index * 2) & (PALETTE_SIZE as usize - 1);
    u16::from_le_bytes([palette[offset], palette[offset + 1]])
}

pub(crate) fn rgba(color: u16) -> [u8; 4] {
    let expand = |shift: u32| {
        let channel = ((color >> shift) & 0x1F) as u8;
        (channel << 3) | (channel >> 2)
    };

    [expand(0), expand(5), expand(10), 255]
}

pub(crate) fn vram_offset(address: u32) -> usize {
    // Each 128 KiB window repeats the last 32 KiB of the 96 KiB VRAM.
    let offset = address as usize & 0x1FFFF;
    if offset >= VRAM_SIZE as usize {
        offset - 0x8000
    } else {
        offset
    }
}

#[derive(Clone, Copy)]
pub(crate) struct BitmapLayout {
    pub(crate) width: usize,
    pub(crate) height: usize,
    pub(crate) bytes_per_pixel: usize,
    pub(crate) base: usize,
}

impl BitmapLayout {
    pub(crate) fn address(self) -> u32 {
        VRAM_BASE + self.base as u32
    }
}

#[derive(Clone, Copy)]
pub(crate) struct BackgroundView<'a> {
    pub(crate) bgcnt: &'a [BackgroundControl; 4],
    pub(crate) bghofs: &'a [BackgroundOffset; 4],
    pub(crate) bgvofs: &'a [BackgroundOffset; 4],
}

#[derive(Clone, Copy)]
pub(crate) struct VideoMemoryView<'a> {
    pub(crate) vram: &'a [u8],
    pub(crate) palette: &'a [u8],
    pub(crate) oam: &'a [u8],
}

#[cfg(feature = "debug")]
#[derive(Clone, Copy)]
pub(crate) struct Revisions {
    pub(crate) palette: u64,
    pub(crate) vram: u64,
    pub(crate) oam: u64,
}

#[cfg(feature = "debug")]
impl Revisions {
    pub(crate) fn bump(&mut self) {
        self.palette += 1;
        self.vram += 1;
        self.oam += 1;
    }
}
