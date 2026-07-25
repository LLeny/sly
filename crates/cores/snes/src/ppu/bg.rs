use crate::ppu::regs::{TileSize, TilemapCount};
use serde::{Deserialize, Serialize};

#[derive(Default, Clone, Copy, Serialize, Deserialize)]
pub struct BackgroundLayer {
    pub tileset_addr: u32,
    pub tilemap_addr: u32,
    pub horizontal_tilemap_count: TilemapCount,
    pub vertical_tilemap_count: TilemapCount,
    pub tile_size: TileSize,
    pub hofs: u16,
    pub vofs: u16,
}

impl BackgroundLayer {
    pub fn scroll_x(&self) -> u32 {
        (self.hofs & 0x3FF) as u32
    }

    pub fn scroll_y(&self) -> u32 {
        (self.vofs & 0x3FF) as u32
    }
}
