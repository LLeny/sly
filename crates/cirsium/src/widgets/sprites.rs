use crate::assets::icon;
use crate::components::{footer_row, mono_text};
use crate::widgets::grid::{GridContent, GridPanel, build_atlas};
use gpui::prelude::*;
use gpui::{RenderImage, div};
use std::sync::Arc;
const COLS: usize = 16;

#[derive(Clone)]
pub struct SpritesView {
    pub cell_px: u32,
    pub entries: Arc<[debugger_model::SpriteEntry]>,
    pub image: Option<Arc<RenderImage>>,
    detail_rows: usize,
}

impl From<debugger_model::Sprites> for SpritesView {
    fn from(mut sprites: debugger_model::Sprites) -> Self {
        let cell = sprites
            .entries
            .iter()
            .map(|s| s.width.max(s.height))
            .max()
            .unwrap_or(1)
            .max(1);

        let rows = sprites.entries.len().div_ceil(COLS);
        let image = (rows > 0).then(|| {
            build_atlas(
                COLS,
                rows,
                cell,
                sprites
                    .entries
                    .iter()
                    .map(|s| (s.pixels.as_slice(), s.width, s.height)),
            )
        });

        for sprite in &mut sprites.entries {
            sprite.pixels = Vec::new();
        }

        Self {
            cell_px: cell,
            detail_rows: super::tileset::detail_rows(
                sprites.entries.iter().map(|e| e.details.as_slice()),
            ),
            entries: sprites.entries.into(),
            image,
        }
    }
}

pub struct SpritesWidget;

impl SpritesWidget {
    pub const ID: &'static str = debugger_model::panel::SPRITES;

    pub fn panel(view: Option<SpritesView>) -> GridPanel {
        let content = view.as_ref().map(|v| GridContent {
            image: v.image.clone(),
            cols: COLS,
            rows: v.entries.len().div_ceil(COLS),
            tile_px: v.cell_px,
            count: v.entries.len(),
            empty_message: "No sprites",
            overlay: None,
            footprint: None,
            window_overlays: vec![],
        });

        let mut panel = GridPanel::new(Self::ID, "Sprites", icon::GHOST, content)
            .placeholder("Select a sprite to inspect");
        if let Some(view) = view {
            panel = panel
                .meta(format!("{} objects", view.entries.len()))
                .detail_rows(view.detail_rows)
                .detail(move |theme, index| {
                    let sprite = &view.entries[index];
                    div()
                        .flex()
                        .flex_col()
                        .child(footer_row(
                            theme,
                            format!("Sprite {index}, {}x{}", sprite.width, sprite.height),
                            mono_text(theme, format!("({}, {})", sprite.x, sprite.y)),
                        ))
                        .child(super::tileset::details(theme, &sprite.details))
                });
        }

        panel
    }
}
