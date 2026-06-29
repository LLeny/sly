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

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum CheckboxSize {
    Small,
    Normal,
}

pub fn checkbox(
    theme: &Theme,
    id: &'static str,
    checked: bool,
    label: impl Into<SharedString>,
    size: CheckboxSize,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    let (box_px, check_px) = match size {
        CheckboxSize::Small => (13., 9.),
        CheckboxSize::Normal => (16., 11.),
    };

    let mut box_ = div()
        .size(px(box_px))
        .flex_none()
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
                .size(px(check_px))
                .text_color(theme.accent),
        );
    }

    let label = div()
        .map(|d| match size {
            CheckboxSize::Small => d.text_xs(),
            CheckboxSize::Normal => d.text_sm(),
        })
        .text_color(theme.text_secondary)
        .child(label.into());

    div()
        .id(id)
        .flex()
        .items_center()
        .map(|d| match size {
            CheckboxSize::Small => d.gap_1(),
            CheckboxSize::Normal => d.gap_2().h(px(super::CONTROL_HEIGHT)),
        })
        .cursor_pointer()
        .on_click(on_click)
        .child(box_)
        .child(label)
}
