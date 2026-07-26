use gpui::prelude::*;

use gpui::{
    App, ClickEvent, Div, FontWeight, Hsla, MouseButton, Pixels, Point, SharedString, Stateful,
    Window, div, point, px, svg,
};

#[cfg(all(not(target_os = "macos"), not(target_family = "wasm")))]
use crate::assets::icon;
use crate::theme::Theme;

pub const TITLE_BAR_HEIGHT: f32 = 36.;
const MAC_WINDOW_BUTTON_HEIGHT: f32 = 16.;

pub fn traffic_light_position() -> Point<Pixels> {
    point(
        px(12.),
        px((TITLE_BAR_HEIGHT - MAC_WINDOW_BUTTON_HEIGHT) / 2.),
    )
}

/// A titlebar region that swallows left-clicks so its contents don't start a
/// window drag.
fn drag_shield() -> Div {
    div()
        .flex()
        .items_center()
        .h_full()
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
}

pub fn title_bar(
    theme: &Theme,
    title: impl Into<SharedString>,
    leading: impl IntoElement,
    trailing: impl IntoElement,
) -> Div {
    let left_controls = drag_shield().gap_2().child(leading);

    let left = div()
        .flex_1()
        .h_full()
        .flex()
        .items_center()
        .px_2()
        .child(left_controls);

    #[cfg(target_os = "macos")]
    let left = left.pl(px(88.));

    let center = div()
        .flex_1()
        .h_full()
        .flex()
        .items_center()
        .justify_center()
        .child(
            div()
                .text_sm()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme.text_secondary)
                .child(title.into()),
        );

    let right_controls = drag_shield().gap_2().px_2().child(trailing);

    let right = div()
        .flex_1()
        .h_full()
        .flex()
        .items_center()
        .justify_end()
        .child(right_controls);

    #[cfg(all(not(target_os = "macos"), not(target_family = "wasm")))]
    let right = right.child(
        drag_shield()
            .child(self::window_button(
                theme,
                "win-min",
                false,
                icon::MINIMIZE,
                |w, _| w.minimize_window(),
            ))
            .child(self::window_button(
                theme,
                "win-max",
                false,
                icon::MAXIMIZE,
                |w, _| w.zoom_window(),
            ))
            .child(self::window_button(
                theme,
                "win-close",
                true,
                icon::CLOSE,
                |_, cx| cx.quit(),
            )),
    );

    let bar = div()
        .flex()
        .items_center()
        .h(px(TITLE_BAR_HEIGHT))
        .flex_none()
        .bg(theme.bg_panel)
        .border_b_1()
        .border_color(theme.border_subtle)
        .font_family(theme.ui_font)
        .child(left)
        .child(center)
        .child(right);

    #[cfg(not(target_family = "wasm"))]
    let bar = bar.on_mouse_down(MouseButton::Left, |_, window, _| window.start_window_move());

    bar
}

pub fn icon_button(
    theme: &Theme,
    id: &'static str,
    hover_bg: Hsla,
    icon_path: &'static str,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    let icon_color = theme.text_secondary;
    let icon_hover = theme.text_primary;

    div()
        .id(id)
        .group(id)
        .w(px(46.))
        .h_full()
        .flex()
        .items_center()
        .justify_center()
        .hover(move |s| s.bg(hover_bg))
        .on_click(on_click)
        .child(
            svg()
                .path(icon_path)
                .size(px(15.))
                .text_color(icon_color)
                .group_hover(id, move |s| s.text_color(icon_hover)),
        )
}

pub fn window_button(
    theme: &Theme,
    id: &'static str,
    danger: bool,
    icon_path: &'static str,
    action: impl Fn(&mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    let hover_bg = if danger {
        theme.danger_strong
    } else {
        theme.bg_panel_alt
    };
    self::icon_button(theme, id, hover_bg, icon_path, move |_, window, cx| {
        action(window, cx)
    })
}
