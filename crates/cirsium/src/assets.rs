use std::borrow::Cow;

use gpui::{App, AssetSource, Result, SharedString};
use rust_embed::RustEmbed;

pub mod icon {
    pub const CHEVRON_DOWN: &str = "icons/lucide/chevron-down.svg";
    pub const CHEVRON_RIGHT: &str = "icons/lucide/chevron-right.svg";
    pub const THEME: &str = "icons/lucide/sun-moon.svg";
    pub const SETTINGS: &str = "icons/lucide/sliders-horizontal.svg";
    pub const MINIMIZE: &str = "icons/titlebar-minimize.svg";
    pub const MAXIMIZE: &str = "icons/titlebar-maximize.svg";
    pub const CLOSE: &str = "icons/titlebar-close.svg";
    pub const CPU: &str = "icons/lucide/cpu.svg";
    pub const CARTRIDGE: &str = "icons/lucide/memory-stick.svg";
    pub const CHECK: &str = "icons/lucide/check.svg";
    pub const CODE: &str = "icons/lucide/code.svg";
    pub const PLUS: &str = "icons/lucide/plus.svg";
    pub const TRASH: &str = "icons/lucide/trash-2.svg";
    pub const CONTROLS: &str = "icons/lucide/circle-play.svg";
    pub const BREAKPOINT: &str = "icons/lucide/bug.svg";
    pub const LOG: &str = "icons/lucide/scroll-text.svg";
    pub const LIST: &str = "icons/lucide/list.svg";
    pub const GRAPH: &str = "icons/lucide/workflow.svg";
    pub const PALETTE: &str = "icons/lucide/palette.svg";
    pub const MONITOR: &str = "icons/lucide/monitor.svg";
    pub const GRID: &str = "icons/lucide/grid-3x3.svg";
    pub const MAP: &str = "icons/lucide/map.svg";
    pub const GHOST: &str = "icons/lucide/ghost.svg";
    pub const AUDIO: &str = "icons/lucide/audio-lines.svg";
    pub const DOWNLOAD: &str = "icons/lucide/download.svg";
    pub const LINK: &str = "icons/lucide/link-2.svg";

    pub const SAVE: &str = "icons/lucide/save.svg";
    pub const RESTORE: &str = "icons/lucide/history.svg";
    pub const BOOKMARK: &str = "icons/lucide/bookmark.svg";

    pub const RUN: &str = "icons/lucide/play.svg";
    pub const PAUSE: &str = "icons/lucide/pause.svg";
    pub const STEP: &str = "icons/lucide/step-forward.svg";
    pub const RESET: &str = "icons/lucide/rotate-ccw.svg";
}

#[derive(RustEmbed)]
#[folder = "assets"]
struct Embedded;

#[derive(RustEmbed)]
#[folder = "../../vendor"]
struct Vendored;

pub struct Assets;

impl Assets {
    pub fn load_fonts(&self, cx: &App) -> Result<()> {
        let mut fonts = Vec::new();
        for path in self.list("fonts")? {
            if path.ends_with(".ttf")
                && let Some(bytes) = self.load(&path)?
            {
                fonts.push(bytes);
            }
        }
        cx.text_system().add_fonts(fonts)
    }
}

impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        Ok(Embedded::get(path)
            .or_else(|| Vendored::get(path))
            .map(|file| file.data))
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        Ok(Embedded::iter()
            .chain(Vendored::iter())
            .filter(|p| p.starts_with(path))
            .map(|p| SharedString::from(p.into_owned()))
            .collect())
    }
}
