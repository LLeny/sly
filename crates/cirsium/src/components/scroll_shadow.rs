use gpui::prelude::*;
use gpui::{
    Div, ScrollHandle, UniformListScrollHandle, canvas, div, fill, linear_color_stop,
    linear_gradient, px, rgba,
};

use crate::theme::{Theme, ThemeMode};

const OVERFLOW_SHADOW_HEIGHT: f32 = 18.;

pub fn vertical_scroll_area(content: impl IntoElement, scroll: ScrollHandle, theme: &Theme) -> Div {
    div()
        .relative()
        .flex()
        .flex_col()
        .min_h(px(0.))
        .child(content)
        .child(overflow_shadow(scroll, theme))
}

pub fn uniform_vertical_scroll_area(
    content: impl IntoElement,
    scroll: &UniformListScrollHandle,
    theme: &Theme,
) -> Div {
    let base = scroll.0.borrow().base_handle.clone();
    vertical_scroll_area(content, base, theme)
}

fn overflow_shadow(scroll: ScrollHandle, theme: &Theme) -> impl IntoElement {
    let shadow = match theme.mode {
        ThemeMode::Dark => rgba(0x00000090),
        ThemeMode::Light => rgba(0x00000030),
    };
    let gradient = linear_gradient(
        180.,
        linear_color_stop(rgba(0x00000000), 0.),
        linear_color_stop(shadow, 1.),
    );

    canvas(
        |_, _, _| {},
        move |bounds, _, window, _| {
            let max_offset = scroll.max_offset().y;
            let current_offset = -scroll.offset().y;
            if max_offset <= px(0.) || current_offset >= max_offset - px(0.5) {
                return;
            }

            window.paint_quad(fill(bounds, gradient));
        },
    )
    .absolute()
    .left_0()
    .right_0()
    .bottom_0()
    .h(px(OVERFLOW_SHADOW_HEIGHT))
}
