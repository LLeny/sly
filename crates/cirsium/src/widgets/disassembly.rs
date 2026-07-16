use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

use gpui::prelude::*;
use gpui::{
    App, Bounds, ClickEvent, Div, Hsla, MouseButton, MouseMoveEvent, PathBuilder, Pixels, Point,
    SharedString, Size, Stateful, UniformListScrollHandle, Window, canvas, div, point, px, size,
    svg, uniform_list,
};

use super::breakpoints::{BreakpointKind, breakpoint_color};
use crate::assets::icon;
use crate::components::{ClickHandler, Panel, Segmented};
use crate::theme::Theme;

mod arrows;
mod graph;
use arrows::{GutterCell, arrow_gutters, paint_arrows};
use graph::{EdgeKind, GraphEdge, GraphLayout};

const ROW_HEIGHT: f32 = 20.;
const MIN_LIST_HEIGHT: f32 = 120.;
const BYTES_WIDTH: f32 = 96.;
const GRAPH_TOP_PAD: f32 = 16.;
const GRAPH_BOTTOM_PAD: f32 = 28.;
const LINEAR_MNEMONIC_W: f32 = 42.;
const GRAPH_MNEMONIC_W: f32 = 36.;

type SelectCallback = Rc<dyn Fn(usize, &mut Window, &mut App) + 'static>;
type ContextCallback = Rc<dyn Fn(usize, Point<Pixels>, &mut Window, &mut App) + 'static>;
type ViewCallback = Rc<dyn Fn(ViewMode, &mut Window, &mut App) + 'static>;
type PanCallback = Rc<dyn Fn(Point<Pixels>, &mut Window, &mut App) + 'static>;
type ViewportCallback = Rc<dyn Fn(Size<f32>, &mut Window, &mut App) + 'static>;
type EdgeHoverCallback = Rc<dyn Fn(Option<usize>, &mut Window, &mut App) + 'static>;
type JumpCallback = Rc<dyn Fn(Point<Pixels>, Point<Pixels>, &mut Window, &mut App) + 'static>;

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum ViewMode {
    #[default]
    Linear,
    Graph,
}

impl ViewMode {
    fn key(self) -> &'static str {
        match self {
            ViewMode::Linear => "linear",
            ViewMode::Graph => "graph",
        }
    }

    fn from_key(key: &str) -> Self {
        match key {
            "graph" => ViewMode::Graph,
            _ => ViewMode::Linear,
        }
    }
}

pub use debugger_model::{FlowKind, TokenKind};

#[derive(Clone)]
pub struct DisasmToken {
    pub text: SharedString,
    pub kind: TokenKind,
}

impl DisasmToken {
    pub fn new(text: impl Into<SharedString>, kind: TokenKind) -> Self {
        Self {
            text: text.into(),
            kind,
        }
    }
}

impl From<debugger_model::DisasmToken> for DisasmToken {
    fn from(token: debugger_model::DisasmToken) -> Self {
        Self {
            text: SharedString::from(token.text.as_ref()),
            kind: token.kind,
        }
    }
}

fn token_color(theme: &Theme, kind: TokenKind) -> Hsla {
    match kind {
        TokenKind::Mnemonic => theme.code_opcode,
        TokenKind::MnemonicFlow => theme.code_flow,
        TokenKind::MnemonicMemory => theme.code_memory,
        TokenKind::MnemonicAlu => theme.code_alu,
        TokenKind::MnemonicTransfer => theme.code_transfer,
        TokenKind::MnemonicSystem => theme.code_system,
        TokenKind::Number => theme.code_number,
        TokenKind::ImmPrefix => theme.code_immediate,
        TokenKind::HexPrefix => theme.code_sigil,
        TokenKind::Register => theme.code_register,
        TokenKind::Symbol => theme.code_symbol,
        TokenKind::Punctuation => theme.code_punctuation,
        TokenKind::Plain => theme.text_secondary,
    }
}

#[derive(Clone, Default)]
pub struct DisasmLine {
    pub addr: u32,
    pub address: SharedString,
    pub bytes: SharedString,
    pub text: SharedString,
    pub tokens: Vec<DisasmToken>,
    /// Muted trailing annotation, e.g. `NMITIMEN = $4200` or a resolved branch target.
    pub note: SharedString,
    pub target: Option<u32>,
    pub flow: FlowKind,
}

impl DisasmLine {
    pub fn new(
        addr: u32,
        address: impl Into<SharedString>,
        bytes: impl Into<SharedString>,
        text: impl Into<SharedString>,
        tokens: Vec<DisasmToken>,
    ) -> Self {
        Self {
            addr,
            address: address.into(),
            bytes: bytes.into(),
            text: text.into(),
            tokens,
            note: SharedString::default(),
            target: None,
            flow: FlowKind::None,
        }
    }

    pub fn target(mut self, target: Option<u32>) -> Self {
        self.target = target;
        self
    }

    pub fn note(mut self, note: impl Into<SharedString>) -> Self {
        self.note = note.into();
        self
    }
}

impl From<debugger_model::DisasmLine> for DisasmLine {
    fn from(line: debugger_model::DisasmLine) -> Self {
        Self {
            addr: line.addr,
            address: SharedString::from(line.address.as_ref()),
            bytes: SharedString::from(line.bytes.as_ref()),
            text: SharedString::from(line.text.as_ref()),
            tokens: line.tokens.into_iter().map(Into::into).collect(),
            note: SharedString::from(line.note.as_ref()),
            target: line.target,
            flow: line.flow,
        }
    }
}

fn addr_index(lines: &[DisasmLine]) -> HashMap<u32, usize> {
    let mut index = HashMap::with_capacity(lines.len());
    for (i, line) in lines.iter().enumerate() {
        index.entry(line.addr).or_insert(i);
    }
    index
}

#[derive(Clone, Default)]
pub struct DisassemblyView {
    pub lines: Vec<DisasmLine>,
    pub show_bytes: bool,
}

#[derive(Default)]
pub struct DisassemblyLayoutCache {
    gutters: Option<GutterEntry>,
    graph: Option<GraphEntry>,
}

struct GutterEntry {
    lines: Arc<DisassemblyView>,
    selected: Option<usize>,
    cells: Rc<(Vec<GutterCell>, f32)>,
}

struct GraphEntry {
    lines: Arc<DisassemblyView>,
    layout: Rc<GraphLayout>,
}

impl DisassemblyLayoutCache {
    fn gutters(
        &mut self,
        view: &Arc<DisassemblyView>,
        selected: Option<usize>,
    ) -> Rc<(Vec<GutterCell>, f32)> {
        if let Some(entry) = &self.gutters
            && Arc::ptr_eq(&entry.lines, view)
            && entry.selected == selected
        {
            return entry.cells.clone();
        }

        let cells = Rc::new(arrow_gutters(&view.lines, selected));
        self.gutters = Some(GutterEntry {
            lines: view.clone(),
            selected,
            cells: cells.clone(),
        });

        cells
    }

    fn graph(&mut self, view: &Arc<DisassemblyView>) -> Rc<GraphLayout> {
        if let Some(entry) = &self.graph
            && Arc::ptr_eq(&entry.lines, view)
        {
            return entry.layout.clone();
        }

        let layout = Rc::new(graph::build(&view.lines));
        self.graph = Some(GraphEntry {
            lines: view.clone(),
            layout: layout.clone(),
        });
        layout
    }
}

#[derive(Clone, Default)]
struct RowStates {
    current: Option<usize>,
    current_note: Option<SharedString>,
    selected: Option<usize>,
    breakpoints: Rc<HashMap<usize, BreakpointKind>>,
}

impl RowStates {
    fn is_current(&self, ix: usize) -> bool {
        self.current == Some(ix)
    }

    fn is_selected(&self, ix: usize) -> bool {
        self.selected == Some(ix)
    }

    fn breakpoint(&self, ix: usize) -> Option<BreakpointKind> {
        self.breakpoints.get(&ix).copied()
    }

    fn note<'a>(&'a self, ix: usize, line: &'a DisasmLine) -> &'a SharedString {
        match &self.current_note {
            Some(note) if self.is_current(ix) => note,
            _ => &line.note,
        }
    }
}

#[derive(IntoElement)]
pub struct DisassemblyWidget {
    view: Arc<DisassemblyView>,
    scroll: UniformListScrollHandle,
    id: SharedString,
    title: SharedString,
    collapsed: bool,
    rows: RowStates,
    layout_cache: Option<Rc<RefCell<DisassemblyLayoutCache>>>,
    on_toggle: Option<ClickHandler>,
    on_select: Option<SelectCallback>,
    on_context: Option<ContextCallback>,
    view_mode: ViewMode,
    graph_offset: Point<Pixels>,
    on_set_view: Option<ViewCallback>,
    on_pan_begin: Option<PanCallback>,
    on_pan_drag: Option<PanCallback>,
    follow: bool,
    graph_viewport: Size<f32>,
    on_viewport: Option<ViewportCallback>,
    hovered_edge: Option<usize>,
    on_edge_hover: Option<EdgeHoverCallback>,
    on_jump: Option<JumpCallback>,
}

impl DisassemblyWidget {
    pub const ID: &'static str = "disassembly";

    pub fn new(view: Arc<DisassemblyView>, scroll: UniformListScrollHandle) -> Self {
        Self {
            view,
            scroll,
            id: Self::ID.into(),
            title: "Disassembly".into(),
            collapsed: false,
            rows: RowStates::default(),
            layout_cache: None,
            on_toggle: None,
            on_select: None,
            on_context: None,
            view_mode: ViewMode::Linear,
            graph_offset: Point::default(),
            on_set_view: None,
            on_pan_begin: None,
            on_pan_drag: None,
            follow: false,
            graph_viewport: size(0., 0.),
            on_viewport: None,
            hovered_edge: None,
            on_edge_hover: None,
            on_jump: None,
        }
    }

    pub fn current(mut self, current: Option<usize>) -> Self {
        self.rows.current = current;
        self
    }

    pub fn current_note(mut self, note: Option<SharedString>) -> Self {
        self.rows.current_note = note;
        self
    }

    pub fn selected(mut self, selected: Option<usize>) -> Self {
        self.rows.selected = selected;
        self
    }

    pub fn breakpoints(mut self, breakpoints: Rc<HashMap<usize, BreakpointKind>>) -> Self {
        self.rows.breakpoints = breakpoints;
        self
    }

    pub fn layout_cache(mut self, cache: Rc<RefCell<DisassemblyLayoutCache>>) -> Self {
        self.layout_cache = Some(cache);
        self
    }

    pub fn id(mut self, id: impl Into<SharedString>) -> Self {
        self.id = id.into();
        self
    }

    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = title.into();
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

    pub fn on_context(
        mut self,
        handler: impl Fn(usize, Point<Pixels>, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_context = Some(Rc::new(handler));
        self
    }

    pub fn view_mode(mut self, mode: ViewMode) -> Self {
        self.view_mode = mode;
        self
    }

    pub fn graph_offset(mut self, offset: Point<Pixels>) -> Self {
        self.graph_offset = offset;
        self
    }

    pub fn on_set_view(
        mut self,
        handler: impl Fn(ViewMode, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_set_view = Some(Rc::new(handler));
        self
    }

    pub fn on_pan_begin(
        mut self,
        handler: impl Fn(Point<Pixels>, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_pan_begin = Some(Rc::new(handler));
        self
    }

    pub fn on_pan_drag(
        mut self,
        handler: impl Fn(Point<Pixels>, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_pan_drag = Some(Rc::new(handler));
        self
    }

    pub fn follow(mut self, follow: bool) -> Self {
        self.follow = follow;
        self
    }

    pub fn graph_viewport(mut self, viewport: Size<f32>) -> Self {
        self.graph_viewport = viewport;
        self
    }

    pub fn on_viewport(
        mut self,
        handler: impl Fn(Size<f32>, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_viewport = Some(Rc::new(handler));
        self
    }

    pub fn hovered_edge(mut self, edge: Option<usize>) -> Self {
        self.hovered_edge = edge;
        self
    }

    pub fn on_edge_hover(
        mut self,
        handler: impl Fn(Option<usize>, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_edge_hover = Some(Rc::new(handler));
        self
    }

    pub fn on_jump(
        mut self,
        handler: impl Fn(Point<Pixels>, Point<Pixels>, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_jump = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for DisassemblyWidget {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let DisassemblyWidget {
            view,
            scroll,
            id,
            title,
            collapsed,
            rows,
            layout_cache,
            view_mode,
            graph_offset,
            on_toggle,
            on_select,
            on_context,
            on_set_view,
            on_pan_begin,
            on_pan_drag,
            follow,
            graph_viewport,
            on_viewport,
            hovered_edge,
            on_edge_hover,
            on_jump,
        } = self;

        let current_addr = rows
            .current
            .and_then(|ix| view.lines.get(ix))
            .map(|l| l.address.clone());

        let selector = Segmented::new(SharedString::from(format!("{id}-view")))
            .option("linear", icon::LIST, "Linear")
            .option("graph", icon::GRAPH, "Graph")
            .selected(view_mode.key())
            .on_select(move |key, window, cx| {
                if let Some(cb) = &on_set_view {
                    cb(ViewMode::from_key(key), window, cx);
                }
            });

        let body = match view_mode {
            ViewMode::Linear => self::render_linear(
                view,
                scroll,
                id.clone(),
                rows,
                layout_cache,
                on_select,
                on_context,
            )
            .into_any_element(),
            ViewMode::Graph => {
                let theme = Theme::global(cx);
                self::render_graph(
                    &theme,
                    view,
                    id.clone(),
                    rows,
                    layout_cache,
                    graph_offset,
                    follow,
                    graph_viewport,
                    hovered_edge,
                    on_select,
                    on_context,
                    on_pan_begin,
                    on_pan_drag,
                    on_viewport,
                    on_edge_hover,
                    on_jump,
                )
                .into_any_element()
            }
        };

        let mut panel = Panel::new(id, title)
            .glyph(icon::CODE)
            .collapsed(collapsed)
            .grow(true)
            .on_toggle_opt(on_toggle)
            .header_action(selector)
            .child(body);
        if let Some(addr) = current_addr {
            panel = panel.meta(addr);
        }

        panel
    }
}

struct DisasmRowOptions {
    arrows_width: f32,
    show_bytes: bool,
    rows: RowStates,
    on_select: Option<SelectCallback>,
    on_context: Option<ContextCallback>,
}

fn disasm_row(
    theme: &Theme,
    ix: usize,
    line: &DisasmLine,
    arrows: &GutterCell,
    options: &DisasmRowOptions,
) -> Stateful<Div> {
    let current = options.rows.is_current(ix);
    let selected = options.rows.is_selected(ix);
    let breakpoint = options.rows.breakpoint(ix);
    let note = options.rows.note(ix, line).clone();

    let mut gutter = div()
        .w(px(13.))
        .flex_none()
        .flex()
        .items_center()
        .justify_center();
    if let Some(kind) = breakpoint {
        gutter = gutter.child(
            div()
                .size(px(8.))
                .rounded_full()
                .bg(breakpoint_color(theme, kind)),
        );
    } else if current {
        gutter = gutter.child(
            svg()
                .path(icon::CHEVRON_RIGHT)
                .size(px(11.))
                .text_color(theme.accent),
        );
    }

    let mut row = div()
        .id(("disasm-row", ix))
        .flex()
        .items_center()
        .gap_2()
        .w_full()
        .h(px(ROW_HEIGHT))
        .px_1()
        .rounded_md()
        .when(current, |d| d.bg(theme.accent_soft))
        .when_some(breakpoint.filter(|_| !current), |d, kind| {
            d.bg(breakpoint_color(theme, kind).opacity(0.20))
        })
        .when(selected && !current && breakpoint.is_none(), |d| {
            d.bg(theme.bg_panel_alt)
        })
        .child(gutter);

    if options.arrows_width > 0. {
        let draws = arrows.clone();
        row = row.child(
            canvas(
                |_, _, _| {},
                move |bounds, _, window, _| paint_arrows(window, bounds, &draws),
            )
            .flex_none()
            .w(px(options.arrows_width))
            .h(px(ROW_HEIGHT)),
        );
    }

    row = row.child(
        div()
            .font_family(theme.mono_font)
            .text_sm()
            .min_w(px(58.))
            .flex_none()
            .text_color(theme.code_address)
            .child(line.address.clone()),
    );

    if options.show_bytes {
        row = row.child(
            div()
                .font_family(theme.mono_font)
                .text_sm()
                .w(px(BYTES_WIDTH))
                .flex_none()
                .text_color(theme.text_dim)
                .child(line.bytes.clone()),
        );
    }

    row = row.child(self::instruction_text(
        theme,
        &line.tokens,
        LINEAR_MNEMONIC_W,
    ));

    if !note.is_empty() {
        row = row.child(
            div()
                .ml_auto()
                .pl_4()
                .flex_none()
                .font_family(theme.mono_font)
                .text_sm()
                .text_color(theme.code_comment)
                .child(note),
        );
    }

    if !current && !selected && breakpoint.is_none() {
        let hover_bg = theme.bg_header;
        row = row.hover(move |s| s.bg(hover_bg));
    }
    if let Some(select) = options.on_select.clone() {
        row = row
            .cursor_pointer()
            .on_click(move |_, window, cx| select(ix, window, cx));
    }
    if let Some(context) = options.on_context.clone() {
        row = row.on_mouse_down(MouseButton::Right, move |event, window, cx| {
            context(ix, event.position, window, cx)
        });
    }

    row
}

fn render_linear(
    view: Arc<DisassemblyView>,
    scroll: UniformListScrollHandle,
    id: SharedString,
    rows: RowStates,
    layout_cache: Option<Rc<RefCell<DisassemblyLayoutCache>>>,
    on_select: Option<SelectCallback>,
    on_context: Option<ContextCallback>,
) -> impl IntoElement {
    let list_id = SharedString::from(format!("{id}-list"));
    let gutters = match &layout_cache {
        Some(cache) => cache.borrow_mut().gutters(&view, rows.selected),
        None => Rc::new(arrow_gutters(&view.lines, rows.selected)),
    };
    let count = view.lines.len();
    let empty_gutter = GutterCell::default();
    uniform_list(list_id, count, move |range, _window, cx| {
        let theme = Theme::global(cx);
        let t = &*theme;
        let (cells, arrows_width) = &*gutters;
        let row_options = DisasmRowOptions {
            arrows_width: *arrows_width,
            show_bytes: view.show_bytes,
            rows: rows.clone(),
            on_select: on_select.clone(),
            on_context: on_context.clone(),
        };
        range
            .map(|ix| {
                self::disasm_row(
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

#[allow(clippy::too_many_arguments)]
fn render_graph(
    theme: &Theme,
    view: Arc<DisassemblyView>,
    id: SharedString,
    rows: RowStates,
    layout_cache: Option<Rc<RefCell<DisassemblyLayoutCache>>>,
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
    let layout = match &layout_cache {
        Some(cache) => cache.borrow_mut().graph(&view),
        None => Rc::new(graph::build(&view.lines)),
    };

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
            let theme_now = Theme::global(cx);
            self::paint_edges(
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
            let hit = self::edge_at(&move_layout.edges, e.position, move_origin.get(), offset);
            hover(hit, window, cx);
        }
    });

    if let Some(jump) = on_jump {
        let jump_layout = layout.clone();
        let jump_origin = origin.clone();
        container = container.on_click(move |e, window, cx| {
            let Some(ei) =
                self::edge_at(&jump_layout.edges, e.position(), jump_origin.get(), offset)
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

    let cullable = viewport.width > 0. && viewport.height > 0.;

    for block in layout.blocks.iter() {
        let bx = f32::from(offset.x) + block.x;
        let by = f32::from(offset.y) + block.y;
        if cullable
            && (bx + block.w < -CULL_MARGIN
                || by + block.h < -CULL_MARGIN
                || bx > viewport.width + CULL_MARGIN
                || by > viewport.height + CULL_MARGIN)
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
                    .child(view.lines[block.start].address.clone()),
            );

        let mut body = div().flex().flex_col().flex_1().justify_center();
        for (k, line) in lines.iter().enumerate() {
            let ix = block.start + k;
            let current = rows.is_current(ix);
            let selected = rows.is_selected(ix);
            let breakpoint = rows.breakpoint(ix);
            let note = rows.note(ix, line).clone();

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
            } else if current {
                gutter = gutter.child(
                    svg()
                        .path(icon::CHEVRON_RIGHT)
                        .size(px(10.))
                        .text_color(theme.accent),
                );
            }

            let mut row = div()
                .id(("graph-line", ix))
                .flex()
                .items_center()
                .pr_3()
                .h(px(graph::BLOCK_LINE_H))
                .flex_none()
                .when(current, |d| d.bg(theme.accent_soft))
                .when_some(breakpoint.filter(|_| !current), |d, kind| {
                    d.bg(breakpoint_color(theme, kind).opacity(0.20))
                })
                .when(selected && !current && breakpoint.is_none(), |d| {
                    d.bg(theme.bg_panel_alt)
                })
                .child(gutter)
                .child(self::instruction_text(
                    theme,
                    &line.tokens,
                    GRAPH_MNEMONIC_W,
                ));

            if !note.is_empty() {
                row = row.child(
                    div()
                        .ml_auto()
                        .pl_4()
                        .font_family(theme.mono_font)
                        .text_xs()
                        .text_color(theme.code_comment)
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

fn paint_edges(
    window: &mut Window,
    bounds: Bounds<Pixels>,
    offset: Point<Pixels>,
    edges: &[GraphEdge],
    hovered: Option<usize>,
    theme: &Theme,
) {
    let ox = bounds.origin.x + offset.x;
    let oy = bounds.origin.y + offset.y;
    let at = |p: (f32, f32)| point(ox + px(p.0), oy + px(p.1));

    let cull_left = f32::from(-offset.x);
    let cull_top = f32::from(-offset.y);
    let cull_right = cull_left + f32::from(bounds.size.width);
    let cull_bottom = cull_top + f32::from(bounds.size.height);

    for (ei, edge) in edges.iter().enumerate() {
        if edge.points.len() < 2 {
            continue;
        }

        let (mut min_x, mut min_y) = (f32::MAX, f32::MAX);
        let (mut max_x, mut max_y) = (f32::MIN, f32::MIN);
        for &(x, y) in &edge.points {
            min_x = min_x.min(x);
            min_y = min_y.min(y);
            max_x = max_x.max(x);
            max_y = max_y.max(y);
        }

        if max_x < cull_left || min_x > cull_right || max_y < cull_top || min_y > cull_bottom {
            continue;
        }

        let base = match edge.kind {
            EdgeKind::Taken => theme.success,
            EdgeKind::FallThrough => theme.danger,
            EdgeKind::Normal => theme.accent,
        };
        let (color, width, head_len, head_w) = if hovered == Some(ei) {
            (self::brighten(base), 2.75, 10., 5.)
        } else {
            (base, 1.5, 8., 4.)
        };

        let mut line = PathBuilder::stroke(px(width));
        line.move_to(at(edge.points[0]));
        for &p in &edge.points[1..] {
            line.line_to(at(p));
        }
        if let Ok(path) = line.build() {
            window.paint_path(path, color);
        }

        let n = edge.points.len();
        let (x0, y0) = edge.points[n - 2];
        let (x1, y1) = edge.points[n - 1];
        let (dx, dy) = (x1 - x0, y1 - y0);
        let len = (dx * dx + dy * dy).sqrt().max(0.001);
        let (ux, uy) = (dx / len, dy / len);
        let (nx, ny) = (-uy, ux);
        let (bx, by) = (x1 - ux * head_len, y1 - uy * head_len);

        let mut head = PathBuilder::fill();
        head.move_to(at((x1, y1)));
        head.line_to(at((bx + nx * head_w, by + ny * head_w)));
        head.line_to(at((bx - nx * head_w, by - ny * head_w)));
        head.line_to(at((x1, y1)));
        if let Ok(path) = head.build() {
            window.paint_path(path, color);
        }
    }
}

fn brighten(c: Hsla) -> Hsla {
    Hsla {
        l: (c.l + 0.18).min(1.),
        ..c
    }
}

fn edge_at(
    edges: &[GraphEdge],
    pos: Point<Pixels>,
    origin: Point<Pixels>,
    offset: Point<Pixels>,
) -> Option<usize> {
    graph::edge_at(
        edges,
        f32::from(pos.x - origin.x - offset.x),
        f32::from(pos.y - origin.y - offset.y),
    )
}

fn instruction_text(theme: &Theme, tokens: &[DisasmToken], mnemonic_min: f32) -> Div {
    let mut text = div()
        .flex()
        .items_center()
        .font_family(theme.mono_font)
        .text_sm();
    if let Some(mnemonic) = tokens.first() {
        text = text.child(
            div()
                .min_w(px(mnemonic_min))
                .pr_2()
                .flex_none()
                .text_color(self::token_color(theme, mnemonic.kind))
                .child(mnemonic.text.clone()),
        );
    }
    for tok in tokens.iter().skip(1) {
        text = text.child(
            div()
                .text_color(self::token_color(theme, tok.kind))
                .child(tok.text.clone()),
        );
    }
    text
}
