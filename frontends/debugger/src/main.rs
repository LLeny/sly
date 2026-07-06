#![cfg_attr(target_family = "wasm", no_main)]

mod app;
mod art;
mod breakpoints;
mod db;
mod disasm;
mod logging;
mod memory;
mod root;
#[cfg(target_family = "wasm")]
mod web_dnd;

#[cfg(not(target_family = "wasm"))]
use std::sync::Arc;

use cirsium::components::register_text_input_keys;
use cirsium::{Assets, Theme, ThemeMode, set_theme};
use gpui::prelude::*;
use gpui::{App, Bounds, WindowBounds, WindowOptions, px, size};
use tracing_subscriber::EnvFilter;
use tracing_subscriber::layer::SubscriberExt;

use debugger_model::DebugCore;
use logging::{LogBuffer, LogLayer};
use root::Root;

fn init_logging() -> LogBuffer {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("warn"));
    let logs = LogBuffer::new();

    let registry = tracing_subscriber::registry()
        .with(filter)
        .with(LogLayer::new(logs.clone()));

    #[cfg(not(target_family = "wasm"))]
    {
        use tracing_subscriber::util::SubscriberInitExt;
        registry
            .with(tracing_subscriber::fmt::layer().without_time())
            .init();
    }

    #[cfg(target_family = "wasm")]
    {
        let _ = tracing::subscriber::set_global_default(registry);
    }

    logs
}

pub(crate) fn load_snes_rom(
    rom: Vec<u8>,
    file_name: String,
) -> Result<Box<dyn DebugCore>, Box<dyn std::error::Error>> {
    use art::{AssetProvider, GameRef, LibretroThumbnails, System};

    let name = db::title(System::Snes, &rom)
        .map(str::to_string)
        .unwrap_or_else(|| {
            std::path::Path::new(&file_name)
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| file_name.clone())
        });

    let cart = snes::Cartridge::new(rom)?;
    let cover = LibretroThumbnails::new().cover_art(&GameRef::new(System::Snes, name));
    Ok(Box::new(snes::debug::SnesDebug::new(
        cart, file_name, cover,
    )))
}

#[cfg(not(target_family = "wasm"))]
fn file_name_of(path: &std::path::Path) -> String {
    path.file_name()
        .unwrap_or(path.as_os_str())
        .to_string_lossy()
        .into_owned()
}

fn run(initial: Option<Box<dyn DebugCore>>, logs: LogBuffer) {
    #[cfg(target_family = "wasm")]
    let app = gpui_platform::single_threaded_web();
    #[cfg(not(target_family = "wasm"))]
    let app = gpui_platform::application();

    app.with_assets(Assets).run(move |cx: &mut App| {
        if let Err(err) = Assets.load_fonts(cx) {
            tracing::error!("font load failed: {err}");
        }

        cx.set_quit_mode(gpui::QuitMode::LastWindowClosed);
        register_text_input_keys(cx);

        #[cfg(not(target_family = "wasm"))]
        cx.set_http_client(Arc::new(
            reqwest_client::ReqwestClient::user_agent("debugger").unwrap(),
        ));

        set_theme(cx, Theme::new(ThemeMode::Dark));

        let bounds = Bounds::centered(None, size(px(1600.), px(960.)), cx);
        let _window = cx
            .open_window(window_options(bounds), |_window, cx| {
                cx.new(|cx| Root::new(logs, initial, cx))
            })
            .unwrap();
        cx.activate(true);

        #[cfg(target_family = "wasm")]
        web_dnd::install(_window, cx);
    });
}

fn window_options(bounds: Bounds<gpui::Pixels>) -> WindowOptions {
    WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        #[cfg(not(target_family = "wasm"))]
        titlebar: Some(gpui::TitlebarOptions {
            title: Some("Sly Debugger".into()),
            appears_transparent: true,
            traffic_light_position: Some(gpui::Point::new(px(12.), px(10.))),
        }),
        #[cfg(not(target_family = "wasm"))]
        window_decorations: Some(gpui::WindowDecorations::Client),
        ..Default::default()
    }
}

// ---------------------------------------------------------------------------
// Native entrypoint
// ---------------------------------------------------------------------------

#[cfg(not(target_family = "wasm"))]
fn main() {
    let logs = init_logging();

    let initial = match std::env::args().nth(1) {
        Some(path) => match load_snes(&path) {
            Ok(target) => Some(target),
            Err(err) => {
                eprintln!("failed to load {path}: {err}");
                std::process::exit(1);
            }
        },
        None => None,
    };

    run(initial, logs);
}

#[cfg(not(target_family = "wasm"))]
fn load_snes(path: &str) -> Result<Box<dyn DebugCore>, Box<dyn std::error::Error>> {
    let rom = std::fs::read(path)?;
    load_snes_rom(rom, file_name_of(std::path::Path::new(path)))
}

// ---------------------------------------------------------------------------
// Web entrypoint
// ---------------------------------------------------------------------------

#[cfg(target_family = "wasm")]
#[wasm_bindgen::prelude::wasm_bindgen(start)]
pub fn start() {
    gpui_platform::web_init();
    let logs = init_logging();
    run(None, logs);
}
