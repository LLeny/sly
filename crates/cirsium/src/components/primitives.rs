use gpui::prelude::*;
use gpui::{App, ClickEvent, Div, FontWeight, Hsla, SharedString, Stateful, Window, div, px, svg};

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

/// A monospace on/off chip, accented when set (CPU flags, MMIO bit fields).
pub fn bool_chip(theme: &Theme, text: impl Into<SharedString>, on: bool) -> Div {
    div()
        .px_1()
        .font_family(theme.mono_font)
        .text_xs()
        .font_weight(FontWeight::MEDIUM)
        .when(on, |d| d.bg(theme.accent_soft).text_color(theme.accent))
        .when(!on, |d| {
            d.bg(theme.bg_panel_alt).text_color(theme.text_muted)
        })
        .child(text.into())
}

/// Flat monospace state label for dense status rows. Unlike [`bool_chip`], this
/// has no background.
pub fn status_flag(theme: &Theme, text: impl Into<SharedString>, on: bool) -> Div {
    div()
        .font_family(theme.mono_font)
        .text_xs()
        .font_weight(FontWeight::MEDIUM)
        .text_color(if on { theme.accent } else { theme.text_dim })
        .child(text.into())
}

pub use debugger_model::hex;

pub fn flip_label(flip_h: bool, flip_v: bool) -> &'static str {
    match (flip_h, flip_v) {
        (false, false) => "",
        (true, false) => " │ flip H",
        (false, true) => " │ flip V",
        (true, true) => " │ flip HV",
    }
}

/// A thin vertical divider for statusbar segments.
pub fn separator(theme: &Theme) -> Div {
    div().w(px(1.)).h(px(12.)).bg(theme.border_strong)
}

/// A small square icon button that sits beside a text input.
pub fn field_button(
    theme: &Theme,
    id: &'static str,
    icon_path: &'static str,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    let hover_bg = theme.bg_header;
    div()
        .id(id)
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .h(px(super::CONTROL_HEIGHT))
        .w(px(26.))
        .bg(theme.bg_panel_alt)
        .border_1()
        .border_color(theme.border_subtle)
        .rounded_md()
        .cursor_pointer()
        .hover(move |s| s.bg(hover_bg))
        .on_click(on_click)
        .child(
            svg()
                .path(icon_path)
                .size(px(12.))
                .text_color(theme.text_secondary),
        )
}

/// A thin horizontal magnitude bar: a track with a fractional fill. `frac` is
/// clamped to `0.0..=1.0`; `accent` picks the accent fill over a dim one.
pub fn meter_bar(theme: &Theme, frac: f32, accent: bool) -> Div {
    let fill = if accent { theme.accent } else { theme.text_dim };
    div()
        .flex_1()
        .min_w(px(6.))
        .h(px(6.))
        .bg(theme.bg_panel_alt)
        .rounded_sm()
        .child(
            div()
                .h_full()
                .w(gpui::relative(frac.clamp(0.0, 1.0)))
                .bg(fill)
                .rounded_sm(),
        )
}

/// The rule under a section heading or table header row: horizontal padding
/// plus a subtle bottom border.
pub fn header_rule(theme: &Theme) -> Div {
    div()
        .px_1()
        .pb(px(3.))
        .mb(px(3.))
        .border_b_1()
        .border_color(theme.border_subtle)
}

/// An uppercase section heading over a [`header_rule`].
pub fn section_header(theme: &Theme, text: impl Into<SharedString>) -> Div {
    header_rule(theme)
        .text_xs()
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(theme.text_muted)
        .whitespace_nowrap()
        .child(text.into().to_uppercase())
}

/// A muted, compact field label.
pub fn label_text(theme: &Theme, text: impl Into<SharedString>) -> Div {
    div()
        .text_xs()
        .text_color(theme.text_muted)
        .child(text.into())
}

/// A compact monospace value (addresses, raw register contents).
pub fn mono_text(theme: &Theme, text: impl Into<SharedString>) -> Div {
    div()
        .font_family(theme.mono_font)
        .text_xs()
        .text_color(theme.text_secondary)
        .child(text.into())
}

/// A dim placeholder message for panels with nothing to show.
pub fn empty_state(theme: &Theme, text: impl Into<SharedString>) -> Div {
    div()
        .px_1()
        .py_1()
        .text_xs()
        .text_color(theme.text_dim)
        .child(text.into())
}

/// [`empty_state`] centered within the space it fills (for panels that grow).
pub fn empty_state_centered(theme: &Theme, text: impl Into<SharedString>) -> Div {
    div()
        .flex()
        .flex_1()
        .items_center()
        .justify_center()
        .child(empty_state(theme, text))
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
        box_ = box_.bg(theme.accent).child(
            svg()
                .path(icon::CHECK)
                .size(px(check_px))
                .text_color(theme.on_accent),
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
