mod app;
mod art;
mod breakpoints;
mod cores;
mod db;
mod disasm;
mod logging;
mod memory;
mod processor;
mod target;

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
use art::LibretroThumbnails;
use cores::SnesTarget;
use logging::{LogBuffer, LogLayer};
use target::DebugTarget;

fn main() {
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("warn,snes=trace"));

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
    let target: Box<dyn DebugTarget> = match SnesTarget::load(&path, &provider) {
        Ok(target) => Box::new(target),
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
