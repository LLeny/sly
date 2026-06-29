mod art;
mod cores;
mod db;
mod disasm;
mod target;

use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use cirsium::assets::icon;
use cirsium::components::{
    MenuItem, Panel, StatusView, checkbox, context_menu, icon_button, popover, status_bar,
    title_bar,
};
use cirsium::widgets::{
    CartridgeView, CartridgeWidget, ControlsView, ControlsWidget, CpuView, CpuWidget,
    DisassemblyWidget,
};
use cirsium::{Assets, Theme, ThemeMode, set_theme};
use gpui::prelude::*;
use gpui::{
    App, Bounds, ClickEvent, ClipboardItem, Context, MouseDownEvent, Pixels, Point, QuitMode,
    SharedString, Task, TitlebarOptions, Window, WindowBounds, WindowDecorations, WindowOptions,
    div, px, size,
};
use gpui_platform::application;
use reqwest_client::ReqwestClient;

use art::LibretroThumbnails;
use cores::SnesTarget;
use disasm::Disasm;
use target::DebugTarget;

fn main() {
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

        let http_client = ReqwestClient::user_agent("debugger").unwrap();
        cx.set_http_client(Arc::new(http_client));

        set_theme(cx, Theme::new(ThemeMode::Dark));

        let bounds = Bounds::centered(None, size(px(1280.), px(760.)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some("debugger".into()),
                    appears_transparent: true,
                    traffic_light_position: Some(Point::new(px(12.), px(10.))),
                }),
                window_decorations: Some(WindowDecorations::Client),
                ..Default::default()
            },
            |_window, cx| cx.new(|_cx| Debugger::new(target)),
        )
        .unwrap();
        cx.activate(true);
    });
}

struct Debugger {
    target: Box<dyn DebugTarget>,
    cartridge: CartridgeView,
    cpu: CpuView,
    status: StatusView,
    disasm: Disasm,
    collapsed: HashSet<&'static str>,
    settings: Option<Point<Pixels>>,
    running: bool,
    run_task: Option<Task<()>>,
}

impl Debugger {
    fn new(target: Box<dyn DebugTarget>) -> Self {
        let mut disasm = Disasm::new();
        disasm.recompute(target.as_ref());

        Self {
            cartridge: target.cartridge().clone(),
            cpu: target.cpu_view(),
            status: target.status("paused"),
            target,
            disasm,
            collapsed: HashSet::new(),
            settings: None,
            running: false,
            run_task: None,
        }
    }

    fn is_collapsed(&self, id: &'static str) -> bool {
        self.collapsed.contains(id)
    }

    fn toggle(&mut self, id: &'static str, cx: &mut Context<Self>) {
        if !self.collapsed.remove(id) {
            self.collapsed.insert(id);
        }
        cx.notify();
    }

    fn set_running(&mut self, running: bool, cx: &mut Context<Self>) {
        if self.running == running {
            return;
        }

        self.running = running;
        self.status.state = self
            .target
            .caption(if running { "running" } else { "paused" });

        if running {
            self.run_task = Some(cx.spawn(async move |this, cx| {
                loop {
                    let keep_going = this
                        .update(cx, |this, cx| this.run_frame(cx))
                        .unwrap_or(false);
                    if !keep_going {
                        break;
                    }

                    cx.background_executor()
                        .timer(Duration::from_millis(16))
                        .await;
                }
            }));
        } else {
            self.run_task = None;
        }

        cx.notify();
    }

    fn run_frame(&mut self, cx: &mut Context<Self>) -> bool {
        if !self.running {
            return false;
        }

        let ok = self.target.run_frame();

        self.cpu = self.target.cpu_view();
        self.disasm.recompute(self.target.as_ref());
        if ok {
            self.status.state = self.target.caption("running");
        } else {
            self.running = false;
            self.status.state = self.target.caption("unimplemented op");
        }

        cx.notify();
        ok
    }

    fn step(&mut self, cx: &mut Context<Self>) {
        let ok = self.target.step();

        self.cpu = self.target.cpu_view();
        self.status.state = self
            .target
            .caption(if ok { "paused" } else { "unimplemented op" });
        self.running = false;
        self.run_task = None;
        self.disasm.recompute(self.target.as_ref());

        cx.notify();
    }

    fn reset(&mut self, cx: &mut Context<Self>) {
        self.target.reset();

        self.cpu = self.target.cpu_view();
        self.status.state = self.target.caption("reset");
        self.running = false;
        self.run_task = None;
        self.disasm.recompute(self.target.as_ref());

        cx.notify();
    }

    fn set_pc_to(&mut self, addr: u32, cx: &mut Context<Self>) {
        self.disasm.close_menu();
        self.target.set_pc(addr);
        self.cpu = self.target.cpu_view();
        self.status.state = self.target.caption("pc set");
        self.disasm.recompute(self.target.as_ref());
        cx.notify();
    }

    fn disasm_select(&mut self, ix: usize, cx: &mut Context<Self>) {
        self.disasm.select(ix);
        cx.notify();
    }

    fn disasm_open_menu(&mut self, ix: usize, position: Point<Pixels>, cx: &mut Context<Self>) {
        self.disasm.open_menu(ix, position);
        cx.notify();
    }

    fn close_menu(&mut self, cx: &mut Context<Self>) {
        if self.disasm.close_menu() {
            cx.notify();
        }
    }

    fn toggle_follow(&mut self, cx: &mut Context<Self>) {
        self.disasm.toggle_follow(self.target.as_ref());
        cx.notify();
    }

    fn toggle_bytes(&mut self, cx: &mut Context<Self>) {
        self.disasm.toggle_bytes();
        cx.notify();
    }

    fn open_settings(&mut self, position: Point<Pixels>, cx: &mut Context<Self>) {
        self.settings = Some(position);
        cx.notify();
    }

    fn close_settings(&mut self, cx: &mut Context<Self>) {
        if self.settings.take().is_some() {
            cx.notify();
        }
    }
}

fn clipboard_listener(
    cx: &mut Context<Debugger>,
    payload: SharedString,
) -> impl Fn(&ClickEvent, &mut Window, &mut App) + 'static {
    cx.listener(move |this, _, _, cx| {
        cx.write_to_clipboard(ClipboardItem::new_string(payload.to_string()));
        this.close_menu(cx);
    })
}

impl Render for Debugger {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::global(cx);
        let t = &*theme;

        let settings_button = icon_button(
            t,
            "settings",
            t.bg_panel_alt,
            icon::SETTINGS,
            cx.listener(|this, ev: &ClickEvent, _, cx| this.open_settings(ev.position(), cx)),
        );
        let theme_toggle = icon_button(
            t,
            "theme-toggle",
            t.bg_panel_alt,
            icon::THEME,
            cx.listener(|_, _, _, cx| {
                let next = Theme::global(cx).mode.toggled();
                set_theme(cx, Theme::new(next));
                cx.notify();
            }),
        );
        let titlebar_controls = div()
            .flex()
            .items_center()
            .h_full()
            .child(settings_button)
            .child(theme_toggle);

        let cartridge = CartridgeWidget::new(self.cartridge.clone())
            .collapsed(self.is_collapsed(CartridgeWidget::ID))
            .on_toggle(cx.listener(|this, _, _, cx| this.toggle(CartridgeWidget::ID, cx)));

        let cpu = CpuWidget::new(self.cpu.clone())
            .collapsed(self.is_collapsed(CpuWidget::ID))
            .on_toggle(cx.listener(|this, _, _, cx| this.toggle(CpuWidget::ID, cx)));

        let controls_view = ControlsView {
            running: self.running,
            status: self.status.state.clone(),
        };
        let controls = ControlsWidget::new(controls_view)
            .collapsed(self.is_collapsed(ControlsWidget::ID))
            .on_toggle(cx.listener(|this, _, _, cx| this.toggle(ControlsWidget::ID, cx)))
            .on_run(cx.listener(|this, _, _, cx| this.set_running(true, cx)))
            .on_pause(cx.listener(|this, _, _, cx| this.set_running(false, cx)))
            .on_step(cx.listener(|this, _, _, cx| this.step(cx)))
            .on_reset(cx.listener(|this, _, _, cx| this.reset(cx)));

        let this = cx.entity();
        let disassembly =
            DisassemblyWidget::new(self.disasm.view.clone(), self.disasm.scroll.clone())
                .collapsed(self.is_collapsed(DisassemblyWidget::ID))
                .on_toggle(cx.listener(|this, _, _, cx| this.toggle(DisassemblyWidget::ID, cx)))
                .on_select({
                    let this = this.clone();
                    move |ix, _, cx| this.update(cx, |d, cx| d.disasm_select(ix, cx))
                })
                .on_context({
                    let this = this.clone();
                    move |ix, pos, _, cx| this.update(cx, |d, cx| d.disasm_open_menu(ix, pos, cx))
                });

        let disasm_menu = self.disasm.menu.as_ref().map(|menu| {
            let addr = menu.addr;
            let items = vec![
                MenuItem::new("Copy Address", clipboard_listener(cx, menu.address.clone())),
                MenuItem::new("Copy Bytes", clipboard_listener(cx, menu.bytes.clone())),
                MenuItem::new(
                    "Copy Disassembly",
                    clipboard_listener(cx, menu.text.clone()),
                ),
                MenuItem::new(
                    "Set PC to here",
                    cx.listener(move |this, _, _, cx| this.set_pc_to(addr, cx)),
                ),
            ];
            context_menu(
                t,
                menu.position,
                items,
                cx.listener(|this, _: &MouseDownEvent, _, cx| this.close_menu(cx)),
            )
        });

        let settings_popup = self.settings.map(|pos| {
            let content = div()
                .flex()
                .flex_col()
                .gap_2()
                .min_w(px(150.))
                .child(
                    div()
                        .text_xs()
                        .text_color(t.text_muted)
                        .child("DISASSEMBLY"),
                )
                .child(checkbox(
                    t,
                    "set-follow",
                    self.disasm.follow_pc(),
                    "Follow PC",
                    cx.listener(|this, _, _, cx| this.toggle_follow(cx)),
                ))
                .child(checkbox(
                    t,
                    "set-bytes",
                    self.disasm.show_bytes(),
                    "Show bytes",
                    cx.listener(|this, _, _, cx| this.toggle_bytes(cx)),
                ));
            popover(
                t,
                pos,
                content,
                cx.listener(|this, _: &MouseDownEvent, _, cx| this.close_settings(cx)),
            )
        });

        let workspace = Panel::new("workspace", "Workspace")
            .collapsed(self.is_collapsed("workspace"))
            .on_toggle(cx.listener(|this, _, _, cx| this.toggle("workspace", cx)))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .h(px(360.))
                    .items_center()
                    .justify_center()
                    .text_color(t.text_muted)
                    .child(
                        div()
                            .text_color(t.text_secondary)
                            .child("Session workspace"),
                    )
                    .child(
                        div()
                            .text_xs()
                            .child("Viewport, disassembly, memory and watches dock here."),
                    ),
            );

        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(t.bg_root)
            .font_family(t.ui_font)
            .text_color(t.text_primary)
            .text_sm()
            .child(title_bar(t, "codename: idk yet", titlebar_controls))
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h(px(0.))
                    .gap_2()
                    .p_2()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .w(px(360.))
                            .flex_none()
                            .gap_2()
                            .child(controls)
                            .child(cpu)
                            .child(cartridge),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .min_w(px(0.))
                            .min_h(px(0.))
                            .gap_2()
                            .child(disassembly)
                            .child(workspace),
                    ),
            )
            .child(status_bar(t, &self.status))
            .children(disasm_menu)
            .children(settings_popup)
    }
}
