pub(crate) mod io;
pub(crate) mod regs;
pub(crate) mod sprite;
pub(crate) mod tile;

use crate::bus::Width;
use crate::ppu::regs::BackgroundMode;
use crate::ppu::sprite::{Attribute, OBJ_TILE_OFFSET};
use crate::ppu::tile::Tile;
use crate::{SCREEN_HEIGHT, SCREEN_WIDTH};
use regs::{BackgroundControl, DisplayControl, DisplayStatus, SpriteMapping, VerticalCount};

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

pub(crate) fn render_bg_scanline(
    layout: BitmapLayout,
    vram: &[u8],
    palette: &[u8],
    line: usize,
    output: &mut [[u8; 4]],
) {
    output.fill(rgba(palette_color(palette, 0)));
    if line >= layout.height {
        return;
    }
    assert!(output.len() >= layout.width);

    let pitch = layout.width * layout.bytes_per_pixel;
    let start = layout.base + (line * pitch);
    let src = &vram[start..start + pitch];
    for (dst, bytes) in output
        .iter_mut()
        .zip(src.chunks_exact(layout.bytes_per_pixel))
    {
        let color = match bytes {
            &[index] => palette_color(palette, index.into()),
            _ => u16::from_le_bytes([bytes[0], bytes[1]]),
        };
        *dst = rgba(color);
    }
}

pub(crate) fn render_sprite_scanline(
    control: DisplayControl,
    vram: &[u8],
    oam: &[u8],
    palette: &[u8],
    line: usize,
    output: &mut [[u8; 4]],
) {
    // Lower OAM entries win, so draw them last.
    for attr in sprite::attributes(oam).rev() {
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
        for (col, color) in sprite_row(attr, control.sprite_mapping(), vram, palette, row) {
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
    vram: &'a [u8],
    palette: &'a [u8],
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
    let tile_bytes = depth.tile_bytes();
    let row_addr = attr.tile_addr(mapping, 0, row / 8);
    let palette_base = attr.palette_base();

    (0..width).filter_map(move |x| {
        let col = if hflip { width - 1 - x } else { x };
        let tile_addr = row_addr + ((col / 8) * tile_bytes);
        let data = vram.get(tile_addr..tile_addr + tile_bytes)?;
        let tile = Tile { data, depth };
        let index = tile.pixel(col % 8, row % 8) as usize;
        if index == 0 {
            return None; // transparent
        }

        let color = palette_color(palette, palette_base + index);
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

        let bitmap = bitmap_layout(self.dispctrl).filter(|_| self.dispctrl.enable_bg2());
        if let Some(layout) = bitmap {
            render_bg_scanline(layout, &self.vram, &self.palette, line, output);
        } else {
            output.fill(rgba(palette_color(&self.palette, 0)));
        }

        if self.dispctrl.enable_obj() {
            render_sprite_scanline(
                self.dispctrl,
                &self.vram,
                &self.oam,
                &self.palette,
                line,
                output,
            );
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
