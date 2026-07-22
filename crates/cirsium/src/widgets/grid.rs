use std::cell::Cell;
use std::rc::Rc;
use std::sync::Arc;

use gpui::prelude::*;
use gpui::{
    App, Bounds, ClickEvent, Div, Hsla, ImageFiltering, MouseMoveEvent, Pixels, Point, RenderImage,
    SharedString, Size, Window, canvas, div, fill, img, point, px, size,
};

use crate::components::{
    ClickHandler, FOOTER_ROW_H, Panel, Segmented, empty_state, footer_placeholder,
};
use crate::theme::{Theme, with_alpha};

mod image_grid;

pub(super) type SelectCallback = Rc<dyn Fn(usize, &mut Window, &mut App) + 'static>;
pub(super) type HoverCallback = Rc<dyn Fn(Option<usize>, &mut Window, &mut App) + 'static>;
pub(super) type ViewportCallback = Rc<dyn Fn(Size<f32>, &mut Window, &mut App) + 'static>;
type OverlayToggleCallback = Rc<dyn Fn(&mut Window, &mut App) + 'static>;

type DetailFn = Box<dyn Fn(&Theme, usize) -> Div + 'static>;

#[derive(Clone)]
pub struct TilesetLayer {
    pub label: SharedString,
    pub enabled: bool,
}

impl From<debugger_model::TilesetLayer> for TilesetLayer {
    fn from(layer: debugger_model::TilesetLayer) -> Self {
        Self {
            label: SharedString::from(layer.name),
            enabled: layer.enabled,
        }
    }
}

pub(super) fn build_atlas<'a>(
    cols: usize,
    rows: usize,
    cell_px: u32,
    cells: impl Iterator<Item = (&'a [(u8, u8, u8)], u32, u32)>,
) -> Arc<RenderImage> {
    let mut atlas = image::RgbaImage::new(cols as u32 * cell_px, rows as u32 * cell_px);

    let mut row = vec![0u8; (cell_px * 4) as usize];
    for (i, (pixels, width, height)) in cells.enumerate() {
        let x = (i % cols) as u32 * cell_px + (cell_px - width) / 2;
        let y = (i / cols) as u32 * cell_px + (cell_px - height) / 2;
        self::blit(&mut atlas, &mut row, x, y, pixels, width, height);
    }

    Arc::new(RenderImage::new(vec![image::Frame::new(atlas)]))
}

pub(super) fn build_image(pixels: &[(u8, u8, u8)], width: u32, height: u32) -> Arc<RenderImage> {
    let mut image = image::RgbaImage::new(width, height);
    let mut row = vec![0u8; (width * 4) as usize];
    self::blit(&mut image, &mut row, 0, 0, pixels, width, height);
    Arc::new(RenderImage::new(vec![image::Frame::new(image)]))
}

pub(in crate::widgets) fn bgra((r, g, b): (u8, u8, u8)) -> [u8; 4] {
    [b, g, r, 0xFF]
}

fn blit(
    atlas: &mut image::RgbaImage,
    row: &mut [u8],
    x0: u32,
    y0: u32,
    pixels: &[(u8, u8, u8)],
    width: u32,
    height: u32,
) {
    let atlas_width = atlas.width() as usize;
    let data: &mut [u8] = atlas;
    let (x0, y0) = (x0 as usize, y0 as usize);
    let (width, height) = (width as usize, height as usize);
    let row = &mut row[..width * 4];

    for sy in 0..height {
        for sx in 0..width {
            let px = self::bgra(pixels[sy * width + sx]);
            row[sx * 4..sx * 4 + 4].copy_from_slice(&px);
        }

        let o = ((y0 + sy) * atlas_width + x0) * 4;
        data[o..o + row.len()].copy_from_slice(row);
    }
}

#[derive(Clone, Copy)]
pub struct GridOverlay {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

#[derive(Clone)]
pub struct GridWindowBand {
    pub rect: GridOverlay,
    pub active: bool,
    pub show_edges: bool,
}

#[derive(Clone)]
pub struct GridWindowOverlay {
    pub bands: Vec<GridWindowBand>,
    pub effective: Vec<GridOverlay>,
    pub label_anchor: Option<GridOverlay>,
    pub label: SharedString,
    pub slot: usize,
}

impl From<debugger_model::TilemapViewport> for GridOverlay {
    fn from(viewport: debugger_model::TilemapViewport) -> Self {
        Self {
            x: viewport.x,
            y: viewport.y,
            width: viewport.width,
            height: viewport.height,
        }
    }
}

impl GridOverlay {
    fn spans(start: u32, len: u32, max: u32) -> impl Iterator<Item = (u32, u32)> {
        let start = start % max;
        let len = len.min(max);
        let first = (start, len.min(max - start));
        let wrapped = (len > max - start).then(|| (0, len - (max - start)));
        std::iter::once(first).chain(wrapped)
    }

    fn rects(self, map_w: u32, map_h: u32, scale: f32) -> Vec<Bounds<Pixels>> {
        let mut rects = Vec::with_capacity(4);
        for (y, h) in Self::spans(self.y, self.height, map_h) {
            for (x, w) in Self::spans(self.x, self.width, map_w) {
                rects.push(Bounds {
                    origin: point(px(x as f32 * scale), px(y as f32 * scale)),
                    size: size(px(w as f32 * scale), px(h as f32 * scale)),
                });
            }
        }
        rects
    }
}

pub struct GridContent {
    pub image: Option<Arc<RenderImage>>,
    pub cols: usize,
    pub rows: usize,
    pub tile_px: u32,
    pub count: usize,
    pub empty_message: &'static str,
    pub overlay: Option<GridOverlay>,
    pub window_overlays: Vec<GridWindowOverlay>,
}

#[derive(IntoElement)]
pub struct GridPanel {
    id: &'static str,
    title: &'static str,
    glyph: &'static str,
    meta: Option<SharedString>,
    content: Option<GridContent>,
    detail: Option<DetailFn>,
    detail_rows: usize,
    placeholder: &'static str,
    layers: Vec<TilesetLayer>,
    layer: usize,
    selected: Option<usize>,
    hovered: Option<usize>,
    viewport: Size<f32>,
    collapsed: bool,
    on_toggle: Option<ClickHandler>,
    on_select: Option<SelectCallback>,
    on_hover: Option<HoverCallback>,
    on_layer_select: Option<SelectCallback>,
    on_viewport: Option<ViewportCallback>,
    window_overlay_toggle: bool,
    show_window_overlays: bool,
    on_toggle_window_overlays: Option<OverlayToggleCallback>,
}

impl GridPanel {
    pub fn new(
        id: &'static str,
        title: &'static str,
        glyph: &'static str,
        content: Option<GridContent>,
    ) -> Self {
        Self {
            id,
            title,
            glyph,
            meta: None,
            content,
            detail: None,
            detail_rows: 1,
            placeholder: "",
            layers: Vec::new(),
            layer: 0,
            selected: None,
            hovered: None,
            viewport: size(0., 0.),
            collapsed: false,
            on_toggle: None,
            on_select: None,
            on_hover: None,
            on_layer_select: None,
            on_viewport: None,
            window_overlay_toggle: false,
            show_window_overlays: true,
            on_toggle_window_overlays: None,
        }
    }

    pub fn meta(mut self, meta: impl Into<SharedString>) -> Self {
        self.meta = Some(meta.into());
        self
    }

    pub fn detail(mut self, detail: impl Fn(&Theme, usize) -> Div + 'static) -> Self {
        self.detail = Some(Box::new(detail));
        self
    }

    pub fn detail_rows(mut self, rows: usize) -> Self {
        self.detail_rows = rows;
        self
    }

    pub fn placeholder(mut self, placeholder: &'static str) -> Self {
        self.placeholder = placeholder;
        self
    }

    pub fn layers(mut self, layers: Vec<TilesetLayer>) -> Self {
        self.layers = layers;
        self
    }

    pub fn layer(mut self, layer: usize) -> Self {
        self.layer = layer;
        self
    }

    pub fn selected(mut self, selected: Option<usize>) -> Self {
        self.selected = selected;
        self
    }

    pub fn hovered(mut self, hovered: Option<usize>) -> Self {
        self.hovered = hovered;
        self
    }

    pub fn viewport(mut self, viewport: Size<f32>) -> Self {
        self.viewport = viewport;
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

    pub fn on_hover(
        mut self,
        handler: impl Fn(Option<usize>, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_hover = Some(Rc::new(handler));
        self
    }

    pub fn on_layer_select(
        mut self,
        handler: impl Fn(usize, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_layer_select = Some(Rc::new(handler));
        self
    }

    pub fn on_viewport(
        mut self,
        handler: impl Fn(Size<f32>, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_viewport = Some(Rc::new(handler));
        self
    }

    pub fn window_overlays_visible(mut self, visible: bool) -> Self {
        self.show_window_overlays = visible;
        self
    }

    pub fn window_overlay_toggle(mut self, enabled: bool) -> Self {
        self.window_overlay_toggle = enabled;
        self
    }

    pub fn on_toggle_window_overlays(
        mut self,
        handler: impl Fn(&mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_toggle_window_overlays = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for GridPanel {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::global(cx);
        let t = &*theme;

        let mut body = div().flex().flex_col().flex_1().min_h(px(0.)).gap_2();

        let mut selectors = div().flex().items_center().gap_2();
        if !self.layers.is_empty() {
            selectors = selectors.child(self::picker(
                format!("{}-layer", self.id),
                &self.layers,
                self.layer,
                self.on_layer_select,
            ));
        }
        if self.window_overlay_toggle {
            let mut toggle = Segmented::new(format!("{}-windows", self.id))
                .option_label("windows", "W1/W2")
                .selected(if self.show_window_overlays {
                    "windows"
                } else {
                    ""
                });
            if let Some(handler) = self.on_toggle_window_overlays.clone() {
                toggle = toggle.on_select(move |_, window, cx| handler(window, cx));
            }
            selectors = selectors.child(toggle);
        }

        match &self.content {
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
            Some(content) => {
                body = body.child(image_grid::cell_grid(
                    t,
                    self.id,
                    content,
                    self.selected,
                    self.hovered,
                    self.viewport,
                    self.on_select,
                    self.on_hover,
                    self.on_viewport,
                    self.show_window_overlays,
                ));

                let selected = self.selected.filter(|&index| index < content.count);
                let slot = div()
                    .flex_none()
                    .flex()
                    .flex_col()
                    .h(px(FOOTER_ROW_H * self.detail_rows as f32));
                body = body.child(match (selected, &self.detail) {
                    (Some(index), Some(detail)) => slot.child(detail(t, index)),
                    _ => slot.child(footer_placeholder(t, self.placeholder)),
                });
            }
        }

        let mut panel = Panel::new(self.id, self.title)
            .glyph(self.glyph)
            .grow(true)
            .collapsed(self.collapsed)
            .on_toggle_opt(self.on_toggle)
            .header_action(selectors);
        if let Some(meta) = self.meta {
            panel = panel.meta(meta);
        }

        panel.child(body)
    }
}

fn picker(
    id: impl Into<SharedString>,
    layers: &[TilesetLayer],
    selected: usize,
    handler: Option<SelectCallback>,
) -> Segmented {
    let mut picker = Segmented::new(id).selected(selected.to_string());
    for (index, layer) in layers.iter().enumerate() {
        picker = if layer.enabled {
            picker.option_label(index.to_string(), layer.label.clone())
        } else {
            picker.option_disabled(index.to_string(), layer.label.clone())
        };
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

fn measure(on_viewport: Option<ViewportCallback>) -> impl IntoElement {
    canvas(
        move |bounds, window, cx| {
            if let Some(cb) = &on_viewport {
                cb(
                    size(f32::from(bounds.size.width), f32::from(bounds.size.height)),
                    window,
                    cx,
                );
            }
        },
        |_, _, _, _| {},
    )
    .absolute()
    .size_full()
}
