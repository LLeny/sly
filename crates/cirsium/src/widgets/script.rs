use std::sync::Arc;

use debugger_model::{BUILTIN_GROUPS, ConsoleLevel, ScriptStatus};
use gpui::prelude::*;
use gpui::{
    AnyElement, App, ClickEvent, FontWeight, Hsla, SharedString, UniformListScrollHandle, Window,
    div, px, uniform_list,
};

use crate::assets::icon;
use crate::components::{
    ClickHandler, PanelChrome, badge, empty_state, field_button, field_text_button,
};
use crate::theme::Theme;

const ROW_HEIGHT: f32 = 17.;
const CONSOLE_ROWS: f32 = 7.;
const CONSOLE_HEIGHT: f32 = ROW_HEIGHT * CONSOLE_ROWS;

fn status_color(status: ScriptStatus, theme: &Theme) -> Hsla {
    match status {
        ScriptStatus::Idle => theme.text_muted,
        ScriptStatus::Active => theme.success,
        ScriptStatus::Failed => theme.danger,
    }
}

#[derive(IntoElement)]
pub struct ScriptEditorWidget {
    chrome: PanelChrome,
    editor: Option<AnyElement>,
    status: ScriptStatus,
    status_line: Option<SharedString>,
    error: Option<SharedString>,
    hooks: usize,
    dirty: bool,
    file_name: SharedString,
    on_run: Option<ClickHandler>,
    on_stop: Option<ClickHandler>,
    on_revert: Option<ClickHandler>,
    on_save: Option<ClickHandler>,
    on_save_as: Option<ClickHandler>,
    on_open: Option<ClickHandler>,
}

impl ScriptEditorWidget {
    pub fn new(chrome: PanelChrome) -> Self {
        Self {
            chrome,
            editor: None,
            status: ScriptStatus::Idle,
            status_line: None,
            error: None,
            hooks: 0,
            dirty: false,
            file_name: "script.lua".into(),
            on_run: None,
            on_stop: None,
            on_revert: None,
            on_save: None,
            on_save_as: None,
            on_open: None,
        }
    }

    pub fn editor(mut self, editor: impl IntoElement) -> Self {
        self.editor = Some(editor.into_any_element());
        self
    }

    pub fn status(mut self, status: ScriptStatus) -> Self {
        self.status = status;
        self
    }

    pub fn status_line(mut self, line: Option<SharedString>) -> Self {
        self.status_line = line;
        self
    }

    pub fn error(mut self, error: Option<SharedString>) -> Self {
        self.error = error;
        self
    }

    pub fn hooks(mut self, hooks: usize) -> Self {
        self.hooks = hooks;
        self
    }

    pub fn dirty(mut self, dirty: bool) -> Self {
        self.dirty = dirty;
        self
    }

    pub fn file_name(mut self, file_name: impl Into<SharedString>) -> Self {
        self.file_name = file_name.into();
        self
    }

    pub fn on_run(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_run = Some(Box::new(handler));
        self
    }

    pub fn on_stop(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_stop = Some(Box::new(handler));
        self
    }

    pub fn on_revert(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_revert = Some(Box::new(handler));
        self
    }

    pub fn on_save(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_save = Some(Box::new(handler));
        self
    }

    pub fn on_save_as(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_save_as = Some(Box::new(handler));
        self
    }

    pub fn on_open(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_open = Some(Box::new(handler));
        self
    }
}

impl RenderOnce for ScriptEditorWidget {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::global(cx);
        let t = &*theme;

        let mut toolbar = div().flex().items_center().gap_1().pb_2();
        if let Some(run) = self.on_run {
            toolbar = toolbar.child(field_button(t, "script-run", icon::RUN, run));
        }
        if let Some(stop) = self.on_stop {
            toolbar = toolbar.child(field_button(t, "script-stop", icon::PAUSE, stop));
        }
        if let Some(revert) = self.on_revert {
            toolbar = toolbar.child(field_button(t, "script-revert", icon::RESET, revert));
        }
        if let Some(open) = self.on_open {
            toolbar = toolbar.child(field_button(t, "script-open", icon::OPEN, open));
        }
        if let Some(save) = self.on_save {
            toolbar = toolbar.child(field_text_button(
                t,
                "script-save",
                icon::SAVE,
                "Save",
                save,
            ));
        }
        if let Some(save_as) = self.on_save_as {
            toolbar = toolbar.child(field_text_button(
                t,
                "script-save-as",
                icon::DOWNLOAD,
                "Save As",
                save_as,
            ));
        }

        let header_badges = div()
            .flex()
            .items_center()
            .gap_1()
            .child(badge(self.status.label(), status_color(self.status, t)))
            .when(self.hooks > 0, |bar| {
                bar.child(badge(format!("{} hooks", self.hooks), t.info))
            })
            .when(self.dirty, |bar| bar.child(badge("edited", t.warning)));

        let message = self.error.clone().or(self.status_line.clone());
        let color = if self.error.is_some() {
            t.danger
        } else {
            t.text_secondary
        };
        toolbar = toolbar.child(
            div()
                .flex_1()
                .min_w(px(0.))
                .flex()
                .justify_end()
                .font_family(t.mono_font)
                .text_xs()
                .text_color(color)
                .truncate()
                .children(message),
        );

        let editor = div()
            .relative()
            .flex()
            .flex_1()
            .min_h(px(0.))
            .children(self.editor)
            .child(
                badge(self.file_name, t.accent)
                    .absolute()
                    .top(px(6.))
                    .right(px(6.))
                    .max_w(px(240.))
                    .truncate(),
            );

        let body = div()
            .flex()
            .flex_col()
            .flex_1()
            .min_h(px(0.))
            .child(toolbar)
            .child(editor);

        self.chrome
            .panel()
            .glyph(icon::CODE)
            .header_action(header_badges)
            .grow(true)
            .child(body)
    }
}

#[derive(Clone)]
pub struct ConsoleEntry {
    pub level: ConsoleLevel,
    pub text: SharedString,
}

impl From<debugger_model::ConsoleLine> for ConsoleEntry {
    fn from(line: debugger_model::ConsoleLine) -> Self {
        Self {
            level: line.level,
            text: SharedString::from(line.text),
        }
    }
}

#[derive(Clone, Default)]
pub struct ScriptConsoleView {
    pub entries: Vec<ConsoleEntry>,
}

#[derive(IntoElement)]
pub struct ScriptConsoleWidget {
    chrome: PanelChrome,
    view: Arc<ScriptConsoleView>,
    scroll: UniformListScrollHandle,
    on_clear: Option<ClickHandler>,
}

impl ScriptConsoleWidget {
    pub fn new(
        chrome: PanelChrome,
        view: Arc<ScriptConsoleView>,
        scroll: UniformListScrollHandle,
    ) -> Self {
        Self {
            chrome,
            view,
            scroll,
            on_clear: None,
        }
    }

    pub fn on_clear(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_clear = Some(Box::new(handler));
        self
    }
}

impl RenderOnce for ScriptConsoleWidget {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::global(cx);
        let t = &*theme;
        let count = self.view.entries.len();

        let list = if count == 0 {
            div()
                .h(px(CONSOLE_HEIGHT))
                .child(empty_state(t, "No script output"))
                .into_any_element()
        } else {
            let view = self.view.clone();
            uniform_list("script-console", count, move |range, _window, cx| {
                let theme = Theme::global(cx);
                range
                    .map(|ix| self::console_row(&theme, ix, &view.entries[ix]))
                    .collect::<Vec<_>>()
            })
            .track_scroll(&self.scroll)
            .h(px(CONSOLE_HEIGHT))
            .into_any_element()
        };

        let mut panel = self
            .chrome
            .panel()
            .glyph(icon::LOG)
            .meta(format!("{count}"));

        if let Some(clear) = self.on_clear {
            panel =
                panel.header_action(field_button(t, "script-console-clear", icon::TRASH, clear));
        }

        panel.child(div().flex().flex_col().child(list))
    }
}

fn console_row(theme: &Theme, ix: usize, entry: &ConsoleEntry) -> gpui::Stateful<gpui::Div> {
    let hover_bg = theme.bg_header;
    div()
        .id(("script-console-row", ix))
        .flex()
        .items_center()
        .w_full()
        .h(px(ROW_HEIGHT))
        .px_1()
        .font_family(theme.mono_font)
        .text_xs()
        .when(ix % 2 == 1, |row| row.bg(theme.bg_panel_alt))
        .hover(move |row| row.bg(hover_bg))
        .child(
            div()
                .flex_1()
                .min_w(px(0.))
                .text_color(self::console_color(entry.level, theme))
                .truncate()
                .child(entry.text.clone()),
        )
}

type InsertCallback = Box<dyn Fn(&str, &mut Window, &mut App) + 'static>;

#[derive(IntoElement)]
pub struct ScriptBuiltinsWidget {
    chrome: PanelChrome,
    filter: Option<AnyElement>,
    query: SharedString,
    on_insert: Option<InsertCallback>,
}

impl ScriptBuiltinsWidget {
    pub fn new(chrome: PanelChrome) -> Self {
        Self {
            chrome,
            filter: None,
            query: SharedString::default(),
            on_insert: None,
        }
    }

    pub fn filter(mut self, input: impl IntoElement, query: impl Into<SharedString>) -> Self {
        self.filter = Some(input.into_any_element());
        self.query = query.into();
        self
    }

    pub fn on_insert(mut self, handler: impl Fn(&str, &mut Window, &mut App) + 'static) -> Self {
        self.on_insert = Some(Box::new(handler));
        self
    }
}

impl RenderOnce for ScriptBuiltinsWidget {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::global(cx);
        let t = &*theme;
        let query = self.query.to_lowercase();
        let on_insert: Option<Arc<InsertCallback>> = self.on_insert.map(Arc::new);

        let mut body = div().flex().flex_col().gap_1();
        if let Some(filter) = self.filter {
            body = body.child(div().flex().items_center().pb_1().child(filter));
        }

        let mut shown = 0;
        for group in BUILTIN_GROUPS {
            let entries: Vec<_> = group
                .entries
                .iter()
                .filter(|builtin| {
                    query.is_empty()
                        || builtin.name.to_lowercase().contains(&query)
                        || builtin.summary.to_lowercase().contains(&query)
                })
                .collect();

            if entries.is_empty() {
                continue;
            }

            shown += entries.len();
            body = body.child(
                div()
                    .pt_1()
                    .text_xs()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(t.text_muted)
                    .child(group.name),
            );

            for builtin in entries {
                let insert = on_insert.clone();
                let call = self::call_snippet(builtin.signature);
                let hover_bg = t.bg_panel_alt;

                body = body.child(
                    div()
                        .id(SharedString::from(format!("builtin-{}", builtin.name)))
                        .flex()
                        .flex_col()
                        .px_1()
                        .py(px(2.))
                        .rounded_sm()
                        .cursor_pointer()
                        .hover(move |row| row.bg(hover_bg))
                        .when_some(insert, |row, insert| {
                            row.on_click(move |_, window, cx| insert(&call, window, cx))
                        })
                        .child(
                            div()
                                .font_family(t.mono_font)
                                .text_xs()
                                .text_color(t.accent)
                                .child(builtin.signature),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(t.text_muted)
                                .child(builtin.summary),
                        ),
                );
            }
        }

        if shown == 0 {
            body = body.child(empty_state(t, "No builtin matches"));
        }

        self.chrome
            .panel()
            .glyph(icon::LIST)
            .grow(true)
            .meta(format!("{shown}"))
            .child(
                div()
                    .id("script-builtins")
                    .flex_1()
                    .min_h(px(0.))
                    .overflow_y_scroll()
                    .child(body),
            )
    }
}

fn call_snippet(signature: &str) -> String {
    let Some((head, tail)) = signature.split_once('(') else {
        return signature.to_string();
    };

    let mut required = String::with_capacity(tail.len());
    let mut optional_depth = 0usize;
    for ch in tail.trim_end_matches(')').chars() {
        match ch {
            '[' => optional_depth += 1,
            ']' => optional_depth = optional_depth.saturating_sub(1),
            _ if optional_depth == 0 => required.push(ch),
            _ => {}
        }
    }

    let args: Vec<&str> = required
        .trim_end_matches(')')
        .split(',')
        .map(str::trim)
        .filter(|arg| !arg.is_empty())
        .collect();

    format!("{head}({})", args.join(", "))
}

pub fn console_color(level: ConsoleLevel, theme: &Theme) -> Hsla {
    match level {
        ConsoleLevel::Print => theme.text_secondary,
        ConsoleLevel::Notice => theme.text_muted,
        ConsoleLevel::Warn => theme.warning,
        ConsoleLevel::Error => theme.danger,
    }
}
