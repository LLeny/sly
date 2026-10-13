use crate::ppu::regs::SpriteMapping;
use crate::ppu::tile::{ColorDepth, Size};

pub const OAM_ATTR_SIZE: usize = 8; // 3 attributes plus 1 affine parameter halfword
pub const OBJ_TILE_OFFSET: usize = 0x10000;
pub const OBJ_TILE_BYTES: usize = 32; // tile numbers always count 32-byte units
pub const OBJ_PALETTE_BASE: usize = 256;

// I should probably go to jail for this
pub fn attributes(oam: &[u8]) -> impl DoubleEndedIterator<Item = Attribute> + '_ {
    oam.as_chunks::<OAM_ATTR_SIZE>()
        .0
        .iter()
        .map(|&bytes| Attribute::from_le_bytes(bytes))
}

#[chapa::bitenum]
pub enum ObjMode {
    Normal = 0,
    SemiTransparent = 1,
    ObjWindow = 2,

    #[fallback]
    Prohibited = 3,
}

#[chapa::bitenum]
pub enum Shape {
    Square = 0,
    Horizontal = 1,
    Vertical = 2,

    #[fallback]
    Prohibited = 3,
}

#[chapa::bitfield(u64, order = lsb0)]
pub struct Attribute {
    #[bits(0..=7)]
    pub y_coordinate: u8,

    #[bits(8)]
    pub affine: bool,

    #[bits(9, overlay = "affine")]
    pub double_size: bool,
    #[bits(9, overlay = "not_affine")]
    pub disable_obj: bool,

    #[bits(10..=11)]
    pub mode: ObjMode,

    #[bits(12)]
    pub mosaic: bool,

    #[bits(13)]
    pub color_depth: ColorDepth,

    #[bits(14..=15)]
    pub shape: Shape,

    #[bits(16..=24)]
    pub x_coordinate: u16,

    #[bits(25..=29, overlay = "affine")]
    pub matrix_index: u8,

    #[bits(28, overlay = "not_affine")]
    pub horizontal_flip: bool,

    #[bits(29, overlay = "not_affine")]
    pub vertical_flip: bool,

    #[bits(30..=31)]
    pub size: Size,

    #[bits(32..=41)]
    pub tile_number: u16,

    #[bits(42..=43)]
    pub priority: u8, // relative to BG

    #[bits(44..=47)]
    pub palette_number: u8, // not used in 256 color/1 palette mode
}

impl Attribute {
    pub fn dimensions(&self) -> (usize, usize) {
        use Shape::*;
        use Size::*;

        match (self.shape(), self.size()) {
            (Square, Size0) => (8, 8),
            (Square, Size1) => (16, 16),
            (Square, Size2) => (32, 32),
            (Square, Size3) => (64, 64),
            (Horizontal, Size0) => (16, 8),
            (Horizontal, Size1) => (32, 8),
            (Horizontal, Size2) => (32, 16),
            (Horizontal, Size3) => (64, 32),
            (Vertical, Size0) => (8, 16),
            (Vertical, Size1) => (8, 32),
            (Vertical, Size2) => (16, 32),
            (Vertical, Size3) => (32, 64),
            // The prohibited shape has no size, so the sprite is never drawn
            // First found in: Wario Land 4 shits itself when loading into a game if I don't support this
            _ => (0, 0),
        }
    }

    pub fn bounds(&self) -> (usize, usize) {
        let (width, height) = self.dimensions();
        if self.affine() && self.double_size() {
            (width * 2, height * 2)
        } else {
            (width, height)
        }
    }

    pub fn flip(&self, x: usize, y: usize) -> (usize, usize) {
        let (width, height) = self.dimensions();

        let x = if !self.affine() && self.horizontal_flip() {
            width - 1 - x
        } else {
            x
        };
        let y = if !self.affine() && self.vertical_flip() {
            height - 1 - y
        } else {
            y
        };

        (x, y)
    }

    pub fn tile_addr(&self, mapping: SpriteMapping, tx: usize, ty: usize) -> usize {
        let (width, _) = self.dimensions();
        let tile_bytes = self.color_depth().tile_size();
        let row_tiles = match mapping {
            SpriteMapping::OneDimensional => width / 8,
            SpriteMapping::TwoDimensional => 1024 / tile_bytes,
        };

        OBJ_TILE_OFFSET
            + (self.tile_number() as usize * OBJ_TILE_BYTES)
            + (((ty * row_tiles) + tx) * tile_bytes)
    }

    pub fn palette_base(&self) -> usize {
        OBJ_PALETTE_BASE
            + self
                .color_depth()
                .palette_base(self.palette_number() as usize)
    }
}
