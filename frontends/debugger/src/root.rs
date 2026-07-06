use cirsium::Theme;
use debugger_model::DebugCore;
use gpui::prelude::*;
use gpui::{Context, Entity, SharedString, Window, div};

use crate::app::Debugger;
use crate::logging::LogBuffer;

pub(crate) struct Root {
    logs: LogBuffer,
    debugger: Option<Entity<Debugger>>,
    error: Option<SharedString>,
}

impl Root {
    pub(crate) fn new(
        logs: LogBuffer,
        initial: Option<Box<dyn DebugCore>>,
        cx: &mut Context<Self>,
    ) -> Self {
        let debugger = initial.map(|target| {
            let logs = logs.clone();
            cx.new(|cx| Debugger::new(target, logs, cx))
        });
        Self {
            logs,
            debugger,
            error: None,
        }
    }

    pub(crate) fn load_rom(&mut self, file_name: String, rom: Vec<u8>, cx: &mut Context<Self>) {
        match crate::load_snes_rom(rom, file_name) {
            Ok(target) => {
                let logs = self.logs.clone();
                self.debugger = Some(cx.new(|cx| Debugger::new(target, logs, cx)));
                self.error = None;
            }
            Err(err) => {
                tracing::error!("failed to load ROM: {err}");
                self.error = Some(format!("Couldn't load ROM: {err}").into());
            }
        }
        cx.notify();
    }

    #[cfg(not(target_family = "wasm"))]
    fn on_files_dropped(&mut self, paths: &gpui::ExternalPaths, cx: &mut Context<Self>) {
        let Some(path) = paths.paths().first() else {
            return;
        };
        match std::fs::read(path) {
            Ok(bytes) => self.load_rom(crate::file_name_of(path), bytes, cx),
            Err(err) => {
                self.error = Some(format!("Couldn't read {}: {err}", path.display()).into());
                cx.notify();
            }
        }
    }

    fn landing(&self, theme: &Theme) -> impl IntoElement {
        let column = div()
            .flex()
            .flex_col()
            .items_center()
            .gap_3()
            .child(
                div()
                    .text_2xl()
                    .text_color(theme.logo)
                    .child("Sly Debugger"),
            )
            .child(
                div()
                    .text_color(theme.text_muted)
                    .child("Drop a ROM file here to start debugging"),
            )
            .when_some(self.error.clone(), |col, error| {
                col.child(div().text_color(theme.danger).child(error))
            });

        div()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .bg(theme.bg_root)
            .font_family(theme.ui_font)
            .text_color(theme.text_primary)
            .child(column)
    }
}

impl Render for Root {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::global(cx);

        let root = div().size_full().bg(theme.bg_root);

        #[cfg(not(target_family = "wasm"))]
        let root = root.on_drop(cx.listener(
            |this, paths: &gpui::ExternalPaths, _window: &mut Window, cx: &mut Context<Self>| {
                this.on_files_dropped(paths, cx);
            },
        ));

        match &self.debugger {
            Some(debugger) => root.child(debugger.clone()),
            None => root.child(self.landing(&theme)),
        }
    }
}
