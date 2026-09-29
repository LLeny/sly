pub(crate) mod io;
pub(crate) mod regs;

use crate::bus::Width;
use regs::{DisplayStatus, VerticalCount};

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
