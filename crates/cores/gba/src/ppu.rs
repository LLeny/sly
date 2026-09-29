pub(crate) mod io;
pub(crate) mod regs;

use crate::bus::Width;
use crate::{SCREEN_HEIGHT, SCREEN_WIDTH};
use regs::{DisplayStatus, VerticalCount};

const SCANLINES_PER_FRAME: u8 = 228;
const VBLANK_START_SCANLINE: u8 = 160;
const VBLANK_END_SCANLINE: u8 = 227;

pub(crate) const SCANLINE_CYCLES: u64 = 1232;
pub(crate) const HBLANK_START_CYCLES: u64 = 1006;
pub(crate) const VBLANK_START_CYCLES: u64 = VBLANK_START_SCANLINE as u64 * SCANLINE_CYCLES;
pub(crate) const VBLANK_END_CYCLES: u64 = VBLANK_END_SCANLINE as u64 * SCANLINE_CYCLES;
pub(crate) const FRAME_CYCLES: u64 = SCANLINES_PER_FRAME as u64 * SCANLINE_CYCLES;

pub struct Ppu {
    pub(crate) dispstat: DisplayStatus,
    pub(crate) vcount: VerticalCount,

    vram: [u8; 0x18000],
    palette: [u8; 0x400],
    oam: [u8; 0x400],

    frame: [[u8; 4]; (SCREEN_WIDTH * SCREEN_HEIGHT) as usize],
}

impl Default for Ppu {
    fn default() -> Self {
        Self::new()
    }
}

impl Ppu {
    pub const fn new() -> Self {
        Self {
            dispstat: DisplayStatus::zeroed().with_vcounter(true),
            vcount: VerticalCount::zeroed(),
            vram: [0; 0x18000],
            palette: [0; 0x400],
            oam: [0; 0x400],
            frame: [[255; 4]; (SCREEN_WIDTH * SCREEN_HEIGHT) as usize],
        }
    }

    pub(crate) fn start_hblank(&mut self) {
        self.dispstat.set_hblank(true);
    }

    pub(crate) fn start_vblank(&mut self) {
        self.dispstat.set_vblank(true);

        for (i, &palette_index) in self.vram[0..=0x95FF].iter().enumerate() {
            let offset = usize::from(palette_index) * 2;
            let color = u16::from_le_bytes([self.palette[offset], self.palette[offset + 1]]);
            let expand = |shift: u32| {
                let channel = ((color >> shift) & 0x1F) as u8;
                (channel << 3) | (channel >> 2)
            };
            self.frame[i] = [expand(0), expand(5), expand(10), 255];
        }
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
            address as usize & 0x3FF
        })
    }

    fn write_palette(&mut self, address: u32, width: Width, value: u32) {
        self::write_memory(&mut self.palette, address, width, value, |address| {
            address as usize & 0x3FF
        });
    }

    fn read_vram(&self, address: u32, width: Width) -> u32 {
        self::read_memory(&self.vram, address, width, self::vram_offset)
    }

    fn write_vram(&mut self, address: u32, width: Width, value: u32) {
        self::write_memory(&mut self.vram, address, width, value, self::vram_offset);
    }

    fn read_oam(&self, address: u32, width: Width) -> u32 {
        self::read_memory(&self.oam, address, width, |address| {
            address as usize & 0x3FF
        })
    }

    fn write_oam(&mut self, address: u32, width: Width, value: u32) {
        self::write_memory(&mut self.oam, address, width, value, |address| {
            address as usize & 0x3FF
        });
    }

    pub fn frame(&self) -> &[[u8; 4]; (SCREEN_WIDTH * SCREEN_HEIGHT) as usize] {
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

fn vram_offset(address: u32) -> usize {
    // Each 128 KiB window repeats the last 32 KiB of the 96 KiB VRAM.
    let offset = address as usize & 0x1FFFF;
    if offset >= 0x18000 {
        offset - 0x8000
    } else {
        offset
    }
}
