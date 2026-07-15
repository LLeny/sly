use gpui::prelude::*;
use gpui::{App, Bounds, ClickEvent, Hsla, Pixels, Window, canvas, div, fill, point, px, size};

use crate::components::{ClickHandler, Panel, empty_state_centered};
use crate::theme::{Theme, with_alpha};
use crate::widgets::scope::scope_baseline;

pub use debugger_model::AudioScopesView;

#[derive(IntoElement)]
pub struct ChannelScopesWidget {
    id: gpui::SharedString,
    title: gpui::SharedString,
    view: AudioScopesView,
    glyph: Option<gpui::SharedString>,
    collapsed: bool,
    on_toggle: Option<ClickHandler>,
}

impl ChannelScopesWidget {
    pub fn new(
        id: impl Into<gpui::SharedString>,
        title: impl Into<gpui::SharedString>,
        view: AudioScopesView,
    ) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            view,
            glyph: None,
            collapsed: false,
            on_toggle: None,
        }
    }

    pub fn glyph(mut self, glyph: impl Into<gpui::SharedString>) -> Self {
        self.glyph = Some(glyph.into());
        self
    }

    pub fn collapsed(mut self, collapsed: bool) -> Self {
        self.collapsed = collapsed;
        self
    }

    pub fn on_toggle(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_toggle = Some(Box::new(handler));
        self
    }
}

impl RenderOnce for ChannelScopesWidget {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::global(cx);
        let t = &*theme;

        let has_data = self.view.channels.iter().any(|c| c.waveform.len() > 1);

        let body = if self.view.channels.is_empty() || !has_data {
            empty_state_centered(t, "No audio").into_any_element()
        } else {
            self.view
                .channels
                .into_iter()
                .fold(
                    div()
                        .id("channel-scopes")
                        .flex()
                        .flex_col()
                        .flex_1()
                        .min_h(px(0.))
                        .gap(px(3.))
                        .pb(px(2.))
                        .overflow_y_scroll(),
                    |col, c| {
                        col.child(scope_row(t, c.label, c.active, c.waveform))
                    },
                )
                .into_any_element()
        };

        let mut panel = Panel::new(self.id, self.title)
            .grow(true)
            .collapsed(self.collapsed)
            .on_toggle_opt(self.on_toggle)
            .child(body);
        if let Some(glyph) = self.glyph {
            panel = panel.glyph(glyph);
        }
        panel
    }
}

fn scope_row(
    t: &Theme,
    label: std::sync::Arc<str>,
    active: bool,
    samples: std::sync::Arc<[f32]>,
) -> impl IntoElement {
    let line = if active { t.accent } else { t.text_dim };
    let axis = with_alpha(t.text_dim, 0.35);

    let scope = canvas(
        |_bounds, _window, _cx| {},
        move |bounds, _, window, _cx| paint_wave(window, bounds, &samples, line, axis),
    )
    .absolute()
    .size_full();

    div()
        .flex()
        .items_center()
        .gap_1()
        .px_1()
        .h(px(34.))
        .child(
            div()
                .w(px(20.))
                .flex_shrink_0()
                .font_family(t.mono_font)
                .text_xs()
                .text_color(if active { t.accent } else { t.text_dim })
                .child(gpui::SharedString::from(label)),
        )
        .child(
            div()
                .flex_1()
                .min_w(px(0.))
                .h_full()
                .relative()
                .child(scope),
        )
}

fn paint_wave(
    window: &mut Window,
    bounds: Bounds<Pixels>,
    samples: &[f32],
    line: Hsla,
    axis: Hsla,
) {
    let (ox, w, mid, amp) = scope_baseline(window, bounds, axis);

    if samples.len() < 2 {
        return;
    }

    let n = samples.len();
    let col_w = (w / n as f32).max(1.0);
    for (i, &s) in samples.iter().enumerate() {
        let x = ox + w * (i as f32 / n as f32);
        let y = mid - s.clamp(-1.0, 1.0) * amp;
        let (top, bar_h) = if y <= mid {
            (y, mid - y)
        } else {
            (mid, y - mid)
        };

        // I kinda need to use quads not cause they look cool but cause they are GPU accelerated
        // ... but it also looks cool :)
        window.paint_quad(fill(
            Bounds::new(point(px(x), px(top)), size(px(col_w), px(bar_h.max(1.0)))),
            line,
        ));
    }
}
