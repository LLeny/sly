use super::*;

pub(super) fn render_linear(
    view: Arc<DisassemblyView>,
    scroll: UniformListScrollHandle,
    id: SharedString,
    rows: row::RowStates,
    layout_cache: Rc<RefCell<DisassemblyLayoutCache>>,
    on_select: Option<SelectCallback>,
    on_context: Option<ContextCallback>,
) -> impl IntoElement {
    let list_id = SharedString::from(format!("{id}-list"));
    let (gutters, mnemonic_width) = {
        let mut cache = layout_cache.borrow_mut();
        let gutters = cache.gutters(&view, rows.selected);
        (gutters, cache.mnemonic_width(&view).max(LINEAR_MNEMONIC_W))
    };
    let count = view.lines.len();
    let empty_gutter = GutterCell::default();
    uniform_list(list_id, count, move |range, _window, cx| {
        let theme = Theme::global(cx);
        let t = &*theme;
        let (cells, arrows_width) = &*gutters;
        let row_options = row::DisasmRowOptions {
            arrows_width: *arrows_width,
            show_bytes: view.show_bytes,
            mnemonic_width,
            rows: rows.clone(),
            on_select: on_select.clone(),
            on_context: on_context.clone(),
        };
        range
            .map(|ix| {
                row::disasm_row(
                    t,
                    ix,
                    &view.lines[ix],
                    cells.get(ix).unwrap_or(&empty_gutter),
                    &row_options,
                )
            })
            .collect::<Vec<_>>()
    })
    .track_scroll(&scroll)
    .flex_1()
    .min_h(px(MIN_LIST_HEIGHT))
}
