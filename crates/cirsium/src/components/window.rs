use gpui::prelude::*;
use gpui::{
    App, ClickEvent, Div, FontWeight, Hsla, MouseButton, SharedString, Stateful, Window, div, px,
    svg,
};

#[cfg(not(target_os = "macos"))]
use crate::assets::icon;
use crate::theme::Theme;

pub fn title_bar(theme: &Theme, title: impl Into<SharedString>, controls: impl IntoElement) -> Div {
    let leading = div()
        .flex_1()
        .h_full()
        .flex()
        .items_center()
        .gap_2()
        .px_3()
        .on_mouse_down(MouseButton::Left, |_, window, _| window.start_window_move());

    #[cfg(not(target_os = "macos"))]
    let leading = leading.child(svg().path(icon::LOGO).size(px(15.)).text_color(theme.logo));

    let leading = leading.child(
        div()
            .text_xs()
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(theme.text_secondary)
            .child(title.into()),
    );

    #[cfg(target_os = "macos")]
    let leading = leading.pl(px(92.));

    let trailing = div().flex().items_center().h_full().child(controls);

    #[cfg(not(target_os = "macos"))]
    let trailing = trailing
        .child(window_button(
            theme,
            "win-min",
            false,
            icon::MINIMIZE,
            |w, _| w.minimize_window(),
        ))
        .child(window_button(
            theme,
            "win-max",
            false,
            icon::MAXIMIZE,
            |w, _| w.zoom_window(),
        ))
        .child(window_button(
            theme,
            "win-close",
            true,
            icon::CLOSE,
            |_, cx| cx.quit(),
        ));

    div()
        .flex()
        .items_center()
        .h(px(34.))
        .flex_none()
        .bg(theme.bg_panel)
        .border_b_1()
        .border_color(theme.border_subtle)
        .font_family(theme.ui_font)
        .child(leading)
        .child(trailing)
}

pub fn icon_button(
    theme: &Theme,
    id: &'static str,
    hover_bg: Hsla,
    icon_path: &'static str,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    let icon_color = theme.text_secondary;
    div()
        .id(id)
        .w(px(46.))
        .h_full()
        .flex()
        .items_center()
        .justify_center()
        .hover(move |s| s.bg(hover_bg))
        .on_click(on_click)
        .child(svg().path(icon_path).size(px(15.)).text_color(icon_color))
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
    icon_button(theme, id, hover_bg, icon_path, move |_, window, cx| {
        action(window, cx)
    })
}
