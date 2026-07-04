use std::rc::Rc;
use std::sync::Arc;

use gpui::prelude::*;
use gpui::{
    App, ClickEvent, Div, RenderImage, SharedString, Stateful, Window, div, img, px,
    transparent_black,
};

use crate::assets::icon;
use crate::components::{ClickHandler, Panel, Segmented, empty_state, footer_row, hex, mono_text};
use crate::theme::Theme;

type SelectCallback = Rc<dyn Fn(usize, &mut Window, &mut App) + 'static>;

const SCALE: u32 = 2;

const TILE_SIZE: u32 = 8;
const TILES_PER_ROW: usize = 16;

#[derive(Clone)]
pub struct Tile {
    pub addr: SharedString,
    pub image: Arc<RenderImage>,
}

#[derive(Clone)]
pub struct TilesetView {
    pub bpp: u8,
    pub tiles: Vec<Tile>,
}

impl From<debugger_model::Tileset> for TilesetView {
    fn from(tileset: debugger_model::Tileset) -> Self {
        Self {
            bpp: tileset.bpp,
            tiles: tileset.tiles.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<debugger_model::TilesetTile> for Tile {
    fn from(tile: debugger_model::TilesetTile) -> Self {
        let size = TILE_SIZE * SCALE;
        let mut data = image::RgbaImage::new(size, size);

        for (y, row) in data.rows_mut().enumerate() {
            let src_row = y as u32 / SCALE * TILE_SIZE;
            for (x, pixel) in row.enumerate() {
                let (r, g, b) = tile.pixels[(src_row + x as u32 / SCALE) as usize];
                *pixel = image::Rgba([b, g, r, 0xFF]);
            }
        }

        Self {
            addr: SharedString::from(tile.addr),
            image: Arc::new(RenderImage::new(vec![image::Frame::new(data)])),
        }
    }
}

#[derive(IntoElement)]
pub struct TilesetWidget {
    view: Option<TilesetView>,
    layers: Vec<SharedString>,
    layer: usize,
    palettes: usize,
    palette: usize,
    selected: Option<usize>,
    collapsed: bool,
    on_toggle: Option<ClickHandler>,
    on_select: Option<SelectCallback>,
    on_layer_select: Option<SelectCallback>,
    on_palette_select: Option<SelectCallback>,
}

impl TilesetWidget {
    pub const ID: &'static str = "tileset";

    pub fn new(view: Option<TilesetView>) -> Self {
        Self {
            view,
            layers: Vec::new(),
            layer: 0,
            palettes: 0,
            palette: 0,
            selected: None,
            collapsed: false,
            on_toggle: None,
            on_select: None,
            on_layer_select: None,
            on_palette_select: None,
        }
    }

    pub fn layers(mut self, layers: Vec<SharedString>) -> Self {
        self.layers = layers;
        self
    }

    pub fn layer(mut self, layer: usize) -> Self {
        self.layer = layer;
        self
    }

    pub fn palettes(mut self, palettes: usize) -> Self {
        self.palettes = palettes;
        self
    }

    pub fn palette(mut self, palette: usize) -> Self {
        self.palette = palette;
        self
    }

    pub fn selected(mut self, selected: Option<usize>) -> Self {
        self.selected = selected;
        self
    }

    pub fn collapsed(mut self, collapsed: bool) -> Self {
        self.collapsed = collapsed;
        self
    }

    pub fn on_toggle(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_toggle = Some(Box::new(handler));
        self
    }

    pub fn on_select(mut self, handler: impl Fn(usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_select = Some(Rc::new(handler));
        self
    }

    pub fn on_layer_select(
        mut self,
        handler: impl Fn(usize, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_layer_select = Some(Rc::new(handler));
        self
    }

    pub fn on_palette_select(
        mut self,
        handler: impl Fn(usize, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_palette_select = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for TilesetWidget {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::global(cx);
        let t = &*theme;

        let mut body = div().flex().flex_col().flex_1().min_h(px(0.)).gap_2();

        let mut selectors = div().flex().items_center().gap_2();
        if !self.layers.is_empty() {
            let labels = self.layers.into_iter().enumerate();
            selectors = selectors.child(self::picker(
                "tileset-layer",
                labels,
                self.layer,
                self.on_layer_select,
            ));
        }
        if self.palettes > 0 {
            let labels = (0..self.palettes).map(|i| (i, SharedString::from(i.to_string())));
            selectors = selectors.child(self::picker(
                "tileset-palette",
                labels,
                self.palette,
                self.on_palette_select,
            ));
        }

        match &self.view {
            None => {
                body = body.child(
                    div()
                        .flex()
                        .flex_1()
                        .items_center()
                        .justify_center()
                        .child(empty_state(t, "Layer disabled by the current mode")),
                );
            }
            Some(view) => {
                body = body.child(self::tile_grid(t, view, self.selected, self.on_select));

                let selected = self
                    .selected
                    .and_then(|index| view.tiles.get(index).map(|tile| (index, tile)));
                body = body.child(match selected {
                    Some((index, tile)) => self::detail_row(t, index, tile, view.bpp),
                    None => self::detail_placeholder(t),
                });
            }
        }

        let mut panel = Panel::new(Self::ID, "Tileset")
            .glyph(icon::GRID)
            .grow(true)
            .collapsed(self.collapsed)
            .on_toggle_opt(self.on_toggle)
            .header_action(selectors);
        if let Some(view) = &self.view {
            panel = panel.meta(format!("{}bpp", view.bpp));
        }

        panel.child(body)
    }
}

fn picker(
    id: &'static str,
    labels: impl Iterator<Item = (usize, SharedString)>,
    selected: usize,
    handler: Option<SelectCallback>,
) -> Segmented {
    let mut picker = Segmented::new(id).selected(selected.to_string());
    for (index, label) in labels {
        picker = picker.option_label(index.to_string(), label);
    }

    if let Some(handler) = handler {
        picker = picker.on_select(move |key, window, cx| {
            if let Ok(index) = key.parse() {
                handler(index, window, cx);
            }
        });
    }

    picker
}

fn tile_grid(
    theme: &Theme,
    view: &TilesetView,
    selected: Option<usize>,
    on_select: Option<SelectCallback>,
) -> Stateful<Div> {
    let mut grid = div()
        .id("tileset-grid")
        .flex()
        .flex_col()
        .flex_1()
        .min_h(px(0.))
        .px_1()
        .overflow_y_scroll();

    if view.tiles.is_empty() {
        return grid
            .items_center()
            .justify_center()
            .child(empty_state(theme, "No tiles"));
    }

    for (row_ix, row) in view.tiles.chunks(TILES_PER_ROW).enumerate() {
        let mut cells = div().id(row_ix).flex();

        for (col_ix, tile) in row.iter().enumerate() {
            let index = row_ix * TILES_PER_ROW + col_ix;
            cells = cells.child(self::tile_cell(
                theme,
                index,
                tile,
                selected == Some(index),
                on_select.clone(),
            ));
        }

        grid = grid.child(cells);
    }

    grid
}

fn tile_cell(
    theme: &Theme,
    index: usize,
    tile: &Tile,
    selected: bool,
    on_select: Option<SelectCallback>,
) -> impl IntoElement {
    let size = px((TILE_SIZE * SCALE) as f32);

    let mut cell = div()
        .id(index)
        .border_1()
        .border_color(if selected {
            theme.accent
        } else {
            transparent_black()
        })
        .child(img(tile.image.clone()).w(size).h(size));

    if let Some(handler) = on_select {
        cell = cell
            .cursor_pointer()
            .hover(|s| s.border_color(theme.accent_hover))
            .on_click(move |_, window, cx| {
                cx.stop_propagation();
                handler(index, window, cx);
            });
    }

    cell
}

fn detail_placeholder(theme: &Theme) -> Div {
    div()
        .flex()
        .items_center()
        .px_1()
        .pt(px(6.))
        .child(empty_state(theme, "Select a tile to inspect"))
}

fn detail_row(theme: &Theme, index: usize, tile: &Tile, bpp: u8) -> Div {
    footer_row(
        theme,
        format!("Tile {index} ({})", hex(index as u32, 8)),
        mono_text(theme, format!("{} bytes @ {}", bpp as u32 * 8, tile.addr)),
    )
}
