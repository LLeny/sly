use std::collections::HashMap;
use std::rc::Rc;

use gpui::prelude::*;
use gpui::{
    App, Bounds, ClickEvent, Div, Hsla, MouseButton, MouseMoveEvent, PathBuilder, Pixels, Point,
    SharedString, Size, Stateful, UniformListScrollHandle, Window, canvas, div, point, px, size,
    svg, uniform_list,
};

use super::breakpoints::BreakpointKind;
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
    pub current: bool,
    pub selected: bool,
    pub breakpoint: Option<BreakpointKind>,
}

impl DisasmLine {
    pub fn new(
        addr: u32,
        address: impl Into<SharedString>,
        bytes: impl Into<SharedString>,
        text: impl Into<SharedString>,
        tokens: Vec<DisasmToken>,
        current: bool,
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
            current,
            selected: false,
            breakpoint: None,
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
            current: line.current,
            selected: false,
            breakpoint: None,
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

#[derive(IntoElement)]
pub struct DisassemblyWidget {
    view: DisassemblyView,
    scroll: UniformListScrollHandle,
    id: SharedString,
    title: SharedString,
    collapsed: bool,
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
}

impl DisassemblyWidget {
    pub const ID: &'static str = "disassembly";

    pub fn new(view: DisassemblyView, scroll: UniformListScrollHandle) -> Self {
        Self {
            view,
            scroll,
            id: Self::ID.into(),
            title: "Disassembly".into(),
            collapsed: false,
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
        }
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
}

impl RenderOnce for DisassemblyWidget {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let DisassemblyWidget {
            view,
            scroll,
            id,
            title,
            collapsed,
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
        } = self;

        let current_addr = view
            .lines
            .iter()
            .find(|l| l.current)
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
            ViewMode::Linear => {
                render_linear(view, scroll, id.clone(), on_select, on_context).into_any_element()
            }
            ViewMode::Graph => {
                let theme = Theme::global(cx);
                render_graph(
                    &theme,
                    view,
                    id.clone(),
                    graph_offset,
                    follow,
                    graph_viewport,
                    on_select,
                    on_pan_begin,
                    on_pan_drag,
                    on_viewport,
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
    let mut gutter = div()
        .w(px(13.))
        .flex_none()
        .flex()
        .items_center()
        .justify_center();
    if let Some(kind) = line.breakpoint {
        gutter = gutter.child(div().size(px(8.)).rounded_full().bg(kind.color(theme)));
    } else if line.current {
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
        .when(line.current, |d| d.bg(theme.accent_soft))
        .when_some(line.breakpoint.filter(|_| !line.current), |d, kind| {
            d.bg(kind.color(theme).opacity(0.20))
        })
        .when(
            line.selected && !line.current && line.breakpoint.is_none(),
            |d| d.bg(theme.bg_panel_alt),
        )
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

    row = row.child(instruction_text(theme, &line.tokens, LINEAR_MNEMONIC_W));

    if !line.note.is_empty() {
        row = row.child(
            div()
                .ml_auto()
                .pl_4()
                .flex_none()
                .font_family(theme.mono_font)
                .text_sm()
                .text_color(theme.code_comment)
                .child(line.note.clone()),
        );
    }

    if !line.current && !line.selected && line.breakpoint.is_none() {
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
    view: DisassemblyView,
    scroll: UniformListScrollHandle,
    id: SharedString,
    on_select: Option<SelectCallback>,
    on_context: Option<ContextCallback>,
) -> impl IntoElement {
    let list_id = SharedString::from(format!("{id}-list"));
    let (gutters, arrows_width) = arrow_gutters(&view.lines);
    let count = view.lines.len();
    let empty_gutter = GutterCell::default();
    uniform_list(list_id, count, move |range, _window, cx| {
        let theme = Theme::global(cx);
        let t = &*theme;
        let row_options = DisasmRowOptions {
            arrows_width,
            show_bytes: view.show_bytes,
            on_select: on_select.clone(),
            on_context: on_context.clone(),
        };
        range
            .map(|ix| {
                disasm_row(
                    t,
                    ix,
                    &view.lines[ix],
                    gutters.get(ix).unwrap_or(&empty_gutter),
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
    view: DisassemblyView,
    id: SharedString,
    manual_offset: Point<Pixels>,
    follow: bool,
    viewport: Size<f32>,
    on_select: Option<SelectCallback>,
    on_pan_begin: Option<PanCallback>,
    on_pan_drag: Option<PanCallback>,
    on_viewport: Option<ViewportCallback>,
) -> impl IntoElement {
    let GraphLayout { blocks, edges } = graph::build(&view.lines);

    let base = if follow {
        blocks
            .iter()
            .find_map(|b| {
                let li = view.lines[b.start..b.end].iter().position(|l| l.current)?;
                Some((b, li))
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

    let canvas_el = canvas(
        move |bounds, window, cx| {
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
            paint_edges(window, bounds, offset, &edges, &theme_now);
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
        .child(canvas_el);

    if let Some(begin) = on_pan_begin {
        container = container.on_mouse_down(MouseButton::Left, move |e, window, cx| {
            begin(e.position, window, cx)
        });
    }
    if let Some(drag) = on_pan_drag {
        container = container.on_mouse_move(move |e: &MouseMoveEvent, window, cx| {
            if e.dragging() {
                drag(e.position, window, cx);
            }
        });
    }

    for block in &blocks {
        let lines = &view.lines[block.start..block.end];
        let has_current = lines.iter().any(|l| l.current);
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
            let mut row = div()
                .id(("graph-line", ix))
                .flex()
                .items_center()
                .px_2()
                .h(px(graph::BLOCK_LINE_H))
                .flex_none()
                .when(line.current, |d| d.bg(theme.accent_soft))
                .when(line.selected && !line.current, |d| d.bg(theme.bg_panel_alt))
                .child(instruction_text(theme, &line.tokens, GRAPH_MNEMONIC_W));

            if !line.note.is_empty() {
                row = row.child(
                    div()
                        .ml_2()
                        .font_family(theme.mono_font)
                        .text_xs()
                        .text_color(theme.code_comment)
                        .child(line.note.clone()),
                );
            }

            if let Some(select) = on_select.clone() {
                row = row
                    .cursor_pointer()
                    .on_click(move |_, window, cx| select(ix, window, cx));
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
    theme: &Theme,
) {
    let ox = bounds.origin.x + offset.x;
    let oy = bounds.origin.y + offset.y;
    let at = |p: (f32, f32)| point(ox + px(p.0), oy + px(p.1));

    for edge in edges {
        if edge.points.len() < 2 {
            continue;
        }

        let color = match edge.kind {
            EdgeKind::Taken => theme.success,
            EdgeKind::FallThrough => theme.danger,
            EdgeKind::Normal => theme.accent,
        };

        let mut line = PathBuilder::stroke(px(1.5));
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
        let (bx, by) = (x1 - ux * 8., y1 - uy * 8.);

        let mut head = PathBuilder::fill();
        head.move_to(at((x1, y1)));
        head.line_to(at((bx + nx * 4., by + ny * 4.)));
        head.line_to(at((bx - nx * 4., by - ny * 4.)));
        head.line_to(at((x1, y1)));
        if let Ok(path) = head.build() {
            window.paint_path(path, color);
        }
    }
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
                .flex_none()
                .text_color(token_color(theme, mnemonic.kind))
                .child(mnemonic.text.clone()),
        );
    }
    for tok in tokens.iter().skip(1) {
        text = text.child(
            div()
                .text_color(token_color(theme, tok.kind))
                .child(tok.text.clone()),
        );
    }
    text
}
