use gpui::prelude::*;
use gpui::{Div, FontWeight, SharedString, div, px};

use super::primitives::separator;
use crate::theme::Theme;

/// The accented system pill shown at the left of the status bar.
pub fn console_pill(theme: &Theme, label: impl Into<SharedString>) -> Div {
    div()
        .flex()
        .items_center()
        .h(px(18.))
        .px_2()
        .bg(theme.accent_soft)
        .text_color(theme.accent)
        .font_weight(FontWeight::SEMIBOLD)
        .child(label.into())
}

#[derive(Clone, Default)]
pub struct StatusView {
    pub system: SharedString,
    pub session: SharedString,
    pub segments: Vec<SharedString>,
    pub state: SharedString,
    pub right: SharedString,
}

pub fn status_bar(theme: &Theme, status: &StatusView) -> Div {
    let mut left = div()
        .flex()
        .items_center()
        .gap_3()
        .child(console_pill(theme, status.system.clone()));
    if !status.session.is_empty() {
        left = left.child(
            div()
                .text_color(theme.text_secondary)
                .child(status.session.clone()),
        );
    }

    for seg in &status.segments {
        left = left.child(separator(theme)).child(div().child(seg.clone()));
    }

    let mut right = div().flex().items_center().gap_3();
    if !status.state.is_empty() {
        right = right.child(
            div()
                .text_color(theme.text_secondary)
                .child(status.state.clone()),
        );
    }
    if !status.right.is_empty() {
        right = right
            .child(separator(theme))
            .child(div().child(status.right.clone()));
    }

    div()
        .flex()
        .items_center()
        .justify_between()
        .gap_3()
        .px_3()
        .h(px(30.))
        .flex_none()
        .bg(theme.bg_panel)
        .border_t_1()
        .border_color(theme.border_subtle)
        .font_family(theme.ui_font)
        .text_xs()
        .text_color(theme.text_muted)
        .child(left)
        .child(right)
}
