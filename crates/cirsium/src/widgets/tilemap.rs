use std::sync::Arc;

use gpui::{Div, RenderImage};

use crate::assets::icon;
use crate::components::{flip_label, footer_row, hex, mono_text};
use crate::theme::Theme;
use crate::widgets::grid::{GridContent, GridOverlay, GridPanel, build_atlas};

#[derive(Clone)]
pub struct TilemapEntryView {
    pub addr: u32,
    pub raw: u16,
    pub tile_index: u16,
    pub palette: u8,
    pub priority: bool,
    pub flip_h: bool,
    pub flip_v: bool,
}

impl From<&debugger_model::TilemapEntry> for TilemapEntryView {
    fn from(entry: &debugger_model::TilemapEntry) -> Self {
        Self {
            addr: entry.addr,
            raw: entry.raw,
            tile_index: entry.tile_index,
            palette: entry.palette,
            priority: entry.priority,
            flip_h: entry.flip_h,
            flip_v: entry.flip_v,
        }
    }
}

#[derive(Clone)]
pub struct TilemapView {
    pub width: usize,
    pub height: usize,
    pub tile_px: u8,
    pub entries: Arc<[TilemapEntryView]>,
    pub image: Option<Arc<RenderImage>>,
    pub viewport: Option<GridOverlay>,
}

impl From<debugger_model::Tilemap> for TilemapView {
    fn from(map: debugger_model::Tilemap) -> Self {
        let tile_px = map.tile_px as u32;
        let scale = (1024 / (map.width as u32 * tile_px).max(1)).max(1);

        let image = (!map.entries.is_empty()).then(|| {
            build_atlas(
                map.width,
                map.height,
                tile_px,
                scale,
                map.entries
                    .iter()
                    .map(|entry| (entry.pixels.as_slice(), tile_px, tile_px)),
            )
        });

        Self {
            width: map.width,
            height: map.height,
            tile_px: map.tile_px,
            entries: map.entries.iter().map(Into::into).collect(),
            image,
            viewport: map.viewport.map(Into::into),
        }
    }
}

pub struct TilemapWidget;

impl TilemapWidget {
    pub const ID: &'static str = "tilemap";

    pub fn panel(view: Option<TilemapView>) -> GridPanel {
        let content = view.as_ref().map(|view| GridContent {
            image: view.image.clone(),
            cols: view.width,
            rows: view.height,
            tile_px: view.tile_px as u32,
            count: view.entries.len(),
            empty_message: "No tilemap",
            overlay: view.viewport,
        });

        let mut panel = GridPanel::new(Self::ID, "Tilemap", icon::MAP, content)
            .placeholder("Select an entry to inspect");
        if let Some(view) = view {
            panel = panel
                .meta(format!("{}×{}", view.width, view.height))
                .detail(move |theme, index| self::detail_row(theme, &view, index));
        }
        panel
    }
}

fn detail_row(theme: &Theme, view: &TilemapView, index: usize) -> Div {
    let entry = &view.entries[index];
    let (x, y) = (index % view.width, index / view.width);

    let flip = flip_label(entry.flip_h, entry.flip_v);

    footer_row(
        theme,
        format!("({x}, {y}) Tile {}", hex(entry.tile_index as u32, 12)),
        mono_text(
            theme,
            format!(
                "{} │ pal {} │ prio {}{flip} @ VRAM {}",
                hex(entry.raw as u32, 16),
                entry.palette,
                entry.priority as u8,
                hex(entry.addr, 16),
            ),
        ),
    )
}
