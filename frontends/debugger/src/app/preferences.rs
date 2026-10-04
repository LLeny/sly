use super::*;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
#[serde(default)]
struct Preferences {
    follow_pc: bool,
    show_bytes: bool,
    live_refresh: bool,
    show_log: bool,
    deduplicate_logs: bool,
    accent: usize,
    light_theme: bool,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            follow_pc: true,
            show_bytes: false,
            live_refresh: true,
            show_log: false,
            deduplicate_logs: false,
            accent: 0,
            light_theme: false,
        }
    }
}

impl Debugger {
    pub(super) fn load_preferences(&mut self, cx: &mut Context<Self>) {
        let p = match settings::frontend::<Preferences>("debugger") {
            Ok(p) => p,
            Err(error) => {
                tracing::error!("{error}");
                return;
            }
        };

        for pane in &mut self.panes {
            if pane.disasm.follow_pc() != p.follow_pc {
                pane.disasm.toggle_follow(self.target.as_ref());
            }

            if pane.disasm.show_bytes() != p.show_bytes {
                pane.disasm.toggle_bytes();
            }
        }

        if self.logs.dedup() != p.deduplicate_logs {
            self.logs.toggle_dedup();
        }

        self.live_refresh = p.live_refresh;
        self.show_log = p.show_log;
        self.accent = p.accent.min(ACCENT_PRESETS.len() + GUEST_COLORS.len() - 1);

        let mode = if p.light_theme {
            ThemeMode::Light
        } else {
            ThemeMode::Dark
        };

        set_theme(cx, render::themed(mode, self.accent));
    }

    pub(super) fn save_preferences(&mut self, cx: &mut Context<Self>) {
        let p = Preferences {
            follow_pc: self.panes.first().is_none_or(|p| p.disasm.follow_pc()),
            show_bytes: self.panes.first().is_some_and(|p| p.disasm.show_bytes()),
            live_refresh: self.live_refresh,
            show_log: self.show_log,
            deduplicate_logs: self.logs.dedup(),
            accent: self.accent,
            light_theme: Theme::global(cx).mode == ThemeMode::Light,
        };

        cx.spawn(async move |_, _| {
            if let Err(error) = settings::save_frontend_async("debugger", p).await {
                tracing::error!("{error}");
            }
        })
        .detach();
    }

    pub(super) fn save_artwork_key(&mut self, cx: &mut Context<Self>) {
        let key = self.artwork_key_input.read(cx).content().to_owned();

        cx.spawn(async move |this, cx| {
            if let Err(error) = settings::set_api_key_async(key.clone()).await {
                tracing::error!("{error}");
                return;
            }

            let _ = this.update(cx, |this, cx| {
                this.artwork_key_input
                    .update(cx, |input, cx| input.set_content(key.trim(), cx));

                if let Some(query) = this.art_query.clone() {
                    this.resolve_cover_art(query, cx);
                }
            });
        })
        .detach();
    }
}
