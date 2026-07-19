use super::*;

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
