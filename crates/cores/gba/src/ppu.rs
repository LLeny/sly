pub(crate) mod io;
pub(crate) mod regs;

use crate::bus::Width;
use crate::ppu::regs::BackgroundMode;
use crate::{SCREEN_HEIGHT, SCREEN_WIDTH};
use regs::{DisplayControl, DisplayStatus, VerticalCount};

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

pub struct Ppu {
    #[cfg(feature = "debug")]
    pub(crate) revisions: Revisions,

    pub(crate) dispstat: DisplayStatus,
    pub(crate) dispctrl: DisplayControl,
    pub(crate) vcount: VerticalCount,

    pub(crate) vram: Vec<u8>,
    pub(crate) palette: Vec<u8>,
    pub(crate) oam: Vec<u8>,

    pub(crate) frame: Vec<[u8; 4]>,
}

/// Bitmap width, height, bytes per pixel and VRAM page offset.
pub(crate) fn bitmap_layout(control: DisplayControl) -> Option<(usize, usize, usize, usize)> {
    let page = control.frame_select().vram_offset();
    match control.mode() {
        BackgroundMode::Mode3 => Some((SCREEN_WIDTH as usize, SCREEN_HEIGHT as usize, 2, 0)),
        BackgroundMode::Mode4 => Some((SCREEN_WIDTH as usize, SCREEN_HEIGHT as usize, 1, page)),
        BackgroundMode::Mode5 => Some((160, 128, 2, page)),
        _ => None,
    }
}

pub(crate) fn render_bg_scanline(
    control: DisplayControl,
    vram: &[u8],
    palette: &[u8],
    line: usize,
    output: &mut [[u8; 4]],
) {
    let (width, height, bytes_per_pixel, base) =
        bitmap_layout(control).expect("render_bg_scanline requires a bitmap background mode");
    output.fill(rgba(palette_color(palette, 0)));
    if line >= height {
        return;
    }
    assert!(output.len() >= width);

    let pitch = width * bytes_per_pixel;
    let start = base + line * pitch;
    let src = &vram[start..start + pitch];
    for (dst, bytes) in output.iter_mut().zip(src.chunks_exact(bytes_per_pixel)) {
        let color = match control.mode() {
            BackgroundMode::Mode4 => palette_color(palette, bytes[0].into()),
            _ => u16::from_le_bytes([bytes[0], bytes[1]]),
        };
        *dst = rgba(color);
    }
}

impl Default for Ppu {
    fn default() -> Self {
        Self::new()
    }
}

impl Ppu {
    pub fn new() -> Self {
        Self {
            #[cfg(feature = "debug")]
            revisions: Revisions {
                palette: 0,
                vram: 0,
            },
            dispstat: DisplayStatus::zeroed().with_vcounter(true),
            dispctrl: DisplayControl::zeroed(),
            vcount: VerticalCount::zeroed(),
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
        let bg2 = self.dispctrl.enable_bg2();
        if bg2
            && matches!(
                self.dispctrl.mode(),
                BackgroundMode::Mode3 | BackgroundMode::Mode4 | BackgroundMode::Mode5
            )
        {
            self.render_bitmap_mode();
        }
    }

    pub(crate) fn render_bitmap_mode(&mut self) {
        let line = self.vcount.scanline() as usize;
        let start = line * SCREEN_WIDTH as usize;
        let end = start + SCREEN_WIDTH as usize;
        render_bg_scanline(
            self.dispctrl,
            &self.vram,
            &self.palette,
            line,
            &mut self.frame[start..end],
        );
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

        self::write_memory(&mut self.oam, address, width, value, |address| {
            address as usize & (OAM_SIZE as usize - 1)
        });
    }

    fn obj_offset(&self) -> usize {
        match self.dispctrl.mode() {
            BackgroundMode::Mode0 | BackgroundMode::Mode1 | BackgroundMode::Mode2 => 0x10000,
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

#[cfg(feature = "debug")]
#[derive(Clone, Copy)]
pub(crate) struct Revisions {
    pub(crate) palette: u64,
    pub(crate) vram: u64,
}

#[cfg(feature = "debug")]
impl Revisions {
    pub(crate) fn bump(&mut self) {
        self.palette += 1;
        self.vram += 1;
    }
}
