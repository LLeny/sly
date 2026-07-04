use std::borrow::Cow;

use gpui::{App, AssetSource, Result, SharedString};
use rust_embed::RustEmbed;

pub mod icon {
    pub const CHEVRON_DOWN: &str = "icons/chevron-down.svg";
    pub const CHEVRON_RIGHT: &str = "icons/chevron-right.svg";
    pub const THEME: &str = "icons/sun-moon.svg";
    pub const SETTINGS: &str = "icons/settings.svg";
    pub const MINIMIZE: &str = "icons/minus.svg";
    pub const MAXIMIZE: &str = "icons/square.svg";
    pub const CLOSE: &str = "icons/x.svg";
    pub const LOGO: &str = "icons/raccoon.svg";
    pub const CPU: &str = "icons/cpu.svg";
    pub const CARTRIDGE: &str = "icons/memory-stick.svg";
    pub const CHECK: &str = "icons/check.svg";
    pub const CODE: &str = "icons/code.svg";
    pub const PLUS: &str = "icons/plus.svg";
    pub const TRASH: &str = "icons/trash-2.svg";
    pub const CONTROLS: &str = "icons/circle-play.svg";
    pub const BREAKPOINT: &str = "icons/bug.svg";
    pub const LOG: &str = "icons/scroll-text.svg";
    pub const LIST: &str = "icons/list.svg";
    pub const GRAPH: &str = "icons/workflow.svg";
    pub const PALETTE: &str = "icons/palette.svg";
    pub const MONITOR: &str = "icons/monitor.svg";
    pub const GRID: &str = "icons/grid-3x3.svg";

    pub const RUN: &str = "icons/play.svg";
    pub const PAUSE: &str = "icons/pause.svg";
    pub const STEP: &str = "icons/step.svg";
    pub const RESET: &str = "icons/reset.svg";
}

#[derive(RustEmbed)]
#[folder = "assets"]
struct Embedded;

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
        Ok(Embedded::get(path).map(|file| file.data))
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        Ok(Embedded::iter()
            .filter(|p| p.starts_with(path))
            .map(|p| SharedString::from(p.into_owned()))
            .collect())
    }
}
