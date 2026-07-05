use crate::ppu::regs::SpriteSize;

#[derive(Default, Clone, Copy)]
pub struct ObjLayer {
    pub tileset_addr: u32,
    pub name_offset: u32,
    pub sprite_size: SpriteSize,
}

impl ObjLayer {
    pub fn tile_addr(&self, tile: u16) -> u32 {
        let mut addr = self.tileset_addr;
        if tile & 0x100 != 0 {
            addr += 0x1000 + self.name_offset;
        }

        (addr + (tile as u32 & 0xFF) * 16) & 0x7FFF
    }
}
