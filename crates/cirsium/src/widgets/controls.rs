use gpui::prelude::*;
use gpui::{App, ClickEvent, Div, SharedString, Stateful, Window, div, px, svg};

use crate::assets::icon;
use crate::components::{ClickHandler, Panel, RowCallback, label_text};
use crate::theme::Theme;

#[derive(Clone)]
pub struct StepTargetView {
    pub id: SharedString,
    pub label: SharedString,
}

#[derive(Clone, Default)]
pub struct ControlsView {
    pub running: bool,
    pub targets: Vec<StepTargetView>,
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
    on_save_state: Option<ClickHandler>,
    on_load_state: Option<ClickHandler>,
    on_run_until: Option<RowCallback>,
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
            on_save_state: None,
            on_load_state: None,
            on_run_until: None,
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

    pub fn on_save_state(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_save_state = Some(Box::new(handler));
        self
    }

    pub fn on_load_state(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_load_state = Some(Box::new(handler));
        self
    }

    pub fn on_run_until(mut self, handler: RowCallback) -> Self {
        self.on_run_until = Some(handler);
        self
    }
}

impl RenderOnce for ControlsWidget {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::global(cx);
        let t = &*theme;

        let buttons = div()
            .flex()
            .items_center()
            .gap_1()
            .child(self::ctrl_button(
                t,
                "ctrl-run",
                icon::RUN,
                self.view.running,
                self.on_run,
            ))
            .child(self::ctrl_button(
                t,
                "ctrl-pause",
                icon::PAUSE,
                !self.view.running,
                self.on_pause,
            ))
            .child(self::ctrl_button(
                t,
                "ctrl-step",
                icon::STEP,
                false,
                self.on_step,
            ))
            .child(self::ctrl_button(
                t,
                "ctrl-reset",
                icon::RESET,
                false,
                self.on_reset,
            ))
            .child(div().w(px(1.)).h(px(18.)).mx_1().bg(t.border_subtle))
            .child(self::ctrl_button(
                t,
                "ctrl-save-state",
                icon::SAVE,
                false,
                self.on_save_state,
            ))
            .child(self::ctrl_button(
                t,
                "ctrl-load-state",
                icon::RESTORE,
                false,
                self.on_load_state,
            ));

        let mut body = div().flex().flex_col().gap_2().child(buttons);
        if !self.view.targets.is_empty() {
            let mut chips = div()
                .flex()
                .items_center()
                .flex_wrap()
                .gap_1()
                .child(label_text(t, "UNTIL"));
            for target in &self.view.targets {
                chips = chips.child(self::until_chip(t, target, self.on_run_until.clone()));
            }
            body = body.child(chips);
        }

        Panel::new(Self::ID, "Controls")
            .glyph(icon::CONTROLS)
            .collapsed(self.collapsed)
            .on_toggle_opt(self.on_toggle)
            .child(body)
    }
}

fn until_chip(
    theme: &Theme,
    target: &StepTargetView,
    handler: Option<RowCallback>,
) -> Stateful<Div> {
    let accent_soft = theme.accent_soft;
    let accent = theme.accent;
    let mut chip = div()
        .id(SharedString::from(format!("until-{}", target.id)))
        .px_1p5()
        .py_0p5()
        .rounded_md()
        .border_1()
        .border_color(theme.border_subtle)
        .bg(theme.bg_panel_alt)
        .text_xs()
        .text_color(theme.text_secondary)
        .hover(move |s| s.bg(accent_soft).border_color(accent))
        .cursor_pointer()
        .child(target.label.clone());

    if let Some(handler) = handler {
        let id = target.id.clone();
        chip = chip.on_click(move |_, window, cx| handler(id.clone(), window, cx));
    }
    chip
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
        theme.on_accent
    } else {
        theme.text_secondary
    };
    let bg = if active {
        theme.accent
    } else {
        theme.bg_panel_alt
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
        .bg(bg)
        .child(svg().path(icon_path).size(px(14.)).text_color(icon_color));

    if enabled && !active {
        let hover_bg = theme.bg_header;
        button = button.hover(move |s| s.bg(hover_bg));
    }
    if let Some(handler) = handler {
        button = button.cursor_pointer().on_click(handler);
    }
    button
}
