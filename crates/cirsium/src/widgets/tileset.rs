use crate::assets::icon;
use crate::components::{footer_row, mono_text};
use crate::widgets::grid::{GridContent, GridPanel, build_atlas};
use debugger_model::{Tileset, TilesetTile};
use gpui::prelude::*;
use gpui::{Div, RenderImage, div};
use std::sync::Arc;

const COLS: usize = 16;

#[derive(Clone)]
pub struct TilesetView {
    pub bpp: u8,
    pub tile_bytes: u32,
    pub cell_px: u32,
    pub tiles: Arc<[TilesetTile]>,
    pub image: Option<Arc<RenderImage>>,
    detail_rows: usize,
}

impl From<Tileset> for TilesetView {
    fn from(mut tileset: Tileset) -> Self {
        let rows = tileset.tiles.len().div_ceil(COLS);
        let cell = tileset.tile_width.max(tileset.tile_height).max(1);
        let image = (rows > 0).then(|| {
            build_atlas(
                COLS,
                rows,
                cell,
                tileset
                    .tiles
                    .iter()
                    .map(|t| (t.pixels.as_slice(), tileset.tile_width, tileset.tile_height)),
            )
        });

        for tile in &mut tileset.tiles {
            tile.pixels = Vec::new();
        }

        Self {
            bpp: tileset.bpp,
            tile_bytes: tileset.tile_bytes,
            cell_px: cell,
            detail_rows: detail_rows(tileset.tiles.iter().map(|t| t.details.as_slice())),
            tiles: tileset.tiles.into(),
            image,
        }
    }
}

pub struct TilesetWidget;

impl TilesetWidget {
    pub const ID: &'static str = debugger_model::panel::TILESET;

    pub fn panel(view: Option<TilesetView>) -> GridPanel {
        let content = view.as_ref().map(|v| GridContent {
            image: v.image.clone(),
            cols: COLS,
            rows: v.tiles.len().div_ceil(COLS),
            tile_px: v.cell_px,
            count: v.tiles.len(),
            empty_message: "No tiles",
            overlay: None,
            footprint: None,
            window_overlays: vec![],
        });

        let mut panel = GridPanel::new(Self::ID, "Tileset", icon::GRID, content)
            .side_details(224.)
            .placeholder("Select a tile to inspect");
        if let Some(view) = view {
            panel = panel
                .meta(format!("{}bpp", view.bpp))
                .detail_rows(view.detail_rows)
                .detail(move |theme, index| {
                    let tile = &view.tiles[index];
                    div()
                        .flex()
                        .flex_col()
                        .child(footer_row(
                            theme,
                            format!("Tile {index}"),
                            mono_text(
                                theme,
                                format!(
                                    "{} bytes @ {}",
                                    view.tile_bytes,
                                    debugger_model::hex(tile.address.addr, 16)
                                ),
                            ),
                        ))
                        .child(details(theme, &tile.details))
                });
        }

        panel
    }
}

pub(super) fn detail_rows<'a>(fields: impl Iterator<Item = &'a [debugger_model::Field]>) -> usize {
    fields.map(|f| f.len() + 1).max().unwrap_or(1)
}

pub(super) fn details(theme: &crate::Theme, fields: &[debugger_model::Field]) -> Div {
    fields.iter().fold(div().flex().flex_col(), |d, f| {
        d.child(footer_row(
            theme,
            f.label.clone(),
            mono_text(theme, f.value.clone()),
        ))
    })
}
