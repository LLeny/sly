use crate::ppu::regs::BackgroundMode;
use crate::ppu::vram::Vram;
use crate::ppu::{Ppu, TilemapData};

const VERTICAL_ROW_OFFSET: u32 = 8;
const FIRST_OVERRIDDEN_COLUMN: u32 = 8;
const LAYERS: usize = 2;

#[chapa::bitfield(u16, order = lsb0)]
struct OffsetEntry {
    #[bits(0..=9)]
    scroll: u16,

    #[bits(13..=14)]
    overridden_layers: u8,

    #[bits(15)]
    vertical: bool,
}

pub(super) struct OffsetPerTile {
    map: TilemapData,
    cell_width: u32,
    packed_directions: bool,
    layer: usize,
    scroll_x: u32,
    scroll_y: u32,
}

pub(super) struct ColumnScroll {
    pub horizontal: u32,
    pub vertical: u32,
}

impl OffsetPerTile {
    pub(super) fn new(ppu: &Ppu, layer: usize) -> Option<Self> {
        if layer >= LAYERS {
            return None;
        }

        let packed_directions = match ppu.bg_mode {
            BackgroundMode::Mode2 | BackgroundMode::Mode6 => false,
            BackgroundMode::Mode4 => true,
            _ => return None,
        };

        let map = ppu.tilemap_geometry(2);
        let cell_width = if ppu.bg_mode.is_hires() {
            8
        } else {
            map.tile_px
        };
        let target = &ppu.bg_layers[layer];

        Some(Self {
            map,
            cell_width,
            packed_directions,
            layer,
            scroll_x: target.scroll_x(),
            scroll_y: target.scroll_y(),
        })
    }

    pub(super) fn column(&self, x: u32) -> Option<u32> {
        let x = (x + (self.scroll_x & 7)).checked_sub(FIRST_OVERRIDDEN_COLUMN)?
            + (self.map.scroll_x & !7);
        Some((x / self.cell_width) % self.map.width)
    }

    pub(super) fn column_scroll(&self, vram: &Vram, column: u32) -> ColumnScroll {
        let mut scroll = ColumnScroll {
            horizontal: self.scroll_x,
            vertical: self.scroll_y,
        };

        let first = self.entry(vram, column, self.map.scroll_y);
        if self.overrides(first) {
            if self.packed_directions && first.vertical() {
                scroll.vertical = first.scroll() as u32;
            } else {
                scroll.horizontal = (first.scroll() as u32 & !7) | (self.scroll_x & 7);
            }
        }

        if !self.packed_directions {
            let second = self.entry(vram, column, self.map.scroll_y + VERTICAL_ROW_OFFSET);
            if self.overrides(second) {
                scroll.vertical = second.scroll() as u32;
            }
        }

        scroll
    }

    fn overrides(&self, entry: OffsetEntry) -> bool {
        entry.overridden_layers() & (1 << self.layer) != 0
    }

    fn entry(&self, vram: &Vram, column: u32, y: u32) -> OffsetEntry {
        let row = (y / self.map.tile_px) % self.map.height;
        OffsetEntry::from_raw(self.map.entry(vram, column, row))
    }
}
