use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn render_graph(
    theme: &Theme,
    view: Arc<DisassemblyView>,
    id: SharedString,
    rows: row::RowStates,
    layout_cache: Rc<RefCell<DisassemblyLayoutCache>>,
    manual_offset: Point<Pixels>,
    follow: bool,
    viewport: Size<f32>,
    hovered_edge: Option<usize>,
    on_select: Option<SelectCallback>,
    on_context: Option<ContextCallback>,
    on_pan_begin: Option<PanCallback>,
    on_pan_drag: Option<PanCallback>,
    on_viewport: Option<ViewportCallback>,
    on_edge_hover: Option<EdgeHoverCallback>,
    on_jump: Option<JumpCallback>,
) -> impl IntoElement {
    let layout = layout_cache.borrow_mut().graph(&view);
    let measured = viewport.width > 0. && viewport.height > 0.;

    let base = if follow {
        rows.current
            .and_then(|cur| {
                let b = layout
                    .blocks
                    .iter()
                    .find(|b| (b.start..b.end).contains(&cur))?;
                Some((b, cur - b.start))
            })
            .map(|(b, li)| {
                let ox = viewport.width / 2. - (b.x + b.w / 2.);
                let oy_block = GRAPH_TOP_PAD - b.y;

                let line_y = b.y
                    + graph::BLOCK_HEADER_H
                    + li as f32 * graph::BLOCK_LINE_H
                    + graph::BLOCK_LINE_H / 2.;
                let line_screen = line_y + oy_block;
                let bottom = viewport.height - GRAPH_BOTTOM_PAD;

                let oy = if line_screen > bottom {
                    GRAPH_TOP_PAD - line_y
                } else {
                    oy_block
                };

                point(px(ox), px(oy))
            })
            .unwrap_or_default()
    } else {
        Point::default()
    };

    let offset = base + manual_offset;

    let origin = Rc::new(Cell::new(point(px(0.), px(0.))));

    let paint_data = layout.clone();
    let origin_paint = origin.clone();
    let canvas_el = canvas(
        move |bounds, window, cx| {
            origin_paint.set(bounds.origin);
            if let Some(cb) = &on_viewport {
                cb(
                    size(f32::from(bounds.size.width), f32::from(bounds.size.height)),
                    window,
                    cx,
                );
            }
        },
        move |bounds, _, window, cx| {
            if !measured {
                return;
            }

            let theme_now = Theme::global(cx);
            graph_paint::paint_edges(
                window,
                bounds,
                offset,
                &paint_data.edges,
                hovered_edge,
                &theme_now,
            );
        },
    )
    .absolute()
    .size_full();

    let mut container = div()
        .id(SharedString::from(format!("{id}-graph")))
        .relative()
        .flex_1()
        .min_h(px(MIN_LIST_HEIGHT))
        .overflow_hidden()
        .when(hovered_edge.is_some(), |d| d.cursor_pointer())
        .child(canvas_el);

    if let Some(begin) = on_pan_begin {
        container = container.on_mouse_down(MouseButton::Left, move |e, window, cx| {
            begin(e.position, window, cx)
        });
    }

    let move_layout = layout.clone();
    let move_origin = origin.clone();
    container = container.on_mouse_move(move |e: &MouseMoveEvent, window, cx| {
        if e.dragging() {
            if let Some(drag) = &on_pan_drag {
                drag(e.position, window, cx);
            }
            return;
        }
        if let Some(hover) = &on_edge_hover {
            let hit =
                graph_paint::edge_at(&move_layout.edges, e.position, move_origin.get(), offset);
            hover(hit, window, cx);
        }
    });

    if let Some(jump) = on_jump {
        let jump_layout = layout.clone();
        let jump_origin = origin.clone();
        container = container.on_click(move |e, window, cx| {
            let Some(ei) =
                graph_paint::edge_at(&jump_layout.edges, e.position(), jump_origin.get(), offset)
            else {
                return;
            };
            let Some(block) = jump_layout.blocks.get(jump_layout.edges[ei].to) else {
                return;
            };
            let to = point(
                px(viewport.width / 2. - (block.x + block.w / 2.)),
                px(viewport.height / 2. - (block.y + block.h / 2.)),
            );
            jump(offset, to, window, cx);
        });
    }

    const CULL_MARGIN: f32 = 64.;

    let blocks = if measured {
        layout.blocks.as_slice()
    } else {
        &[]
    };
    for block in blocks {
        let bx = f32::from(offset.x) + block.x;
        let by = f32::from(offset.y) + block.y;
        if bx + block.w < -CULL_MARGIN
            || by + block.h < -CULL_MARGIN
            || bx > viewport.width + CULL_MARGIN
            || by > viewport.height + CULL_MARGIN
        {
            continue;
        }

        let lines = &view.lines[block.start..block.end];
        let has_current = rows
            .current
            .is_some_and(|cur| (block.start..block.end).contains(&cur));
        let border = if has_current {
            theme.accent
        } else {
            theme.border_strong
        };

        let mut card = div()
            .absolute()
            .left(offset.x + px(block.x))
            .top(offset.y + px(block.y))
            .w(px(block.w))
            .h(px(block.h))
            .flex()
            .flex_col()
            .bg(theme.bg_panel)
            .border_1()
            .border_color(border)
            .rounded_md()
            .overflow_hidden()
            .child(
                div()
                    .flex()
                    .items_center()
                    .px_2()
                    .h(px(graph::BLOCK_HEADER_H))
                    .flex_none()
                    .bg(theme.bg_header)
                    .border_b_1()
                    .border_color(theme.border_subtle)
                    .font_family(theme.mono_font)
                    .text_xs()
                    .text_color(theme.code_address)
                    .child(SharedString::from(view.lines[block.start].address.clone())),
            );

        let mut body = div().flex().flex_col().flex_1().justify_center();
        for (k, line) in lines.iter().enumerate() {
            let ix = block.start + k;
            let breakpoint = rows.breakpoint(ix);
            let note = SharedString::from(rows.note(ix, line));

            let mut gutter = div()
                .w(px(graph::BLOCK_GUTTER_W))
                .flex_none()
                .flex()
                .items_center()
                .justify_center();
            if let Some(kind) = breakpoint {
                gutter = gutter.child(
                    div()
                        .size(px(7.))
                        .rounded_full()
                        .bg(breakpoint_color(theme, kind)),
                );
            } else if let Some(color) = rows.marker_color(theme, ix) {
                gutter = gutter.child(
                    svg()
                        .path(icon::CHEVRON_RIGHT)
                        .size(px(10.))
                        .text_color(color),
                );
            }

            let mut row = div()
                .id(("graph-line", ix))
                .flex()
                .items_center()
                .pr_3()
                .h(px(graph::BLOCK_LINE_H))
                .flex_none()
                .when_some(rows.background(theme, ix), |d, bg| d.bg(bg))
                .child(gutter)
                .child(row::instruction_text(
                    theme,
                    &line.tokens,
                    layout.mnemonic_w,
                ));

            if !note.is_empty() {
                row = row.child(
                    div()
                        .flex_1()
                        .min_w(px(0.))
                        .pl_4()
                        .font_family(theme.mono_font)
                        .text_xs()
                        .text_color(theme.code_comment)
                        .text_right()
                        .whitespace_nowrap()
                        .child(note),
                );
            }

            if let Some(select) = on_select.clone() {
                row = row
                    .cursor_pointer()
                    .on_click(move |_, window, cx| select(ix, window, cx));
            }
            if let Some(context) = on_context.clone() {
                row = row.on_mouse_down(MouseButton::Right, move |e, window, cx| {
                    context(ix, e.position, window, cx)
                });
            }

            body = body.child(row);
        }
        card = card.child(body);

        container = container.child(card);
    }

    container
}
