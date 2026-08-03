use super::SuperFx;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Default, Serialize, Deserialize)]
pub struct PixelCache {
    pub x: u8,
    pub y: u8,
    pub pixels: [u8; 8],
    pub valid: u8,
}

impl PixelCache {
    pub(super) fn covers(&self, x: u8, y: u8) -> bool {
        self.x == x & 0xF8 && self.y == y
    }

    pub(super) fn insert(&mut self, x: u8, color: u8) {
        let bit = self::pixel_bit(x);
        self.pixels[bit as usize] = color;
        self.valid |= 1 << bit;
    }

    pub(super) fn full(&self) -> bool {
        self.valid == 0xFF
    }

    pub(super) fn restart(&mut self, x: u8, y: u8) {
        self.valid = 0;
        self.x = x & 0xF8;
        self.y = y;
    }

    pub(super) fn plane_bits(&self, plane: u32) -> u8 {
        let mut bits = 0;
        for x in 0..8 {
            bits |= ((self.pixels[x] >> plane) & 0x01) << x;
        }
        bits
    }
}

pub(crate) fn pixel_bit(x: u8) -> u8 {
    (x & 7) ^ 7
}

pub(crate) struct Screen {
    base: u32,
    height: Height,
    bpp: u32,
}

enum Height {
    Pixels128,
    Pixels160,
    Pixels192,
    Obj,
}

impl Screen {
    #[cfg(feature = "debug")]
    pub(crate) fn at_last_stop(fx: &SuperFx) -> Self {
        use super::regs::{Por, Scmr};

        let Some((scbr, scmr, por)) = fx.stopped_screen else {
            return Self::of(fx);
        };
        Self::from_regs(scbr, &Scmr::from_raw(scmr), &Por::from_raw(por))
    }

    pub(crate) fn of(fx: &SuperFx) -> Self {
        Self::from_regs(fx.scbr, &fx.scmr, &fx.por)
    }

    fn from_regs(scbr: u8, scmr: &super::regs::Scmr, por: &super::regs::Por) -> Self {
        let height = match (por.obj_mode(), scmr.height()) {
            (true, _) | (false, 3) => Height::Obj,
            (false, 0) => Height::Pixels128,
            (false, 1) => Height::Pixels160,
            _ => Height::Pixels192,
        };

        Self {
            base: (scbr as u32) << 10,
            height,
            bpp: scmr.bpp(),
        }
    }

    pub(crate) fn tile_row(&self, x: u8, y: u8) -> TileRow {
        let tile = self.tile_index(x, y);
        TileRow {
            base: self.base + tile * (self.bpp << 3) + (y as u32 & 0x07) * 2,
            bpp: self.bpp,
        }
    }

    #[cfg(feature = "debug")]
    pub(crate) fn bpp(&self) -> u32 {
        self.bpp
    }

    #[cfg(feature = "debug")]
    pub(crate) fn tile_grid(&self) -> (u32, u32) {
        let rows = match self.height {
            Height::Pixels128 => 16,
            Height::Pixels160 => 20,
            Height::Pixels192 => 24,
            Height::Obj => 32,
        };
        (32, rows)
    }

    fn tile_index(&self, x: u8, y: u8) -> u32 {
        let (col, row) = ((x >> 3) as u32, (y >> 3) as u32);

        match self.height {
            Height::Pixels128 => col * 16 + row,
            Height::Pixels160 => col * 20 + row,
            Height::Pixels192 => col * 24 + row,
            Height::Obj => {
                let quadrant = ((y as u32 & 0x80) << 2) | ((x as u32 & 0x80) << 1);
                quadrant | ((row & 0x0F) << 4) | (col & 0x0F)
            }
        }
    }
}

pub(crate) struct TileRow {
    base: u32,
    bpp: u32,
}

pub(crate) struct Plane {
    pub(crate) index: u32,
    pub(crate) addr: u32,
}

impl TileRow {
    pub(crate) fn planes(&self) -> impl Iterator<Item = Plane> + '_ {
        (0..self.bpp).map(|index| Plane {
            index,
            addr: self.base + ((index >> 1) << 4) + (index & 0x01),
        })
    }
}
