pub(crate) mod io;
pub(crate) mod regs;

use crate::bus::Width;
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
        }
    }

    pub(crate) fn start_hblank(&mut self) {
        self.dispstat.set_hblank(true);
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

    pub(crate) fn peek(&self, address: u32, width: Width) -> u32 {
        tracing::error!(address = %format_args!("{address:#010X}"), ?width, "Peeking at PPU");
        0xFF
    }

    pub(crate) fn read(&mut self, address: u32, width: Width) -> u32 {
        tracing::error!(address = %format_args!("{address:#010X}"), ?width, "Reading from PPU");
        0xFF
    }

    pub(crate) fn write(&mut self, address: u32, width: Width, value: u32) {
        tracing::error!(
            address = %format_args!("{address:#010X}"),
            ?width,
            value = %format_args!("{value:#010X}"),
            "Writing to PPU"
        );
    }
}
