use gpui::prelude::*;
use gpui::{App, ClickEvent, Div, Stateful, Window, div, px, svg};

use crate::assets::icon;
use crate::components::{ClickHandler, Panel};
use crate::theme::Theme;

#[derive(Clone, Default)]
pub struct ControlsView {
    pub running: bool,
}

#[derive(IntoElement)]
pub struct ControlsWidget {
    view: ControlsView,
    collapsed: bool,
    on_toggle: Option<ClickHandler>,
    on_run: Option<ClickHandler>,
    on_pause: Option<ClickHandler>,
    on_step: Option<ClickHandler>,
    on_reset: Option<ClickHandler>,
}

impl ControlsWidget {
    pub const ID: &'static str = "controls";

    pub fn new(view: ControlsView) -> Self {
        Self {
            view,
            collapsed: false,
            on_toggle: None,
            on_run: None,
            on_pause: None,
            on_step: None,
            on_reset: None,
        }
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

    pub fn on_run(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_run = Some(Box::new(handler));
        self
    }

    pub fn on_pause(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_pause = Some(Box::new(handler));
        self
    }

    pub fn on_step(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_step = Some(Box::new(handler));
        self
    }

    pub fn on_reset(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_reset = Some(Box::new(handler));
        self
    }
}

impl RenderOnce for ControlsWidget {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::global(cx);
        let t = &*theme;
        let running = self.view.running;

        let buttons = div()
            .flex()
            .items_center()
            .gap_1()
            .child(ctrl_button(t, "ctrl-run", icon::RUN, running, self.on_run))
            .child(ctrl_button(
                t,
                "ctrl-pause",
                icon::PAUSE,
                !running,
                self.on_pause,
            ))
            .child(ctrl_button(t, "ctrl-step", icon::STEP, false, self.on_step))
            .child(ctrl_button(
                t,
                "ctrl-reset",
                icon::RESET,
                false,
                self.on_reset,
            ));

        let body = div().flex().flex_col().gap_2().child(buttons);

        Panel::new(Self::ID, "Controls")
            .glyph(icon::CONTROLS)
            .collapsed(self.collapsed)
            .on_toggle_opt(self.on_toggle)
            .child(body)
    }
}

fn ctrl_button(
    theme: &Theme,
    id: &'static str,
    icon_path: &'static str,
    active: bool,
    handler: Option<ClickHandler>,
) -> Stateful<Div> {
    let enabled = handler.is_some();
    let icon_color = if !enabled {
        theme.text_dim
    } else if active {
        theme.accent
    } else {
        theme.text_secondary
    };
    let border = if active {
        theme.accent
    } else {
        theme.border_subtle
    };

    let mut button = div()
        .id(id)
        .flex()
        .items_center()
        .justify_center()
        .size(px(28.))
        .rounded_md()
        .border_1()
        .border_color(border)
        .when(active, |d| d.bg(theme.accent_soft))
        .child(svg().path(icon_path).size(px(14.)).text_color(icon_color));

    if let Some(handler) = handler {
        let hover_bg = theme.bg_panel_alt;
        button = button
            .cursor_pointer()
            .hover(move |s| s.bg(hover_bg))
            .on_click(handler);
    }
    button
}
