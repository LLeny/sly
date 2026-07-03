mod app;
mod art;
mod breakpoints;
mod db;
mod disasm;
mod logging;
mod memory;

use std::sync::Arc;

use cirsium::components::register_text_input_keys;
use cirsium::{Assets, Theme, ThemeMode, set_theme};
use gpui::prelude::*;
use gpui::{
    App, Bounds, Point, QuitMode, TitlebarOptions, WindowBounds, WindowDecorations, WindowOptions,
    px, size,
};
use gpui_platform::application;
use reqwest_client::ReqwestClient;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

use app::Debugger;
use art::{AssetProvider, GameRef, LibretroThumbnails, System};
use debugger_model::DebugCore;
use logging::{LogBuffer, LogLayer};

fn main() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("warn"));

    let logs = LogBuffer::new();
    tracing_subscriber::registry()
        .with(filter)
        .with(tracing_subscriber::fmt::layer().without_time())
        .with(LogLayer::new(logs.clone()))
        .init();

    let Some(path) = std::env::args().nth(1) else {
        eprintln!("usage: debugger <rom.sfc>");
        std::process::exit(2);
    };
    let provider = LibretroThumbnails::new();
    let target: Box<dyn DebugCore> = match self::load_snes(&path, &provider) {
        Ok(target) => target,
        Err(err) => {
            eprintln!("failed to load {path}: {err}");
            std::process::exit(1);
        }
    };

    application().with_assets(Assets).run(move |cx: &mut App| {
        if let Err(err) = Assets.load_fonts(cx) {
            eprintln!("font load failed: {err}");
        }

        cx.set_quit_mode(QuitMode::LastWindowClosed);
        register_text_input_keys(cx);

        let http_client = ReqwestClient::user_agent("debugger").unwrap();
        cx.set_http_client(Arc::new(http_client));

        set_theme(cx, Theme::new(ThemeMode::Dark));

        let bounds = Bounds::centered(None, size(px(1600.), px(960.)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some("Sly Debugger".into()),
                    appears_transparent: true,
                    traffic_light_position: Some(Point::new(px(12.), px(10.))),
                }),
                window_decorations: Some(WindowDecorations::Client),
                ..Default::default()
            },
            |_window, cx| cx.new(|cx| Debugger::new(target, logs, cx)),
        )
        .unwrap();
        cx.activate(true);
    });
}

fn load_snes(
    path: &str,
    provider: &dyn AssetProvider,
) -> Result<Box<dyn DebugCore>, Box<dyn std::error::Error>> {
    let rom = std::fs::read(path)?;
    let title = db::title(System::Snes, &rom);

    let file_name = std::path::Path::new(path)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string());

    let name = title.map(str::to_string).unwrap_or_else(|| {
        std::path::Path::new(path)
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| file_name.clone())
    });

    let cart = snes::Cartridge::new(rom)?;
    let cover = provider.cover_art(&GameRef::new(System::Snes, name));

    Ok(Box::new(snes::debug::SnesDebug::new(
        cart, file_name, cover,
    )))
}
