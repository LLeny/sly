use gpui::prelude::*;
use gpui::{
    Anchor, App, ClickEvent, MouseDownEvent, Pixels, Point, SharedString, Window, anchored,
    deferred, div, px,
};

use super::ClickHandler;
use crate::theme::Theme;

pub(super) fn overlay<C, D>(
    theme: &Theme,
    position: Point<Pixels>,
    anchor: Anchor,
    id: &'static str,
    content: C,
    on_dismiss: D,
) -> impl IntoElement + use<C, D>
where
    C: IntoElement,
    D: Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
{
    let panel = div()
        .id(id)
        .occlude()
        .bg(theme.bg_panel)
        .border_1()
        .border_color(theme.border_strong)
        .rounded_md()
        .on_mouse_down_out(on_dismiss)
        .child(content);
    deferred(
        anchored()
            .anchor(anchor)
            .position(position)
            .snap_to_window_with_margin(px(6.))
            .child(panel),
    )
    .priority(1)
}

pub fn popover(
    theme: &Theme,
    position: Point<Pixels>,
    content: impl IntoElement,
    on_dismiss: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    overlay(
        theme,
        position,
        Anchor::TopRight,
        "popover",
        div().p_2().child(content),
        on_dismiss,
    )
}

pub struct MenuItem {
    label: SharedString,
    on_click: ClickHandler,
}

impl MenuItem {
    pub fn new(
        label: impl Into<SharedString>,
        on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        Self {
            label: label.into(),
            on_click: Box::new(on_click),
        }
    }
}

pub fn context_menu(
    theme: &Theme,
    position: Point<Pixels>,
    items: Vec<MenuItem>,
    on_dismiss: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    let list = div().min_w(px(180.)).flex().flex_col().py_1().children(
        items
            .into_iter()
            .enumerate()
            .map(|(i, item)| menu_row(theme, i, item)),
    );
    overlay(
        theme,
        position,
        Anchor::TopLeft,
        "context-menu",
        list,
        on_dismiss,
    )
}

fn menu_row(theme: &Theme, i: usize, item: MenuItem) -> impl IntoElement {
    let accent = theme.accent;
    let accent_soft = theme.accent_soft;
    div()
        .id(("menu-item", i))
        .px_2()
        .py_1()
        .text_xs()
        .text_color(theme.text_secondary)
        .cursor_pointer()
        .hover(move |s| s.bg(accent_soft).text_color(accent))
        .on_click(item.on_click)
        .child(item.label)
}
