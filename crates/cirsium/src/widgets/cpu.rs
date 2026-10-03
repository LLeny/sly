use gpui::prelude::*;
use gpui::{App, ClickEvent, SharedString, Window, div};
use std::sync::Arc;

use crate::assets::icon;
use crate::components::{ClickHandler, Copyable, Panel, bool_chip, label_text};
use crate::diff::highlight_color;
use crate::theme::Theme;

pub use debugger_model::{CpuView, FlagView, RegisterView};

#[derive(IntoElement)]
pub struct CpuWidget {
    view: Arc<CpuView>,
    id: SharedString,
    title: SharedString,
    collapsed: bool,
    on_toggle: Option<ClickHandler>,
}

impl CpuWidget {
    pub const ID: &'static str = "cpu";

    pub fn new(view: Arc<CpuView>) -> Self {
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

        let columns = if self
            .view
            .registers
            .iter()
            .any(|register| register.bits > 24)
        {
            2
        } else {
            3
        };
        let registers =
            div()
                .flex()
                .flex_col()
                .gap_1()
                .children(self.view.registers.chunks(columns).map(|row| {
                    div()
                        .flex()
                        .gap_1()
                        .children(row.iter().map(|r| self::reg_cell(t, &self.id, r)))
                }));

        let flags = div()
            .flex()
            .flex_wrap()
            .gap_1()
            .items_center()
            .justify_between()
            .pt_2()
            .border_t_1()
            .border_color(t.border_subtle)
            .children(
                self.view
                    .flags
                    .iter()
                    .map(|f| bool_chip(t, f.name.clone(), f.set)),
            );

        let body = div()
            .flex()
            .flex_col()
            .gap_2()
            .child(registers)
            .child(flags);

        Panel::new(self.id, self.title)
            .glyph(icon::CPU)
            .meta(SharedString::from(self.view.isa.clone()))
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
