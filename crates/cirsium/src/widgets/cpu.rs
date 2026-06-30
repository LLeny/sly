use gpui::prelude::*;
use gpui::{App, ClickEvent, Div, FontWeight, SharedString, Window, div};

use crate::assets::icon;
use crate::components::{ClickHandler, Copyable, Panel, label_text};
use crate::diff::{Highlight, highlight_color};
use crate::theme::Theme;

#[derive(Clone)]
pub struct RegisterView {
    pub name: SharedString,
    pub value: u32,
    pub bits: u8,
    pub highlight: Highlight,
}

impl RegisterView {
    pub fn new(name: impl Into<SharedString>, value: u32, bits: u8) -> Self {
        Self {
            name: name.into(),
            value,
            bits,
            highlight: Highlight::None,
        }
    }

    fn hex(&self) -> String {
        let width = (self.bits as usize).div_ceil(4);
        format!("${:0width$X}", self.value, width = width)
    }
}

#[derive(Clone)]
pub struct FlagView {
    pub name: SharedString,
    pub set: bool,
}

impl FlagView {
    pub fn new(name: impl Into<SharedString>, set: bool) -> Self {
        Self {
            name: name.into(),
            set,
        }
    }
}

#[derive(Clone, Default)]
pub struct CpuView {
    pub isa: SharedString,
    pub registers: Vec<RegisterView>,
    pub flags: Vec<FlagView>,
}

#[derive(IntoElement)]
pub struct CpuWidget {
    view: CpuView,
    id: SharedString,
    title: SharedString,
    collapsed: bool,
    on_toggle: Option<ClickHandler>,
}

impl CpuWidget {
    pub const ID: &'static str = "cpu";

    pub fn new(view: CpuView) -> Self {
        Self {
            view,
            id: Self::ID.into(),
            title: "CPU".into(),
            collapsed: false,
            on_toggle: None,
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
}

impl RenderOnce for CpuWidget {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::global(cx);
        let t = &*theme;

        let registers =
            div()
                .flex()
                .flex_col()
                .gap_1()
                .children(self.view.registers.chunks(3).map(|row| {
                    div()
                        .flex()
                        .gap_1()
                        .children(row.iter().map(|r| reg_cell(t, &self.id, r)))
                }));

        let flags = div()
            .flex()
            .items_center()
            .justify_between()
            .pt_2()
            .border_t_1()
            .border_color(t.border_subtle)
            .children(self.view.flags.iter().map(|f| flag_chip(t, f)));

        let body = div()
            .flex()
            .flex_col()
            .gap_2()
            .child(registers)
            .child(flags);

        Panel::new(self.id, self.title)
            .glyph(icon::CPU)
            .meta(self.view.isa)
            .collapsed(self.collapsed)
            .on_toggle_opt(self.on_toggle)
            .child(body)
    }
}

fn reg_cell(theme: &Theme, id: &str, r: &RegisterView) -> Copyable {
    let hex = r.hex();
    let inner = div()
        .flex()
        .items_center()
        .justify_between()
        .w_full()
        .child(label_text(theme, r.name.clone()))
        .child(
            div()
                .font_family(theme.mono_font)
                .text_color(highlight_color(theme, r.highlight))
                .child(hex.clone()),
        );

    Copyable::new(format!("{id}-reg-{}", r.name), hex)
        .cell()
        .child(inner)
}

fn flag_chip(theme: &Theme, f: &FlagView) -> Div {
    div()
        .px_1()
        .font_family(theme.mono_font)
        .text_xs()
        .font_weight(FontWeight::MEDIUM)
        .when(f.set, |d| d.bg(theme.accent_soft).text_color(theme.accent))
        .when(!f.set, |d| {
            d.bg(theme.bg_panel_alt).text_color(theme.text_muted)
        })
        .child(f.name.clone())
}
