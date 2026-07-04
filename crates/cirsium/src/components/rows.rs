use std::rc::Rc;

use gpui::prelude::*;
use gpui::{App, Div, SharedString, Stateful, Window, div, px, svg};

use crate::assets::icon;
use crate::components::label_text;
use crate::theme::Theme;

pub type RowCallback = Rc<dyn Fn(SharedString, &mut Window, &mut App) + 'static>;

pub const CHEVRON_SIZE: f32 = 12.;
pub const ROW_PAD: f32 = 4.;
pub const ROW_GAP: f32 = 8.;
pub const DETAIL_INDENT: f32 = ROW_PAD + CHEVRON_SIZE + ROW_GAP;
pub const INDENT_STEP: f32 = 12.;

pub fn chevron_slot(theme: &Theme, expandable: bool, expanded: bool) -> Div {
    div()
        .flex_none()
        .size(px(CHEVRON_SIZE))
        .when(expandable, |d| {
            d.child(
                svg()
                    .path(if expanded {
                        icon::CHEVRON_DOWN
                    } else {
                        icon::CHEVRON_RIGHT
                    })
                    .size(px(CHEVRON_SIZE))
                    .text_color(theme.text_muted),
            )
        })
}

pub fn row_toggle(
    row: Stateful<Div>,
    theme: &Theme,
    id: SharedString,
    handler: RowCallback,
) -> Stateful<Div> {
    row.cursor_pointer()
        .hover(|s| s.bg(theme.bg_panel_alt))
        .on_click(move |_, window, cx| handler(id.clone(), window, cx))
}

pub fn detail_row(theme: &Theme, label: impl Into<SharedString>, value: impl IntoElement) -> Div {
    div()
        .flex()
        .items_center()
        .gap_2()
        .pl(px(DETAIL_INDENT))
        .pr_1()
        .py(px(2.))
        .child(label_text(theme, label))
        .child(div().flex_1())
        .child(value)
}

/// A label/value row pinned under a panel's scroll area (selection details).
pub fn footer_row(theme: &Theme, label: impl Into<SharedString>, value: impl IntoElement) -> Div {
    div()
        .flex()
        .items_center()
        .gap_2()
        .px_1()
        .pt(px(6.))
        .child(label_text(theme, label))
        .child(div().flex_1())
        .child(value)
}
