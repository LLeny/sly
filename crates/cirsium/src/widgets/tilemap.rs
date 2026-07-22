use std::sync::Arc;

use gpui::{Div, RenderImage};

use crate::assets::icon;
use crate::components::{divided_mono, flip_label, footer_row, hex};
use crate::theme::Theme;
use crate::widgets::grid::{
    GridContent, GridOverlay, GridPanel, GridWindowBand, GridWindowOverlay, build_atlas,
};

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
    overlays: Arc<[debugger_model::TilemapOverlay]>,
}

impl TilemapView {
    pub fn set_overlays(&mut self, overlays: Vec<debugger_model::TilemapOverlay>) {
        self.overlays = overlays.into();
    }

    fn outside_window_band(viewport: GridOverlay, cutout: GridOverlay) -> Vec<GridOverlay> {
        let x1 = cutout.x.min(viewport.width);
        let y1 = cutout.y.min(viewport.height);
        let x2 = cutout.x.saturating_add(cutout.width).min(viewport.width);
        let y2 = cutout.y.saturating_add(cutout.height).min(viewport.height);

        if y1 >= y2 {
            return Vec::new();
        }
        if x1 >= x2 {
            return vec![GridOverlay {
                x: viewport.x,
                y: viewport.y + y1,
                width: viewport.width,
                height: y2 - y1,
            }];
        }

        let mut rects = Vec::with_capacity(2);
        let mut push = |x: u32, width: u32| {
            if width > 0 {
                rects.push(GridOverlay {
                    x: viewport.x + x,
                    y: viewport.y + y1,
                    width,
                    height: y2 - y1,
                });
            }
        };

        push(0, x1);
        push(x2, viewport.width - x2);
        rects
    }

    fn window_overlays(&self) -> Vec<GridWindowOverlay> {
        let Some(viewport) = self.viewport else {
            return Vec::new();
        };

        self.overlays
            .iter()
            .map(|overlay| {
                let mut effective = Vec::new();
                let bands: Vec<_> = overlay
                    .bands
                    .iter()
                    .map(|band| {
                        let relative = GridOverlay {
                            x: band.x,
                            y: band.y,
                            width: band.width,
                            height: band.height,
                        };
                        let rect = GridOverlay {
                            x: viewport.x + band.x,
                            y: viewport.y + band.y,
                            width: band.width,
                            height: band.height,
                        };
                        if band.active {
                            if band.inverted {
                                effective.extend(Self::outside_window_band(viewport, relative));
                            } else if band.width > 0 {
                                effective.push(rect);
                            }
                        }

                        GridWindowBand {
                            rect,
                            active: band.active,
                            show_edges: !(band.active
                                && band.inverted
                                && band.x == 0
                                && band.width >= viewport.width),
                        }
                    })
                    .collect();
                let label_anchor = bands
                    .iter()
                    .filter(|band| band.rect.width > 0)
                    .max_by_key(|band| (band.active, band.rect.width))
                    .map(|band| band.rect);

                GridWindowOverlay {
                    bands,
                    effective,
                    label_anchor,
                    label: overlay.label.to_string().into(),
                    slot: overlay.slot,
                }
            })
            .collect()
    }
}

impl From<debugger_model::Tilemap> for TilemapView {
    fn from(map: debugger_model::Tilemap) -> Self {
        let tile_px = map.tile_px as u32;

        let image = (!map.entries.is_empty()).then(|| {
            build_atlas(
                map.width,
                map.height,
                tile_px,
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
            overlays: Vec::new().into(),
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
            window_overlays: view.window_overlays(),
        });

        let mut panel = GridPanel::new(Self::ID, "Tilemap", icon::MAP, content)
            .window_overlay_toggle(true)
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

    let mut segs = vec![
        hex(entry.raw as u32, 16),
        format!("pal {}", entry.palette),
        format!("prio {}", entry.priority as u8),
    ];
    if let Some(flip) = flip_label(entry.flip_h, entry.flip_v) {
        segs.push(flip.to_string());
    }
    if let Some(last) = segs.last_mut() {
        *last = format!("{last} @ VRAM {}", hex(entry.addr, 16));
    }

    footer_row(
        theme,
        format!("({x}, {y}) Tile {}", hex(entry.tile_index as u32, 12)),
        divided_mono(theme, segs),
    )
}
