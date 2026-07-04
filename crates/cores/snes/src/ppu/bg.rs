use crate::ppu::regs::{TileSize, TilemapCount};

#[derive(Default, Clone, Copy)]
pub struct BackgroundLayer {
    pub tileset_addr: u32,
    pub tilemap_addr: u32,
    pub horizontal_tilemap_count: TilemapCount,
    pub vertical_tilemap_count: TilemapCount,
    pub tile_size: TileSize,
}
