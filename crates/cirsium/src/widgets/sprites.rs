use std::sync::Arc;

use gpui::prelude::*;
use gpui::{Div, RenderImage, SharedString, div};

use crate::assets::icon;
use crate::components::{flip_label, footer_row, hex, mono_text};
use crate::theme::Theme;
use crate::widgets::grid::{GridContent, GridPanel, build_atlas};

const COLS: usize = 16;

#[derive(Clone)]
pub struct SpritesView {
    pub cell_px: u32,
    pub meta: SharedString,
    pub entries: Arc<[debugger_model::SpriteEntry]>,
    pub image: Option<Arc<RenderImage>>,
}

impl From<debugger_model::Sprites> for SpritesView {
    fn from(sprites: debugger_model::Sprites) -> Self {
        let (small_w, small_h) = sprites.small;
        let (large_w, large_h) = sprites.large;
        let cell = large_w.max(large_h).max(8);
        let rows = sprites.entries.len().div_ceil(COLS);
        let scale = (64 / cell).max(1);

        let image = (rows > 0).then(|| {
            build_atlas(
                COLS,
                rows,
                cell,
                scale,
                sprites
                    .entries
                    .iter()
                    .map(|sprite| (sprite.pixels.as_slice(), sprite.width, sprite.height)),
            )
        });

        Self {
            cell_px: cell,
            meta: format!("{small_w}×{small_h} / {large_w}×{large_h}").into(),
            entries: sprites.entries.into(),
            image,
        }
    }
}

pub struct SpritesWidget;

impl SpritesWidget {
    pub const ID: &'static str = "sprites";

    pub fn panel(view: Option<SpritesView>) -> GridPanel {
        let content = view.as_ref().map(|view| GridContent {
            image: view.image.clone(),
            cols: COLS,
            rows: view.entries.len().div_ceil(COLS),
            tile_px: view.cell_px,
            count: view.entries.len(),
            empty_message: "No sprites",
            overlay: None,
        });

        let mut panel = GridPanel::new(Self::ID, "Sprites", icon::GHOST, content)
            .placeholder("Select a sprite to inspect")
            .detail_rows(2);
        if let Some(view) = view {
            panel = panel
                .meta(view.meta.clone())
                .detail(move |theme, index| self::detail_row(theme, &view, index));
        }
        panel
    }
}

fn detail_row(theme: &Theme, view: &SpritesView, index: usize) -> Div {
    let sprite = &view.entries[index];

    let flip = flip_label(sprite.flip_h, sprite.flip_v);

    div()
        .flex()
        .flex_col()
        .child(footer_row(
            theme,
            format!("Sprite {index} │ {}×{}", sprite.width, sprite.height),
            mono_text(theme, format!("({}, {}){flip}", sprite.x, sprite.y)),
        ))
        .child(footer_row(
            theme,
            format!("Tile {}", hex(sprite.tile as u32, 12)),
            mono_text(
                theme,
                format!(
                    "pal {} │ prio {} @ VRAM {}",
                    sprite.palette,
                    sprite.priority,
                    hex(sprite.tile_addr, 16),
                ),
            ),
        ))
}
