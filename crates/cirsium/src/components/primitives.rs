use gpui::prelude::*;
use gpui::{App, ClickEvent, Div, Hsla, SharedString, Stateful, Window, div, px, svg};

use crate::assets::icon;
use crate::theme::{MONO_FONT, Theme, with_alpha};

/// A compact colored pill (breakpoint kinds, watch types, status chips).
pub fn badge(text: impl Into<SharedString>, color: Hsla) -> Div {
    div()
        .px_1()
        .font_family(MONO_FONT)
        .text_xs()
        .bg(with_alpha(color, 0.15))
        .text_color(color)
        .child(text.into())
}

/// A thin vertical divider for statusbar segments.
pub fn separator(theme: &Theme) -> Div {
    div().w(px(1.)).h(px(12.)).bg(theme.border_strong)
}

/// A muted, compact field label.
pub fn label_text(theme: &Theme, text: impl Into<SharedString>) -> Div {
    div()
        .text_xs()
        .text_color(theme.text_muted)
        .child(text.into())
}

/// A compact labeled checkbox (e.g. "Follow PC").
pub fn checkbox(
    theme: &Theme,
    id: &'static str,
    checked: bool,
    label: impl Into<SharedString>,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    let mut box_ = div()
        .size(px(13.))
        .flex()
        .items_center()
        .justify_center()
        .rounded_sm()
        .border_1()
        .border_color(if checked {
            theme.accent
        } else {
            theme.border_strong
        });

    if checked {
        box_ = box_.bg(theme.accent_soft).child(
            svg()
                .path(icon::CHECK)
                .size(px(9.))
                .text_color(theme.accent),
        );
    }

    div()
        .id(id)
        .flex()
        .items_center()
        .gap_1()
        .cursor_pointer()
        .on_click(on_click)
        .child(box_)
        .child(
            div()
                .text_xs()
                .text_color(theme.text_secondary)
                .child(label.into()),
        )
}
