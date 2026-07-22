use std::sync::Arc;

use gpui::RenderImage;

use crate::assets::icon;
use crate::components::{footer_row, hex, mono_text};
use crate::widgets::grid::{GridContent, GridPanel, build_atlas};

const TILE_SIZE: u32 = 8;
const TILES_PER_ROW: usize = 16;

#[derive(Clone)]
pub struct TilesetView {
    pub bpp: u8,
    pub tile_bytes: u32,
    pub addrs: Arc<[u32]>,
    pub image: Option<Arc<RenderImage>>,
}

impl TilesetView {
    fn rows(&self) -> usize {
        self.addrs.len().div_ceil(TILES_PER_ROW)
    }
}

impl From<debugger_model::Tileset> for TilesetView {
    fn from(tileset: debugger_model::Tileset) -> Self {
        let rows = tileset.tiles.len().div_ceil(TILES_PER_ROW);
        let image = (rows > 0).then(|| {
            build_atlas(
                TILES_PER_ROW,
                rows,
                TILE_SIZE,
                tileset
                    .tiles
                    .iter()
                    .map(|tile| (tile.pixels.as_slice(), TILE_SIZE, TILE_SIZE)),
            )
        });

        Self {
            bpp: tileset.bpp,
            tile_bytes: tileset.tile_bytes,
            addrs: tileset.tiles.iter().map(|tile| tile.addr).collect(),
            image,
        }
    }
}

pub struct TilesetWidget;

impl TilesetWidget {
    pub const ID: &'static str = "tileset";

    pub fn panel(view: Option<TilesetView>) -> GridPanel {
        let content = view.as_ref().map(|view| GridContent {
            image: view.image.clone(),
            cols: TILES_PER_ROW,
            rows: view.rows(),
            tile_px: TILE_SIZE,
            count: view.addrs.len(),
            empty_message: "No tiles",
            overlay: None,
            window_overlays: Vec::new(),
        });

        let mut panel = GridPanel::new(Self::ID, "Tileset", icon::GRID, content)
            .placeholder("Select a tile to inspect");
        if let Some(view) = view {
            panel = panel
                .meta(format!("{}bpp", view.bpp))
                .detail(move |theme, index| {
                    footer_row(
                        theme,
                        format!("Tile {index} ({})", hex(index as u32, 8)),
                        mono_text(
                            theme,
                            format!(
                                "{} bytes @ VRAM {}",
                                view.tile_bytes,
                                hex(view.addrs[index], 16)
                            ),
                        ),
                    )
                });
        }
        panel
    }
}
