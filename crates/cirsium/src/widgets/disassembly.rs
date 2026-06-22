use std::rc::Rc;

use gpui::{
    App, ClickEvent, Div, Hsla, MouseButton, Pixels, Point, SharedString, Stateful,
    UniformListScrollHandle, Window, div, prelude::*, px, svg, uniform_list,
};

use crate::assets::icon;
use crate::components::{ClickHandler, Panel};
use crate::theme::Theme;

const ROW_HEIGHT: f32 = 20.;
const LIST_HEIGHT: f32 = 260.;
const BYTES_WIDTH: f32 = 96.;

type SelectCallback = Rc<dyn Fn(usize, &mut Window, &mut App) + 'static>;
type ContextCallback = Rc<dyn Fn(usize, Point<Pixels>, &mut Window, &mut App) + 'static>;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TokenKind {
    Mnemonic,
    Number,
    Register,
    Punctuation,
    Plain,
}

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

fn token_color(theme: &Theme, kind: TokenKind) -> Hsla {
    match kind {
        TokenKind::Mnemonic => theme.code_opcode,
        TokenKind::Number => theme.code_number,
        TokenKind::Register => theme.info,
        TokenKind::Punctuation => theme.text_dim,
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
    pub current: bool,
    pub selected: bool,
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
            current,
            selected: false,
        }
    }
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
    collapsed: bool,
    on_toggle: Option<ClickHandler>,
    on_select: Option<SelectCallback>,
    on_context: Option<ContextCallback>,
}

impl DisassemblyWidget {
    pub const ID: &'static str = "disassembly";

    pub fn new(view: DisassemblyView, scroll: UniformListScrollHandle) -> Self {
        Self {
            view,
            scroll,
            collapsed: false,
            on_toggle: None,
            on_select: None,
            on_context: None,
        }
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
}

impl RenderOnce for DisassemblyWidget {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let current_addr = self
            .view
            .lines
            .iter()
            .find(|l| l.current)
            .map(|l| l.address.clone());

        let show_bytes = self.view.show_bytes;
        let lines = self.view.lines;
        let on_select = self.on_select;
        let on_context = self.on_context;
        let count = lines.len();
        let list = uniform_list("disasm-list", count, move |range, _window, cx| {
            let theme = Theme::global(cx);
            let t = &*theme;
            range
                .map(|ix| {
                    disasm_row(
                        t,
                        ix,
                        &lines[ix],
                        show_bytes,
                        on_select.clone(),
                        on_context.clone(),
                    )
                })
                .collect::<Vec<_>>()
        })
        .track_scroll(&self.scroll)
        .h(px(LIST_HEIGHT));

        let mut panel = Panel::new(Self::ID, "Disassembly")
            .glyph(icon::CODE)
            .collapsed(self.collapsed)
            .on_toggle_opt(self.on_toggle)
            .child(list);
        if let Some(addr) = current_addr {
            panel = panel.meta(addr);
        }

        panel
    }
}

fn disasm_row(
    theme: &Theme,
    ix: usize,
    line: &DisasmLine,
    show_bytes: bool,
    on_select: Option<SelectCallback>,
    on_context: Option<ContextCallback>,
) -> Stateful<Div> {
    let mut gutter = div()
        .w(px(13.))
        .flex_none()
        .flex()
        .items_center()
        .justify_center();
    if line.current {
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
        .when(line.selected && !line.current, |d| d.bg(theme.bg_panel_alt))
        .child(gutter)
        .child(
            div()
                .font_family(theme.mono_font)
                .text_sm()
                .min_w(px(58.))
                .flex_none()
                .text_color(theme.code_address)
                .child(line.address.clone()),
        );
    if show_bytes {
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

    let mut text = div()
        .flex()
        .items_center()
        .font_family(theme.mono_font)
        .text_sm();
    if let Some(mnemonic) = line.tokens.first() {
        text = text.child(
            div()
                .min_w(px(42.))
                .flex_none()
                .text_color(token_color(theme, mnemonic.kind))
                .child(mnemonic.text.clone()),
        );
    }
    for tok in line.tokens.iter().skip(1) {
        text = text.child(
            div()
                .text_color(token_color(theme, tok.kind))
                .child(tok.text.clone()),
        );
    }
    row = row.child(text);

    if !line.current && !line.selected {
        let hover_bg = theme.bg_header;
        row = row.hover(move |s| s.bg(hover_bg));
    }
    if let Some(select) = on_select {
        row = row
            .cursor_pointer()
            .on_click(move |_, window, cx| select(ix, window, cx));
    }
    if let Some(context) = on_context {
        row = row.on_mouse_down(MouseButton::Right, move |event, window, cx| {
            context(ix, event.position, window, cx)
        });
    }

    row
}
