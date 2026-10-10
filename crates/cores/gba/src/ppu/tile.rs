#[chapa::bitenum]
pub enum ColorDepth {
    Bpp4 = 0, // 16 colors, 16 palettes
    Bpp8 = 1, // 256 colors, 1 palette

    #[fallback]
    Prohibited = 2,
}

#[chapa::bitenum]
pub enum Size {
    Size0 = 0,
    Size1 = 1,
    Size2 = 2,
    Size3 = 3,

    #[fallback]
    Prohibited = 4,
}

impl Size {
    pub fn text_dimensions(self) -> Option<(usize, usize)> {
        match self {
            Self::Size0 => Some((256, 256)),
            Self::Size1 => Some((512, 256)),
            Self::Size2 => Some((256, 512)),
            Self::Size3 => Some((512, 512)),
            Self::Prohibited => None,
        }
    }

    pub fn affine_dimensions(self) -> Option<(usize, usize)> {
        let side = match self {
            Self::Size0 => 128,
            Self::Size1 => 256,
            Self::Size2 => 512,
            Self::Size3 => 1024,
            Self::Prohibited => return None,
        };
        Some((side, side))
    }
}

impl std::fmt::Display for ColorDepth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Bpp4 => "4bpp",
            Self::Bpp8 => "8bpp",
            Self::Prohibited => unreachable!(),
        })
    }
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
