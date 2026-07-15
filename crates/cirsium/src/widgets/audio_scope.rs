use gpui::prelude::*;
use gpui::{App, Bounds, ClickEvent, PathBuilder, Pixels, Window, canvas, div, point, px};

use crate::components::{ClickHandler, Panel, empty_state_centered};
use crate::theme::{Theme, with_alpha};
use crate::widgets::scope::scope_baseline;

pub use debugger_model::AudioScopeView;

#[derive(IntoElement)]
pub struct AudioScopeWidget {
    id: gpui::SharedString,
    title: gpui::SharedString,
    view: AudioScopeView,
    glyph: Option<gpui::SharedString>,
    collapsed: bool,
    on_toggle: Option<ClickHandler>,
}

impl AudioScopeWidget {
    pub fn new(
        id: impl Into<gpui::SharedString>,
        title: impl Into<gpui::SharedString>,
        view: AudioScopeView,
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

impl RenderOnce for AudioScopeWidget {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::global(cx);
        let t = &*theme;

        let peak = self
            .view
            .samples
            .iter()
            .fold(0.0f32, |m, s| m.max(s.abs()))
            .clamp(0.0, 1.0);

        let body = if self.view.samples.len() < 2 {
            empty_state_centered(t, "No audio")
        } else {
            let samples = self.view.samples;
            let line = t.accent;
            let axis = with_alpha(t.text_dim, 0.4);
            let scope = canvas(
                |_bounds, _window, _cx| {},
                move |bounds, _, window, _cx| paint_scope(window, bounds, &samples, line, axis),
            )
            .absolute()
            .size_full();

            div().flex().flex_1().min_h(px(64.)).relative().child(scope)
        };

        let mut panel = Panel::new(self.id, self.title)
            .meta(format!("peak {:.0}%", peak * 100.0))
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

fn paint_scope(
    window: &mut Window,
    bounds: Bounds<Pixels>,
    samples: &[f32],
    line: gpui::Hsla,
    axis: gpui::Hsla,
) {
    let (ox, w, mid, amp) = scope_baseline(window, bounds, axis);

    let n = samples.len();
    if n < 2 {
        return;
    }

    let mut wave = PathBuilder::stroke(px(1.0));
    for (i, &s) in samples.iter().enumerate() {
        let x = ox + w * (i as f32 / (n - 1) as f32);
        let y = mid - s.clamp(-1.0, 1.0) * amp;
        let p = point(px(x), px(y));

        if i == 0 {
            wave.move_to(p);
        } else {
            wave.line_to(p);
        }
    }

    if let Ok(path) = wave.build() {
        window.paint_path(path, line);
    }
}
