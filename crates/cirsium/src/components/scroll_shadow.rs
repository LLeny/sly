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
        .child(overflow_shadow(scroll.clone(), theme, Edge::Top))
        .child(overflow_shadow(scroll, theme, Edge::Bottom))
}

pub fn uniform_vertical_scroll_area(
    content: impl IntoElement,
    scroll: &UniformListScrollHandle,
    theme: &Theme,
) -> Div {
    let base = scroll.0.borrow().base_handle.clone();
    vertical_scroll_area(content, base, theme)
}

#[derive(Clone, Copy)]
enum Edge {
    Top,
    Bottom,
}

fn overflow_shadow(scroll: ScrollHandle, theme: &Theme, edge: Edge) -> impl IntoElement {
    let shadow = match theme.mode {
        ThemeMode::Dark => rgba(0x00000090),
        ThemeMode::Light => rgba(0x00000030),
    };
    // Fades from transparent towards the edge it sits on.
    let angle = match edge {
        Edge::Top => 0.,
        Edge::Bottom => 180.,
    };
    let gradient = linear_gradient(
        angle,
        linear_color_stop(rgba(0x00000000), 0.),
        linear_color_stop(shadow, 1.),
    );

    canvas(
        |_, _, _| {},
        move |bounds, _, window, _| {
            let max_offset = scroll.max_offset().y;
            let current_offset = -scroll.offset().y;
            let hidden_content = match edge {
                Edge::Top => current_offset > px(0.5),
                Edge::Bottom => current_offset < max_offset - px(0.5),
            };
            if max_offset <= px(0.) || !hidden_content {
                return;
            }

            window.paint_quad(fill(bounds, gradient));
        },
    )
    .absolute()
    .left_0()
    .right_0()
    .map(|canvas| match edge {
        Edge::Top => canvas.top_0(),
        Edge::Bottom => canvas.bottom_0(),
    })
    .h(px(OVERFLOW_SHADOW_HEIGHT))
}
