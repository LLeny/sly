use gpui::prelude::*;
use gpui::{
    Anchor, App, ClickEvent, Div, MouseButton, MouseDownEvent, Pixels, Point, SharedString,
    Stateful, Window, anchored, deferred, div, hsla, px,
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
    let panel = floating_panel(theme, id, content).on_mouse_down_out(on_dismiss);
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
    let panel = floating_panel(theme, "popover", div().p_2().child(content)).shadow_lg();

    deferred(
        div()
            .occlude()
            .absolute()
            .inset_0()
            .size_full()
            .bg(hsla(0., 0., 0., 0.45))
            .on_mouse_down(MouseButton::Left, on_dismiss)
            .child(
                anchored()
                    .anchor(Anchor::TopRight)
                    .position(position)
                    .snap_to_window_with_margin(px(6.))
                    .child(panel),
            ),
    )
    .priority(1)
}

fn floating_panel(theme: &Theme, id: &'static str, content: impl IntoElement) -> Stateful<Div> {
    div()
        .id(id)
        .occlude()
        .bg(theme.bg_panel)
        .border_1()
        .border_color(theme.border_strong)
        .rounded_md()
        .child(content)
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
            .map(|(i, item)| self::menu_row(theme, i, item)),
    );
    self::overlay(
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
