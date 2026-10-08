#[chapa::bitenum]
pub enum ColorDepth {
    Bpp4 = 0, // 16 colors, 16 palettes
    Bpp8 = 1, // 256 colors, 1 palette

    #[fallback]
    Prohibited = 2,
}

impl ColorDepth {
    pub const fn tile_bytes(self) -> usize {
        match self {
            ColorDepth::Bpp4 => 32,
            ColorDepth::Bpp8 => 64,
            ColorDepth::Prohibited => unreachable!(),
        }
    }
}

pub struct Tile<'a> {
    pub data: &'a [u8],
    pub depth: ColorDepth,
}

impl Tile<'_> {
    pub fn pixel(&self, x: usize, y: usize) -> u8 {
        match self.depth {
            ColorDepth::Bpp4 => {
                let byte = self.data[(y * 4) + (x / 2)];
                (byte >> ((x % 2) * 4)) & 0xF
            }
            ColorDepth::Bpp8 => self.data[(y * 8) + x],
            ColorDepth::Prohibited => 0,
        }
    }
}
