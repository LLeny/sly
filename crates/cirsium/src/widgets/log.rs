use gpui::prelude::*;
use gpui::{
    AnyElement, App, ClickEvent, Hsla, SharedString, Stateful, UniformListScrollHandle, Window,
    div, px, uniform_list,
};

use crate::assets::icon;
use crate::components::{ClickHandler, Panel, combobox_button, field_button};
use crate::theme::Theme;

const ROW_HEIGHT: f32 = 17.;
const MIN_LIST_HEIGHT: f32 = 80.;
const LEVEL_WIDTH: f32 = 42.;
const TARGET_WIDTH: f32 = 120.;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

impl LogLevel {
    pub const ALL: [LogLevel; 5] = [
        LogLevel::Error,
        LogLevel::Warn,
        LogLevel::Info,
        LogLevel::Debug,
        LogLevel::Trace,
    ];

    pub fn label(self) -> &'static str {
        match self {
            LogLevel::Error => "ERROR",
            LogLevel::Warn => "WARN",
            LogLevel::Info => "INFO",
            LogLevel::Debug => "DEBUG",
            LogLevel::Trace => "TRACE",
        }
    }

    pub fn id(self) -> &'static str {
        match self {
            LogLevel::Error => "error",
            LogLevel::Warn => "warn",
            LogLevel::Info => "info",
            LogLevel::Debug => "debug",
            LogLevel::Trace => "trace",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        match id {
            "error" => Some(LogLevel::Error),
            "warn" => Some(LogLevel::Warn),
            "info" => Some(LogLevel::Info),
            "debug" => Some(LogLevel::Debug),
            "trace" => Some(LogLevel::Trace),
            _ => None,
        }
    }

    pub fn rank(self) -> u8 {
        match self {
            LogLevel::Error => 4,
            LogLevel::Warn => 3,
            LogLevel::Info => 2,
            LogLevel::Debug => 1,
            LogLevel::Trace => 0,
        }
    }

    pub fn color(self, theme: &Theme) -> Hsla {
        match self {
            LogLevel::Error => theme.danger,
            LogLevel::Warn => theme.warning,
            LogLevel::Info => theme.info,
            LogLevel::Debug => theme.text_muted,
            LogLevel::Trace => theme.text_dim,
        }
    }
}

#[derive(Clone)]
pub struct LogEntry {
    pub level: LogLevel,
    /// Full tracing target (module path), e.g. `snes::cpu::interpreter`.
    pub target: SharedString,
    pub message: SharedString,
}

#[derive(Clone, Default)]
pub struct LogView {
    pub entries: Vec<LogEntry>,
}

#[derive(IntoElement)]
pub struct LogWidget {
    view: LogView,
    scroll: UniformListScrollHandle,
    target_label: SharedString,
    level_label: SharedString,
    message_input: Option<AnyElement>,
    collapsed: bool,
    on_toggle: Option<ClickHandler>,
    on_clear: Option<ClickHandler>,
    on_target_open: Option<ClickHandler>,
    on_level_open: Option<ClickHandler>,
}

impl LogWidget {
    pub const ID: &'static str = "log";

    pub fn new(view: LogView, scroll: UniformListScrollHandle) -> Self {
        Self {
            view,
            scroll,
            target_label: "All".into(),
            level_label: "All".into(),
            message_input: None,
            collapsed: false,
            on_toggle: None,
            on_clear: None,
            on_target_open: None,
            on_level_open: None,
        }
    }

    pub fn collapsed(mut self, collapsed: bool) -> Self {
        self.collapsed = collapsed;
        self
    }

    pub fn target_label(mut self, label: impl Into<SharedString>) -> Self {
        self.target_label = label.into();
        self
    }

    pub fn level_label(mut self, label: impl Into<SharedString>) -> Self {
        self.level_label = label.into();
        self
    }

    pub fn message_input(mut self, input: impl IntoElement) -> Self {
        self.message_input = Some(input.into_any_element());
        self
    }

    pub fn on_toggle(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_toggle = Some(Box::new(handler));
        self
    }

    pub fn on_clear(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_clear = Some(Box::new(handler));
        self
    }

    pub fn on_target_open(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_target_open = Some(Box::new(handler));
        self
    }

    pub fn on_level_open(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_level_open = Some(Box::new(handler));
        self
    }
}

impl RenderOnce for LogWidget {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::global(cx);
        let t = &*theme;

        let count = self.view.entries.len();
        let entries = self.view.entries;

        let list = if count == 0 {
            div()
                .flex_1()
                .min_h(px(MIN_LIST_HEIGHT))
                .flex()
                .items_center()
                .justify_center()
                .text_xs()
                .text_color(t.text_dim)
                .child("No log output")
                .into_any_element()
        } else {
            uniform_list("log-list", count, move |range, _window, cx| {
                let theme = Theme::global(cx);
                let t = &*theme;
                range
                    .map(|ix| log_row(t, ix, &entries[ix]))
                    .collect::<Vec<_>>()
            })
            .track_scroll(&self.scroll)
            .flex_1()
            .min_h(px(MIN_LIST_HEIGHT))
            .into_any_element()
        };

        let on_target_open = self.on_target_open;
        let target_combo = combobox_button(t, "log-target", self.target_label, move |e, w, cx| {
            if let Some(handler) = on_target_open.as_ref() {
                handler(e, w, cx);
            }
        });

        let on_level_open = self.on_level_open;
        let level_combo = combobox_button(t, "log-level", self.level_label, move |e, w, cx| {
            if let Some(handler) = on_level_open.as_ref() {
                handler(e, w, cx);
            }
        });

        let mut toolbar = div()
            .flex()
            .items_center()
            .gap_1()
            .pb_2()
            .child(target_combo)
            .child(level_combo)
            .children(self.message_input);
        if let Some(clear) = self.on_clear {
            toolbar = toolbar.child(field_button(t, "log-clear", icon::TRASH, clear));
        }

        let body = div().flex().flex_col().h_full().child(toolbar).child(list);

        Panel::new(Self::ID, "Log")
            .glyph(icon::LOG)
            .collapsed(self.collapsed)
            .grow(true)
            .meta(format!("{count}"))
            .on_toggle_opt(self.on_toggle)
            .child(body)
    }
}

pub fn short_target(target: &str) -> String {
    let mut segments: Vec<&str> = target.rsplit("::").take(2).collect();
    segments.reverse();
    segments.join("::")
}

fn log_row(theme: &Theme, ix: usize, entry: &LogEntry) -> Stateful<gpui::Div> {
    let hover_bg = theme.bg_header;
    div()
        .id(("log-row", ix))
        .flex()
        .items_center()
        .gap_2()
        .w_full()
        .h(px(ROW_HEIGHT))
        .px_1()
        .font_family(theme.mono_font)
        .text_xs()
        .when(ix % 2 == 1, |d| d.bg(theme.bg_panel_alt))
        .hover(move |s| s.bg(hover_bg))
        .child(
            div()
                .w(px(LEVEL_WIDTH))
                .flex_none()
                .text_color(entry.level.color(theme))
                .child(entry.level.label()),
        )
        .child(
            div()
                .w(px(TARGET_WIDTH))
                .flex_none()
                .text_color(theme.text_muted)
                .truncate()
                .child(short_target(&entry.target)),
        )
        .child(
            div()
                .flex_1()
                .min_w(px(0.))
                .text_color(theme.text_secondary)
                .truncate()
                .child(entry.message.clone()),
        )
}
