use std::rc::Rc;

use gpui::prelude::*;
use gpui::{
    Anchor, App, ClickEvent, MouseDownEvent, Pixels, Point, SharedString, Window, div, px, svg,
};

use super::menu::overlay;
use crate::assets::icon;
use crate::theme::Theme;

#[derive(Clone)]
pub struct ComboOption {
    pub id: SharedString,
    pub label: SharedString,
}

impl ComboOption {
    pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
        }
    }
}

pub fn combobox_button(
    theme: &Theme,
    id: impl Into<SharedString>,
    label: impl Into<SharedString>,
    on_open: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    let hover_bg = theme.bg_header;
    let hover_border = theme.border_strong;

    div()
        .id(id.into())
        .flex()
        .items_center()
        .gap_2()
        .h(px(super::CONTROL_HEIGHT))
        .px_2()
        .bg(theme.bg_panel_alt)
        .border_1()
        .border_color(theme.border_subtle)
        .rounded_md()
        .cursor_pointer()
        .hover(move |s| s.bg(hover_bg).border_color(hover_border))
        .child(
            div()
                .font_family(theme.ui_font)
                .text_xs()
                .text_color(theme.text_primary)
                .child(label.into()),
        )
        .child(
            svg()
                .path(icon::CHEVRON_DOWN)
                .size(px(11.))
                .text_color(theme.text_muted),
        )
        .on_click(on_open)
}

pub fn combobox_menu<F, G>(
    theme: &Theme,
    position: Point<Pixels>,
    options: &[ComboOption],
    selected: &str,
    on_select: F,
    on_dismiss: G,
) -> impl IntoElement + use<F, G>
where
    F: Fn(SharedString, &mut Window, &mut App) + 'static,
    G: Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
{
    let on_select = Rc::new(on_select);
    let accent = theme.accent;
    let accent_soft = theme.accent_soft;

    let list =
        div()
            .min_w(px(160.))
            .flex()
            .flex_col()
            .py_1()
            .children(options.iter().enumerate().map(|(i, opt)| {
                let is_selected = opt.id.as_ref() == selected;
                let id = opt.id.clone();
                let on_select = on_select.clone();
                div()
                    .id(("combo-option", i))
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_3()
                    .px_2()
                    .py_1()
                    .text_xs()
                    .cursor_pointer()
                    .text_color(if is_selected {
                        accent
                    } else {
                        theme.text_secondary
                    })
                    .when(is_selected, |d| d.bg(accent_soft))
                    .hover(move |s| s.bg(accent_soft).text_color(accent))
                    .on_click(move |_, window, cx| on_select(id.clone(), window, cx))
                    .child(div().font_family(theme.ui_font).child(opt.label.clone()))
                    .when(is_selected, |d| {
                        d.child(svg().path(icon::CHECK).size(px(10.)).text_color(accent))
                    })
            }));

    overlay(
        theme,
        position,
        Anchor::TopLeft,
        "combobox-menu",
        list,
        on_dismiss,
    )
}
