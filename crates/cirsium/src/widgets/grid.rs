use std::cell::Cell;
use std::rc::Rc;
use std::sync::Arc;

use gpui::prelude::*;
use gpui::{
    App, Bounds, ClickEvent, Div, Hsla, MouseMoveEvent, Pixels, Point, RenderImage, SharedString,
    Size, Window, canvas, div, img, point, px, size,
};

use crate::components::{
    ClickHandler, FOOTER_ROW_H, Panel, Segmented, empty_state, footer_placeholder,
};
use crate::theme::{Theme, with_alpha};

pub(super) type SelectCallback = Rc<dyn Fn(usize, &mut Window, &mut App) + 'static>;
pub(super) type HoverCallback = Rc<dyn Fn(Option<usize>, &mut Window, &mut App) + 'static>;
pub(super) type ViewportCallback = Rc<dyn Fn(Size<f32>, &mut Window, &mut App) + 'static>;

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

const BLEED_BORDER: u32 = 1;

pub(super) fn build_atlas<'a>(
    cols: usize,
    rows: usize,
    cell_px: u32,
    scale: u32,
    cells: impl Iterator<Item = (&'a [(u8, u8, u8)], u32, u32)>,
) -> Arc<RenderImage> {
    let mut atlas = self::bordered(cols as u32 * cell_px * scale, rows as u32 * cell_px * scale);

    let mut row = vec![0u8; (cell_px * scale * 4) as usize];
    for (i, (pixels, width, height)) in cells.enumerate() {
        let x = (i % cols) as u32 * cell_px * scale + (cell_px - width) / 2 * scale + BLEED_BORDER;
        let y = (i / cols) as u32 * cell_px * scale + (cell_px - height) / 2 * scale + BLEED_BORDER;
        self::blit(&mut atlas, &mut row, x, y, pixels, width, height, scale);
    }

    self::finish(atlas)
}

/// A single image scaled up by an integer factor (e.g. the framebuffer).
pub(super) fn build_image(
    pixels: &[(u8, u8, u8)],
    width: u32,
    height: u32,
    scale: u32,
) -> Arc<RenderImage> {
    let mut image = self::bordered(width * scale, height * scale);
    let mut row = vec![0u8; (width * scale * 4) as usize];
    self::blit(
        &mut image,
        &mut row,
        BLEED_BORDER,
        BLEED_BORDER,
        pixels,
        width,
        height,
        scale,
    );
    self::finish(image)
}

fn bordered(width: u32, height: u32) -> image::RgbaImage {
    image::RgbaImage::new(width + BLEED_BORDER * 2, height + BLEED_BORDER * 2)
}

fn finish(mut image: image::RgbaImage) -> Arc<RenderImage> {
    self::replicate_border(&mut image);
    Arc::new(RenderImage::new(vec![image::Frame::new(image)]))
}

fn replicate_border(image: &mut image::RgbaImage) {
    let (w, h) = (image.width() as usize, image.height() as usize);
    let stride = w * 4;
    let data: &mut [u8] = image;

    for y in 1..h - 1 {
        let row = &mut data[y * stride..(y + 1) * stride];
        row.copy_within(4..8, 0);
        row.copy_within(stride - 8..stride - 4, stride - 4);
    }

    data.copy_within(stride..stride * 2, 0);
    data.copy_within((h - 2) * stride..(h - 1) * stride, (h - 1) * stride);
}

fn blit(
    atlas: &mut image::RgbaImage,
    row: &mut [u8],
    x0: u32,
    y0: u32,
    pixels: &[(u8, u8, u8)],
    width: u32,
    height: u32,
    scale: u32,
) {
    let atlas_width = atlas.width() as usize;
    let data: &mut [u8] = atlas;
    let (x0, y0) = (x0 as usize, y0 as usize);
    let (width, height, scale) = (width as usize, height as usize, scale as usize);
    let row = &mut row[..width * scale * 4];

    for sy in 0..height {
        for sx in 0..width {
            let (r, g, b) = pixels[sy * width + sx];
            for dx in 0..scale {
                let o = (sx * scale + dx) * 4;
                row[o..o + 4].copy_from_slice(&[b, g, r, 0xFF]);
            }
        }

        for dy in 0..scale {
            let o = ((y0 + sy * scale + dy) * atlas_width + x0) * 4;
            data[o..o + row.len()].copy_from_slice(row);
        }
    }
}

#[derive(Clone, Copy)]
pub struct GridOverlay {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
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
                body = body.child(self::cell_grid(
                    t,
                    self.id,
                    content,
                    self.selected,
                    self.hovered,
                    self.viewport,
                    self.on_select,
                    self.on_hover,
                    self.on_viewport,
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

fn cell_size(viewport: Size<f32>, cols: usize, rows: usize, tile_px: u32) -> f32 {
    if viewport.width < 1. || viewport.height < 1. {
        return (tile_px * 2) as f32;
    }

    (viewport.width / cols as f32)
        .min(viewport.height / rows as f32)
        .floor()
        .max(4.)
}

#[expect(clippy::too_many_arguments)]
fn cell_grid(
    theme: &Theme,
    id: &'static str,
    content: &GridContent,
    selected: Option<usize>,
    hovered: Option<usize>,
    viewport: Size<f32>,
    on_select: Option<SelectCallback>,
    on_hover: Option<HoverCallback>,
    on_viewport: Option<ViewportCallback>,
) -> Div {
    let container = div()
        .relative()
        .flex()
        .flex_col()
        .flex_1()
        .min_h(px(0.))
        .px_1()
        .items_center()
        .justify_center()
        .overflow_hidden()
        .child(self::measure(on_viewport));

    let Some(image) = content.image.clone() else {
        return container.child(empty_state(theme, content.empty_message));
    };

    let cell = self::cell_size(viewport, content.cols, content.rows, content.tile_px);
    container.child(self::image_grid(
        theme,
        GridSpec {
            id: format!("{id}-grid").into(),
            image,
            cols: content.cols,
            rows: content.rows,
            tile_px: content.tile_px,
            count: content.count,
            cell,
            overlay: content.overlay,
        },
        selected,
        hovered,
        on_select,
        on_hover,
    ))
}

struct GridSpec {
    id: SharedString,
    image: Arc<RenderImage>,
    cols: usize,
    rows: usize,
    tile_px: u32,
    count: usize,
    cell: f32,
    overlay: Option<GridOverlay>,
}

fn image_grid(
    theme: &Theme,
    spec: GridSpec,
    selected: Option<usize>,
    hovered: Option<usize>,
    on_select: Option<SelectCallback>,
    on_hover: Option<HoverCallback>,
) -> impl IntoElement {
    let GridSpec {
        id,
        image,
        cols,
        rows,
        tile_px,
        count,
        cell,
        overlay,
    } = spec;

    let bounds = Rc::new(Cell::new(Bounds::<Pixels>::default()));
    let hit = {
        let bounds = bounds.clone();
        move |position: Point<Pixels>| -> Option<usize> {
            let bounds = bounds.get();
            if !bounds.contains(&position) {
                return None;
            }

            let col = (f32::from(position.x - bounds.origin.x) / cell) as usize;
            let row = (f32::from(position.y - bounds.origin.y) / cell) as usize;
            let index = row.min(rows - 1) * cols + col.min(cols - 1);
            (index < count).then_some(index)
        }
    };

    let track_bounds = canvas(
        {
            let bounds = bounds.clone();
            move |grid_bounds, _, _| bounds.set(grid_bounds)
        },
        |_, _, _, _| {},
    )
    .absolute()
    .inset_0();

    let outline = |bounds: Bounds<Pixels>, color: Hsla| {
        div()
            .absolute()
            .left(bounds.origin.x)
            .top(bounds.origin.y)
            .w(bounds.size.width)
            .h(bounds.size.height)
            .border_1()
            .border_color(color)
    };

    let ring = |index: usize, color: Hsla| {
        outline(
            Bounds {
                origin: point(
                    px((index % cols) as f32 * cell),
                    px((index / cols) as f32 * cell),
                ),
                size: size(px(cell), px(cell)),
            },
            color,
        )
    };

    let mut grid = div()
        .id(id)
        .relative()
        .flex_none()
        .w(px(cell * cols as f32))
        .h(px(cell * rows as f32))
        .child(img(image).size_full())
        .child(track_bounds);

    if let Some(overlay) = overlay {
        let (map_w, map_h) = (cols as u32 * tile_px, rows as u32 * tile_px);
        for bounds in overlay.rects(map_w, map_h, cell / tile_px as f32) {
            grid = grid.child(
                outline(bounds, with_alpha(theme.accent, 0.7)).bg(with_alpha(theme.accent, 0.12)),
            );
        }
    }

    if let Some(index) = hovered.filter(|&i| i < count && selected != Some(i)) {
        grid = grid.child(ring(index, theme.accent_hover));
    }
    if let Some(index) = selected.filter(|&i| i < count) {
        grid = grid.child(ring(index, theme.accent));
    }

    if let Some(handler) = on_select {
        let hit = hit.clone();
        grid = grid
            .cursor_pointer()
            .on_click(move |event: &ClickEvent, window, cx| {
                cx.stop_propagation();
                if let Some(index) = hit(event.position()) {
                    handler(index, window, cx);
                }
            });
    }

    if let Some(handler) = on_hover {
        let moved = handler.clone();
        grid = grid
            .on_mouse_move(move |event: &MouseMoveEvent, window, cx| {
                moved(hit(event.position), window, cx);
            })
            .on_hover(move |hovered, window, cx| {
                if !hovered {
                    handler(None, window, cx);
                }
            });
    }

    grid
}
