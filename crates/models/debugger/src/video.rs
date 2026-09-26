use std::sync::Arc;

use crate::{Field, MemoryAddress};

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct VideoResourceId(Arc<str>);

impl VideoResourceId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for VideoResourceId {
    fn from(value: &str) -> Self {
        Self(value.into())
    }
}

impl From<String> for VideoResourceId {
    fn from(value: String) -> Self {
        Self(value.into())
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum VideoResourceKind {
    Palette,
    Tileset,
    Tilemap,
    Sprites,
    Raster,
}

#[derive(Clone, Debug)]
pub struct VideoResource {
    pub id: VideoResourceId,
    pub name: Arc<str>,
    pub kind: VideoResourceKind,
    pub enabled: bool,
}

impl VideoResource {
    pub fn new(
        id: impl Into<VideoResourceId>,
        name: impl Into<Arc<str>>,
        kind: VideoResourceKind,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            kind,
            enabled: true,
        }
    }
}

/// The selected palette isn't part of these, so tileset caches need to add it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct VideoRevision {
    pub pixels: u64,
    pub metadata: u64,
    pub overlay: u64,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PaletteSelection {
    pub resource: VideoResourceId,
    pub color: usize,
}

pub trait VideoDebug {
    fn resources(&self) -> Vec<VideoResource>;

    fn revision(&self, _resource: &VideoResourceId) -> Option<VideoRevision> {
        None
    }

    fn palette(&self, _resource: &VideoResourceId) -> Option<PaletteView> {
        None
    }

    fn tileset(
        &self,
        _resource: &VideoResourceId,
        _palette: Option<&PaletteSelection>,
    ) -> Option<Tileset> {
        None
    }

    fn tilemap(&self, _resource: &VideoResourceId) -> Option<Tilemap> {
        None
    }

    fn tilemap_overlays(&self, _resource: &VideoResourceId) -> Vec<TilemapOverlay> {
        Vec::new()
    }

    fn tilemap_viewport(&self, _resource: &VideoResourceId) -> Option<TilemapViewport> {
        None
    }

    fn sprites(&self, _resource: &VideoResourceId) -> Option<Sprites> {
        None
    }

    fn raster(&self, _resource: &VideoResourceId) -> Option<RasterImage> {
        None
    }
}

#[derive(Clone)]
pub struct TilesetTile {
    pub address: MemoryAddress,
    pub pixels: Vec<[u8; 4]>,
    pub details: Vec<Field>,
}

#[derive(Clone)]
pub struct Tileset {
    pub bpp: u8,
    pub tile_bytes: u32,
    pub tile_width: u32,
    pub tile_height: u32,
    pub tiles: Vec<TilesetTile>,
}

#[derive(Clone)]
pub struct TilemapEntry {
    pub address: MemoryAddress,
    pub pixels: Vec<[u8; 4]>,
    pub details: Vec<Field>,
}

/// `transform` is a row-major 2x3 affine matrix [a, b, tx, c, d, ty].
#[derive(Clone, Copy)]
pub struct TilemapViewport {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    pub transform: Option<[f32; 6]>,
}

#[derive(Clone)]
pub struct TilemapOverlayBand {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    pub inverted: bool,
    pub active: bool,
}

#[derive(Clone)]
pub struct TilemapOverlay {
    pub label: Arc<str>,
    pub slot: usize,
    pub bands: Vec<TilemapOverlayBand>,
}

#[derive(Clone)]
pub struct Tilemap {
    pub width: usize,
    pub height: usize,
    pub tile_px: u8,
    pub entries: Vec<TilemapEntry>,
    pub viewport: Option<TilemapViewport>,
}

#[derive(Clone)]
pub struct SpriteEntry {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<[u8; 4]>,
    pub details: Vec<Field>,
}

#[derive(Clone)]
pub struct Sprites {
    pub entries: Vec<SpriteEntry>,
}

#[derive(Clone, Copy)]
pub struct PaletteColor {
    pub raw: u32,
    pub rgb: (u8, u8, u8),
}

#[derive(Clone, Default)]
pub struct PaletteView {
    pub palettes: Vec<Vec<PaletteColor>>,
    pub meta: Arc<str>,
}

impl PaletteView {
    pub fn color_index(&self, bank: usize, color: usize) -> Option<usize> {
        self.palettes.get(bank)?.get(color)?;
        self.palettes[..bank]
            .iter()
            .map(Vec::len)
            .sum::<usize>()
            .checked_add(color)
    }
}

#[derive(Clone)]
pub struct RasterImage {
    pub width: u32,
    pub height: u32,
    pub pixels: Arc<[[u8; 4]]>,
    pub details: Vec<Field>,
}

impl RasterImage {
    pub fn is_valid(&self) -> bool {
        self.width > 0
            && self.height > 0
            && (self.width as usize).checked_mul(self.height as usize) == Some(self.pixels.len())
    }
}
