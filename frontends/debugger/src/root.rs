use cirsium::Theme;
use cirsium::components::title_bar;
use debugger_model::DebugCore;
use gpui::prelude::*;
use gpui::{Context, Entity, SharedString, Window, div};

use crate::app::Debugger;
use crate::cover::ArtQuery;
use crate::logging::LogBuffer;

pub(crate) struct Root {
    logs: LogBuffer,
    debugger: Option<Entity<Debugger>>,
    error: Option<SharedString>,
}

impl Root {
    pub(crate) fn new(
        logs: LogBuffer,
        initial: Option<(Box<dyn DebugCore>, ArtQuery)>,
        cx: &mut Context<Self>,
    ) -> Self {
        let debugger = initial.map(|(target, art)| {
            let debugger = cx.new(|cx| Debugger::new(target, logs.clone(), cx));
            Self::resolve_cover_art(debugger.clone(), art, cx);
            debugger
        });
        Self {
            logs,
            debugger,
            error: None,
        }
    }

    pub(crate) fn load_rom(&mut self, file_name: String, rom: Vec<u8>, cx: &mut Context<Self>) {
        match crate::load_snes_rom(rom, file_name) {
            Ok((target, art)) => {
                let logs = self.logs.clone();
                let debugger = cx.new(|cx| Debugger::new(target, logs, cx));
                Self::resolve_cover_art(debugger.clone(), art, cx);
                self.debugger = Some(debugger);
                self.error = None;
            }
            Err(err) => {
                tracing::error!("failed to load ROM: {err}");
                self.error = Some(format!("Couldn't load ROM: {err}").into());
            }
        }
        cx.notify();
    }

    fn resolve_cover_art(debugger: Entity<Debugger>, art: ArtQuery, cx: &mut Context<Self>) {
        let client = cx.http_client();
        cx.spawn(async move |_this, cx| {
            let Some(cover) = crate::cover::resolve(client, &art).await else {
                return;
            };
            let _ = debugger.update(cx, |debugger, cx| debugger.set_cover_art(cover, cx));
        })
        .detach();
    }

    #[cfg(not(target_family = "wasm"))]
    fn load_rom_from_path(&mut self, path: &std::path::Path, cx: &mut Context<Self>) {
        match std::fs::read(path) {
            Ok(bytes) => self.load_rom(crate::file_name_of(path), bytes, cx),
            Err(err) => {
                self.error = Some(format!("Couldn't read {}: {err}", path.display()).into());
                cx.notify();
            }
        }
    }

    #[cfg(not(target_family = "wasm"))]
    fn on_files_dropped(&mut self, paths: &gpui::ExternalPaths, cx: &mut Context<Self>) {
        let Some(path) = paths.paths().first() else {
            return;
        };
        self.load_rom_from_path(path, cx);
    }

    fn browse_for_rom(&mut self, cx: &mut Context<Self>) {
        #[cfg(target_family = "wasm")]
        {
            let _ = cx;
            crate::web_dnd::open_file_picker();
        }

        #[cfg(not(target_family = "wasm"))]
        {
            let paths = cx.prompt_for_paths(gpui::PathPromptOptions {
                files: true,
                directories: false,
                multiple: false,
                prompt: None,
            });
            cx.spawn(async move |this, cx| {
                let Ok(Ok(Some(paths))) = paths.await else {
                    return;
                };
                let Some(path) = paths.into_iter().next() else {
                    return;
                };
                let _ = this.update(cx, |this, cx| this.load_rom_from_path(&path, cx));
            })
            .detach();
        }
    }

    fn landing(&self, theme: &Theme, cx: &mut Context<Self>) -> impl IntoElement {
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
                    .child("Drop a ROM file here or click to browse"),
            )
            .when_some(self.error.clone(), |col, error| {
                col.child(div().text_color(theme.danger).child(error))
            });

        let drop_zone = div()
            .id("rom-landing")
            .flex_1()
            .w_full()
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .on_click(cx.listener(|this, _event, _window, cx| this.browse_for_rom(cx)))
            .child(column);

        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(theme.bg_root)
            .font_family(theme.ui_font)
            .text_color(theme.text_primary)
            .child(title_bar(theme, "Sly Debugger", div(), div()))
            .child(drop_zone)
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
            None => root.child(self.landing(&theme, cx)),
        }
    }
}
