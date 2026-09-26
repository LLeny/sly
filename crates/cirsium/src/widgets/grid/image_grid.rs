use super::*;

fn cell_size(viewport: Size<f32>, cols: usize, rows: usize, tile_px: u32) -> f32 {
    if viewport.width < 1. || viewport.height < 1. {
        return (tile_px * 2) as f32;
    }

    let fit = (viewport.width / cols as f32).min(viewport.height / rows as f32);
    if tile_px == 1 {
        fit.max(0.1)
    } else {
        fit.floor().max(4.)
    }
}

#[expect(clippy::too_many_arguments)]
pub(super) fn cell_grid(
    theme: &Theme,
    id: &'static str,
    content: &GridContent,
    selected: Option<usize>,
    hovered: Option<usize>,
    viewport: Size<f32>,
    on_select: Option<SelectCallback>,
    on_hover: Option<HoverCallback>,
    on_viewport: Option<ViewportCallback>,
    show_window_overlays: bool,
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
    container.child(render(
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
            footprint: content.footprint,
            window_overlays: if show_window_overlays {
                content.window_overlays.clone()
            } else {
                Vec::new()
            },
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
    footprint: Option<debugger_model::TilemapViewport>,
    window_overlays: Vec<GridWindowOverlay>,
}

fn render(
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
        footprint,
        window_overlays,
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
        .child(
            img(image)
                .size_full()
                .image_filtering(ImageFiltering::Nearest),
        )
        .child(track_bounds);

    if let Some(overlay) = overlay {
        let (map_w, map_h) = (cols as u32 * tile_px, rows as u32 * tile_px);
        for bounds in overlay.rects(map_w, map_h, cell / tile_px as f32) {
            grid = grid.child(
                outline(bounds, with_alpha(theme.accent, 0.7)).bg(with_alpha(theme.accent, 0.12)),
            );
        }
    }

    if let Some(viewport) = footprint {
        let color = theme.accent;
        let scale = cell / tile_px as f32;
        grid = grid.child(
            canvas(
                |_, _, _| {},
                move |bounds, _, window, _| {
                    let [a, b, tx, c, d, ty] =
                        viewport.transform.unwrap_or([1., 0., 0., 0., 1., 0.]);
                    let (x, y, w, h) = (
                        viewport.x as f32,
                        viewport.y as f32,
                        viewport.width as f32,
                        viewport.height as f32,
                    );

                    let mut path = gpui::PathBuilder::stroke(px(1.));
                    for (i, (x, y)) in [(x, y), (x + w, y), (x + w, y + h), (x, y + h), (x, y)]
                        .into_iter()
                        .enumerate()
                    {
                        let p = point(
                            bounds.origin.x + px((a * x + b * y + tx) * scale),
                            bounds.origin.y + px((c * x + d * y + ty) * scale),
                        );

                        if i == 0 {
                            path.move_to(p);
                        } else {
                            path.line_to(p);
                        }
                    }

                    if let Ok(path) = path.build() {
                        window.paint_path(path, color);
                    }
                },
            )
            .absolute()
            .size_full(),
        );
    }

    let (map_w, map_h) = (cols as u32 * tile_px, rows as u32 * tile_px);
    let scale = cell / tile_px as f32;
    let window_overlays: Arc<[GridWindowOverlay]> = window_overlays.into();
    if !window_overlays.is_empty() {
        let painted = window_overlays.clone();
        let colors = [theme.warning, theme.info];
        let inactive = theme.text_dim;
        grid = grid.child(
            canvas(
                |_, _, _| {},
                move |bounds, _, window, _| {
                    self::paint_window_overlays(
                        window, bounds, &painted, map_w, map_h, scale, colors, inactive,
                    );
                },
            )
            .absolute()
            .inset_0(),
        );

        for overlay in window_overlays.iter() {
            let Some(anchor) = overlay.label_anchor else {
                continue;
            };
            let Some(bounds) = anchor
                .rects(map_w, map_h, scale)
                .into_iter()
                .max_by_key(|bounds| u32::from(bounds.size.width))
            else {
                continue;
            };
            let color = colors[overlay.slot.min(1)];
            let label_width = px((f32::from(bounds.size.width) - 4.).max(0.));
            grid = grid.child(
                div()
                    .absolute()
                    .left(bounds.origin.x + px(2.))
                    .top(bounds.origin.y + px(2.))
                    .flex()
                    .items_start()
                    .w(label_width)
                    .h(px(15.))
                    .overflow_hidden()
                    .child(
                        div()
                            .flex_none()
                            .px_1()
                            .rounded_sm()
                            .bg(with_alpha(theme.bg_root, 0.82))
                            .text_xs()
                            .whitespace_nowrap()
                            .text_color(color)
                            .child(overlay.label.clone()),
                    ),
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

#[expect(clippy::too_many_arguments)]
fn paint_window_overlays(
    window: &mut Window,
    canvas_bounds: Bounds<Pixels>,
    overlays: &[GridWindowOverlay],
    map_w: u32,
    map_h: u32,
    scale: f32,
    colors: [Hsla; 2],
    inactive: Hsla,
) {
    let offset = |bounds: Bounds<Pixels>| Bounds {
        origin: point(
            canvas_bounds.origin.x + bounds.origin.x,
            canvas_bounds.origin.y + bounds.origin.y,
        ),
        size: bounds.size,
    };

    for overlay in overlays {
        let color = colors[overlay.slot.min(1)];
        for rect in &overlay.effective {
            for bounds in rect.rects(map_w, map_h, scale) {
                window.paint_quad(fill(offset(bounds), with_alpha(color, 0.14)));
            }
        }
    }

    for overlay in overlays {
        for band in &overlay.bands {
            if !band.show_edges || band.rect.width == 0 || band.rect.height == 0 {
                continue;
            }

            let color = if band.active {
                colors[overlay.slot.min(1)]
            } else {
                inactive
            };
            let left = band.rect.x % map_w;
            let right = band.rect.x.saturating_add(band.rect.width) % map_w;
            for (y, height) in GridOverlay::spans(band.rect.y, band.rect.height, map_h) {
                let edge = |x: u32| {
                    offset(Bounds {
                        origin: point(px(x as f32 * scale), px(y as f32 * scale)),
                        size: size(px(1.), px(height as f32 * scale)),
                    })
                };
                window.paint_quad(fill(edge(left), with_alpha(color, 0.9)));
                if right != left {
                    window.paint_quad(fill(edge(right), with_alpha(color, 0.9)));
                }
            }
        }
    }
}
