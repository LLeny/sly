use crate::ppu::regs::TileSize;

#[derive(Default, Clone, Copy)]
pub struct BackgroundLayer {
    pub tileset_addr: u32,
    pub tile_size: TileSize,
}
