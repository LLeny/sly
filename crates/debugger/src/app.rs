use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use cirsium::assets::icon;
use cirsium::components::{
    CheckboxSize, MenuItem, StatusView, TextInput, TextInputEvent, checkbox, combobox_menu,
    context_menu, field_button, icon_button, popover, status_bar, title_bar,
};
use cirsium::widgets::{
    BreakpointKind, BreakpointsWidget, CartridgeView, CartridgeWidget, ControlsView,
    ControlsWidget, CpuWidget, DisassemblyWidget, LogWidget, MemoryWidget, MmioView, MmioWidget,
    RowProvider, ViewMode,
};
use cirsium::{ChangeTracker, Highlight, Theme, set_theme};
use debugger_model::CpuView;
use frontend_model::DockLocation;
use gpui::prelude::*;
use gpui::{
    AnyElement, App, ClickEvent, ClipboardItem, Context, Entity, MouseDownEvent, Pixels, Point,
    SharedString, Size, Task, Window, div, px,
};

use crate::breakpoints::Breakpoints;
use crate::disasm::Disasm;
use crate::logging::{LogBuffer, Logs};
use crate::memory::{self, Memory};
use crate::processor::CpuId;
use crate::target::{DebugTarget, RunOutcome};

struct ProcessorPane {
    id: CpuId,
    title: SharedString,
    space: SharedString,
    cpu_view: CpuView,
    disasm: Disasm,
    reg_changes: ChangeTracker<Arc<str>>,
}

pub(crate) struct Debugger {
    target: Box<dyn DebugTarget>,
    cartridge: CartridgeView,
    mmio_view: MmioView,
    mmio_expanded: HashSet<SharedString>,
    status: StatusView,
    panes: Vec<ProcessorPane>,
    memory: Memory,
    breakpoints: Breakpoints,
    logs: Logs,
    jump_input: Entity<TextInput>,
    bp_input: Entity<TextInput>,
    log_filter_input: Entity<TextInput>,
    collapsed: HashSet<SharedString>,
    settings: Option<Point<Pixels>>,
    running: bool,
    run_task: Option<Task<()>>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum CpuRefresh {
    Highlight,
    Baseline,
}

impl Debugger {
    pub(crate) fn new(
        target: Box<dyn DebugTarget>,
        log_buffer: LogBuffer,
        cx: &mut Context<Self>,
    ) -> Self {
        let panes: Vec<ProcessorPane> = target
            .cpus()
            .into_iter()
            .map(|desc| {
                let mut pane = ProcessorPane {
                    disasm: Disasm::new(desc.id.clone()),
                    id: desc.id,
                    title: desc.title,
                    space: desc.space,
                    cpu_view: CpuView::default(),
                    reg_changes: ChangeTracker::default(),
                };
                Self::refresh_pane_view(target.as_ref(), &mut pane, CpuRefresh::Baseline);
                pane
            })
            .collect();
        assert!(
            !panes.is_empty(),
            "a DebugTarget must expose at least one processor"
        );

        let jump_input = cx.new(|cx| TextInput::new(cx, "go to...").hex_only(true).max_len(6));
        cx.subscribe(&jump_input, Self::on_jump_event).detach();

        let bp_input = cx.new(|cx| TextInput::new(cx, "address...").hex_only(true).max_len(6));
        cx.subscribe(&bp_input, Self::on_bp_event).detach();

        let log_filter_input = cx.new(|cx| TextInput::new(cx, "filter messages..."));
        cx.subscribe(&log_filter_input, Self::on_log_filter_event)
            .detach();

        Self {
            cartridge: target.cartridge().clone(),
            mmio_view: target.mmio_view(),
            mmio_expanded: HashSet::new(),
            status: target.status("paused"),
            memory: Memory::new(target.as_ref()),
            breakpoints: Breakpoints::default(),
            logs: Logs::new(log_buffer),
            jump_input,
            bp_input,
            log_filter_input,
            target,
            panes,
            collapsed: HashSet::new(),
            settings: None,
            running: false,
            run_task: None,
        }
    }

    fn refresh_pane_view(target: &dyn DebugTarget, pane: &mut ProcessorPane, mode: CpuRefresh) {
        pane.cpu_view = Self::track_changes(target.cpu_view(&pane.id), &mut pane.reg_changes, mode);
        pane.disasm.recompute(target);
    }

    fn refresh_panes(&mut self, mode: CpuRefresh) {
        for pane in &mut self.panes {
            Self::refresh_pane_view(self.target.as_ref(), pane, mode);
        }
        self.mmio_view = self.target.mmio_view();
    }

    fn track_changes(
        mut cpu: CpuView,
        changes: &mut ChangeTracker<Arc<str>>,
        mode: CpuRefresh,
    ) -> CpuView {
        if mode == CpuRefresh::Baseline {
            changes.clear();
        }

        for reg in &mut cpu.registers {
            let (key, value) = (reg.name.clone(), reg.value as u64);
            let highlight = changes.highlight(key, value);
            reg.highlight = match mode {
                CpuRefresh::Highlight => highlight,
                CpuRefresh::Baseline => Highlight::None,
            };
        }
        cpu
    }

    fn is_collapsed(&self, id: &str) -> bool {
        self.collapsed.contains(id)
    }

    fn toggle(&mut self, id: impl Into<SharedString>, cx: &mut Context<Self>) {
        let id = id.into();
        if !self.collapsed.remove(&id) {
            self.collapsed.insert(id);
        }
        cx.notify();
    }

    fn toggle_mmio(&mut self, id: SharedString, cx: &mut Context<Self>) {
        if !self.mmio_expanded.remove(&id) {
            self.mmio_expanded.insert(id);
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

        let outcome = self.target.run_frame(self.breakpoints.marks());

        self.refresh_panes(CpuRefresh::Highlight);
        self.follow_memory();

        let keep_going = outcome == RunOutcome::Done;
        if !keep_going {
            self.running = false;
        }

        self.status.state = self.target.caption(match outcome {
            RunOutcome::Done => "running",
            RunOutcome::Breakpoint => "breakpoint",
            RunOutcome::Fault => "unimplemented op",
        });

        cx.notify();
        keep_going
    }

    fn step(&mut self, cx: &mut Context<Self>) {
        let ok = self.target.step();

        self.refresh_panes(CpuRefresh::Highlight);
        self.status.state = self
            .target
            .caption(if ok { "paused" } else { "unimplemented op" });
        self.running = false;
        self.run_task = None;
        self.follow_memory();

        cx.notify();
    }

    fn reset(&mut self, cx: &mut Context<Self>) {
        self.target.reset();

        self.refresh_panes(CpuRefresh::Baseline);
        self.status.state = self.target.caption("reset");
        self.running = false;
        self.run_task = None;

        cx.notify();
    }

    fn set_pc_to(&mut self, pane: usize, addr: u32, cx: &mut Context<Self>) {
        let id = self.panes[pane].id.clone();
        self.panes[pane].disasm.close_menu();
        self.target.set_pc(&id, addr);
        Self::refresh_pane_view(
            self.target.as_ref(),
            &mut self.panes[pane],
            CpuRefresh::Highlight,
        );
        self.status.state = self.target.caption("pc set");
        cx.notify();
    }

    fn disasm_select(&mut self, pane: usize, ix: usize, cx: &mut Context<Self>) {
        self.panes[pane].disasm.select(ix);
        cx.notify();
    }

    fn disasm_open_menu(
        &mut self,
        pane: usize,
        ix: usize,
        position: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        self.panes[pane].disasm.open_menu(ix, position);
        cx.notify();
    }

    fn disasm_set_view(&mut self, pane: usize, mode: ViewMode, cx: &mut Context<Self>) {
        self.panes[pane]
            .disasm
            .set_view_mode(mode, self.target.as_ref());
        cx.notify();
    }

    fn disasm_pan_begin(&mut self, pane: usize, pos: Point<Pixels>) {
        self.panes[pane].disasm.pan_begin(pos);
    }

    fn disasm_pan_drag(&mut self, pane: usize, pos: Point<Pixels>, cx: &mut Context<Self>) {
        self.panes[pane].disasm.pan_drag(pos);
        cx.notify();
    }

    fn disasm_set_viewport(&mut self, pane: usize, viewport: Size<f32>, cx: &mut Context<Self>) {
        if self.panes[pane].disasm.set_graph_viewport(viewport) {
            cx.notify();
        }
    }

    fn close_menu(&mut self, cx: &mut Context<Self>) {
        let mut changed = false;
        for pane in &mut self.panes {
            changed |= pane.disasm.close_menu();
        }
        if changed {
            cx.notify();
        }
    }

    fn memory_set_region(&mut self, id: SharedString, cx: &mut Context<Self>) {
        self.memory.set_region(&id);
        cx.notify();
    }

    fn memory_open_combo(&mut self, position: Point<Pixels>, cx: &mut Context<Self>) {
        self.memory.open_combo(position);
        cx.notify();
    }

    fn memory_close_combo(&mut self, cx: &mut Context<Self>) {
        if self.memory.close_combo() {
            cx.notify();
        }
    }

    fn memory_byte_down(&mut self, addr: u32, shift: bool, cx: &mut Context<Self>) {
        self.memory.begin(addr, shift);
        cx.notify();
    }

    fn memory_byte_drag(&mut self, addr: u32, cx: &mut Context<Self>) {
        if self.memory.drag(addr) {
            cx.notify();
        }
    }

    fn memory_open_menu(&mut self, addr: u32, position: Point<Pixels>, cx: &mut Context<Self>) {
        self.memory.open_menu(addr, position, self.target.as_ref());
        cx.notify();
    }

    fn memory_close_menu(&mut self, cx: &mut Context<Self>) {
        if self.memory.close_menu() {
            cx.notify();
        }
    }

    fn memory_toggle_follow(&mut self, cx: &mut Context<Self>) {
        self.memory.toggle_follow();
        self.follow_memory();
        cx.notify();
    }

    fn follow_memory(&mut self) {
        if self.memory.follow_enabled()
            && let Some((region, addr)) = self.target.latest_access()
        {
            self.memory.follow_to(&region, addr);
        }
    }

    fn on_jump_event(
        &mut self,
        _input: Entity<TextInput>,
        event: &TextInputEvent,
        cx: &mut Context<Self>,
    ) {
        if let TextInputEvent::Submit = event {
            self.jump_submit(cx);
        }
    }

    fn jump_submit(&mut self, cx: &mut Context<Self>) {
        let text = self.jump_input.read(cx).content().trim().to_string();
        if let Ok(addr) = u32::from_str_radix(&text, 16) {
            self.memory.jump_to(addr);
        }
        self.jump_input.update(cx, |input, cx| input.clear(cx));
        cx.notify();
    }

    fn on_bp_event(
        &mut self,
        _input: Entity<TextInput>,
        event: &TextInputEvent,
        cx: &mut Context<Self>,
    ) {
        if let TextInputEvent::Submit = event {
            self.bp_add(cx);
        }
    }

    fn bp_add(&mut self, cx: &mut Context<Self>) {
        let text = self.bp_input.read(cx).content().trim().to_string();
        if let Ok(addr) = u32::from_str_radix(&text, 16) {
            let (cpu, region) = self.pane_space(0);
            self.breakpoints
                .add(addr, cpu, region, self.target.as_ref());
            self.bp_input.update(cx, |input, cx| input.clear(cx));
        }
        self.sync_bp_markers();
        cx.notify();
    }

    fn bp_add_at(
        &mut self,
        pane: usize,
        addr: u32,
        len: u32,
        kind: BreakpointKind,
        cx: &mut Context<Self>,
    ) {
        let (cpu, region) = self.pane_space(pane);
        self.panes[pane].disasm.close_menu();
        let end = addr.saturating_add(len.saturating_sub(1));
        self.breakpoints
            .add_kind(addr, end, kind, cpu, region, self.target.as_ref());
        self.sync_bp_markers();
        cx.notify();
    }

    fn bp_add_mem(
        &mut self,
        start: u32,
        end: u32,
        kind: BreakpointKind,
        label: SharedString,
        cx: &mut Context<Self>,
    ) {
        self.memory.close_menu();
        let cpu = self.panes[0].id.clone();
        let region: SharedString = self.memory.region_id().to_string().into();
        let note = self.target.annotate(start);
        self.breakpoints
            .add_labeled(start, end, kind, cpu, region, label, note);
        self.sync_bp_markers();
        cx.notify();
    }

    /// A processor's id and address space, used to tag breakpoints raised from
    /// that pane (or pane 0 for manually-typed and memory-panel breakpoints).
    fn pane_space(&self, pane: usize) -> (CpuId, SharedString) {
        (self.panes[pane].id.clone(), self.panes[pane].space.clone())
    }

    fn bp_remove(&mut self, id: usize, cx: &mut Context<Self>) {
        self.breakpoints.remove(id);
        self.sync_bp_markers();
        cx.notify();
    }

    fn bp_toggle(&mut self, id: usize, cx: &mut Context<Self>) {
        self.breakpoints.toggle(id);
        self.sync_bp_markers();
        cx.notify();
    }

    fn sync_bp_markers(&mut self) {
        let marks = self.breakpoints.marks();
        for pane in &mut self.panes {
            pane.disasm.set_breakpoints(marks);
        }
        let primary = &self.panes[0].id;
        let mem_marks = marks
            .iter()
            .filter(|m| &m.cpu == primary)
            .cloned()
            .collect();
        self.memory.set_breakpoints(mem_marks);
    }

    fn bp_set_kind(&mut self, id: SharedString, cx: &mut Context<Self>) {
        if let Some(kind) = BreakpointKind::from_id(&id) {
            self.breakpoints.set_draft_kind(kind);
        }
        cx.notify();
    }

    fn bp_open_combo(&mut self, position: Point<Pixels>, cx: &mut Context<Self>) {
        self.breakpoints.open_combo(position);
        cx.notify();
    }

    fn bp_close_combo(&mut self, cx: &mut Context<Self>) {
        if self.breakpoints.close_combo() {
            cx.notify();
        }
    }

    fn toggle_follow(&mut self, cx: &mut Context<Self>) {
        for pane in &mut self.panes {
            pane.disasm.toggle_follow(self.target.as_ref());
        }
        cx.notify();
    }

    fn toggle_bytes(&mut self, cx: &mut Context<Self>) {
        for pane in &mut self.panes {
            pane.disasm.toggle_bytes();
        }
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

    fn clear_logs(&mut self, cx: &mut Context<Self>) {
        self.logs.clear();
        cx.notify();
    }

    fn on_log_filter_event(
        &mut self,
        input: Entity<TextInput>,
        _event: &TextInputEvent,
        cx: &mut Context<Self>,
    ) {
        let text = input.read(cx).content().to_string();
        self.logs.set_message(text);
        cx.notify();
    }

    fn log_open_target(&mut self, position: Point<Pixels>, cx: &mut Context<Self>) {
        self.logs.open_target_combo(position);
        cx.notify();
    }

    fn log_close_target(&mut self, cx: &mut Context<Self>) {
        if self.logs.close_target_combo() {
            cx.notify();
        }
    }

    fn log_set_target(&mut self, id: SharedString, cx: &mut Context<Self>) {
        self.logs.set_target(&id);
        cx.notify();
    }

    fn log_open_level(&mut self, position: Point<Pixels>, cx: &mut Context<Self>) {
        self.logs.open_level_combo(position);
        cx.notify();
    }

    fn log_close_level(&mut self, cx: &mut Context<Self>) {
        if self.logs.close_level_combo() {
            cx.notify();
        }
    }

    fn log_set_level(&mut self, id: SharedString, cx: &mut Context<Self>) {
        self.logs.set_level(&id);
        cx.notify();
    }
}

fn clipboard_listener(
    cx: &mut Context<Debugger>,
    payload: SharedString,
    close: fn(&mut Debugger, &mut Context<Debugger>),
) -> impl Fn(&ClickEvent, &mut Window, &mut App) + 'static {
    cx.listener(move |this, _, _, cx| {
        cx.write_to_clipboard(ClipboardItem::new_string(payload.to_string()));
        close(this, cx);
    })
}

impl Render for Debugger {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::global(cx);
        let t = &*theme;

        self.logs.follow_tail();

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

        let controls_view = ControlsView {
            running: self.running,
        };
        let controls = ControlsWidget::new(controls_view)
            .collapsed(self.is_collapsed(ControlsWidget::ID))
            .on_toggle(cx.listener(|this, _, _, cx| this.toggle(ControlsWidget::ID, cx)))
            .on_run(cx.listener(|this, _, _, cx| this.set_running(true, cx)))
            .on_pause(cx.listener(|this, _, _, cx| this.set_running(false, cx)))
            .on_step(cx.listener(|this, _, _, cx| this.step(cx)))
            .on_reset(cx.listener(|this, _, _, cx| this.reset(cx)));

        let this = cx.entity();

        let mmio = MmioWidget::new(self.mmio_view.clone())
            .expanded(self.mmio_expanded.clone())
            .collapsed(self.is_collapsed(MmioWidget::ID))
            .on_toggle(cx.listener(|this, _, _, cx| this.toggle(MmioWidget::ID, cx)))
            .on_toggle_register({
                let this = this.clone();
                move |id, _, cx| this.update(cx, |d, cx| d.toggle_mmio(id, cx))
            });

        let breakpoints =
            BreakpointsWidget::new(self.breakpoints.view(), self.breakpoints.draft_kind())
                .collapsed(self.is_collapsed(BreakpointsWidget::ID))
                .scrollable(true)
                .address_input(self.bp_input.clone())
                .on_toggle(cx.listener(|this, _, _, cx| this.toggle(BreakpointsWidget::ID, cx)))
                .on_add(cx.listener(|this, _, _, cx| this.bp_add(cx)))
                .on_kind_open(
                    cx.listener(|this, ev: &ClickEvent, _, cx| {
                        this.bp_open_combo(ev.position(), cx)
                    }),
                )
                .on_remove({
                    let this = this.clone();
                    move |id, _, cx| this.update(cx, |d, cx| d.bp_remove(id, cx))
                })
                .on_toggle_entry({
                    let this = this.clone();
                    move |id, _, cx| this.update(cx, |d, cx| d.bp_toggle(id, cx))
                });

        let mut cpu_panels: Vec<(String, AnyElement)> = Vec::with_capacity(self.panes.len());
        let mut disasm_panels: Vec<(String, AnyElement)> = Vec::with_capacity(self.panes.len());
        let mut disasm_menus = Vec::new();

        for (pane_ix, pane) in self.panes.iter().enumerate() {
            let cpu_id = SharedString::from(&pane.id);
            let cpu_widget = CpuWidget::new(pane.cpu_view.clone().into())
                .id(cpu_id.clone())
                .title(pane.title.clone())
                .collapsed(self.is_collapsed(&cpu_id))
                .on_toggle({
                    let id = cpu_id.clone();
                    cx.listener(move |this, _, _, cx| this.toggle(id.clone(), cx))
                });
            cpu_panels.push((
                pane.id.as_str().to_string(),
                div().flex_none().child(cpu_widget).into_any_element(),
            ));

            let disasm_id = SharedString::from(format!("{}-disasm", pane.id.as_str()));
            let disasm_title = SharedString::from(format!("{} Disassembly", pane.title));
            let disassembly =
                DisassemblyWidget::new(pane.disasm.view.clone(), pane.disasm.scroll.clone())
                    .id(disasm_id.clone())
                    .title(disasm_title)
                    .collapsed(self.is_collapsed(&disasm_id))
                    .on_toggle({
                        let id = disasm_id.clone();
                        cx.listener(move |this, _, _, cx| this.toggle(id.clone(), cx))
                    })
                    .on_select({
                        let this = this.clone();
                        move |ix, _, cx| this.update(cx, |d, cx| d.disasm_select(pane_ix, ix, cx))
                    })
                    .on_context({
                        let this = this.clone();
                        move |ix, pos, _, cx| {
                            this.update(cx, |d, cx| d.disasm_open_menu(pane_ix, ix, pos, cx))
                        }
                    })
                    .view_mode(pane.disasm.view_mode())
                    .graph_offset(pane.disasm.graph_offset())
                    .on_set_view({
                        let this = this.clone();
                        move |mode, _, cx| {
                            this.update(cx, |d, cx| d.disasm_set_view(pane_ix, mode, cx))
                        }
                    })
                    .on_pan_begin({
                        let this = this.clone();
                        move |pos, _, cx| this.update(cx, |d, _cx| d.disasm_pan_begin(pane_ix, pos))
                    })
                    .on_pan_drag({
                        let this = this.clone();
                        move |pos, _, cx| {
                            this.update(cx, |d, cx| d.disasm_pan_drag(pane_ix, pos, cx))
                        }
                    })
                    .follow(pane.disasm.follow_pc())
                    .graph_viewport(pane.disasm.graph_viewport())
                    .on_viewport({
                        let this = this.clone();
                        move |viewport, _, cx| {
                            this.update(cx, |d, cx| d.disasm_set_viewport(pane_ix, viewport, cx))
                        }
                    });
            disasm_panels.push((
                format!("{}.disasm", pane.id.as_str()),
                disassembly.into_any_element(),
            ));

            if let Some(menu) = pane.disasm.menu.as_ref() {
                let addr = menu.addr;
                let len = menu.len;

                let mut items = vec![
                    MenuItem::new(
                        "Copy Address",
                        clipboard_listener(cx, menu.address.clone(), Debugger::close_menu),
                    ),
                    MenuItem::new(
                        "Copy Bytes",
                        clipboard_listener(cx, menu.bytes.clone(), Debugger::close_menu),
                    ),
                    MenuItem::new(
                        "Copy Disassembly",
                        clipboard_listener(cx, menu.text.clone(), Debugger::close_menu),
                    ),
                    MenuItem::new(
                        "Set PC to here",
                        cx.listener(move |this, _, _, cx| this.set_pc_to(pane_ix, addr, cx)),
                    ),
                    MenuItem::new(
                        "Add execute breakpoint",
                        cx.listener(move |this, _, _, cx| {
                            this.bp_add_at(pane_ix, addr, len, BreakpointKind::Execute, cx)
                        }),
                    ),
                ];

                if pane_ix == 0 {
                    items.push(MenuItem::new(
                        "Add read breakpoint",
                        cx.listener(move |this, _, _, cx| {
                            this.bp_add_at(pane_ix, addr, len, BreakpointKind::Read, cx)
                        }),
                    ));
                    items.push(MenuItem::new(
                        "Add write breakpoint",
                        cx.listener(move |this, _, _, cx| {
                            this.bp_add_at(pane_ix, addr, len, BreakpointKind::Write, cx)
                        }),
                    ));
                }

                disasm_menus.push(context_menu(
                    t,
                    menu.position,
                    items,
                    cx.listener(|this, _: &MouseDownEvent, _, cx| this.close_menu(cx)),
                ));
            }
        }

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
                    self.panes[0].disasm.follow_pc(),
                    "Follow PC",
                    CheckboxSize::Small,
                    cx.listener(|this, _, _, cx| this.toggle_follow(cx)),
                ))
                .child(checkbox(
                    t,
                    "set-bytes",
                    self.panes[0].disasm.show_bytes(),
                    "Show bytes",
                    CheckboxSize::Small,
                    cx.listener(|this, _, _, cx| this.toggle_bytes(cx)),
                ));
            popover(
                t,
                pos,
                content,
                cx.listener(|this, _: &MouseDownEvent, _, cx| this.close_settings(cx)),
            )
        });

        let row_provider: RowProvider = {
            let this = this.clone();
            Rc::new(move |range, cx| {
                let debugger = this.read(cx);
                debugger.memory.rows_for(range, debugger.target.as_ref())
            })
        };

        let jump_button = field_button(
            t,
            "mem-jump-go",
            icon::CHEVRON_RIGHT,
            cx.listener(|this, _, _, cx| this.jump_submit(cx)),
        );
        let jump_field = div()
            .flex()
            .flex_none()
            .items_center()
            .gap_1()
            .w(px(150.))
            .child(self.jump_input.clone())
            .child(jump_button);
        let follow_checkbox = checkbox(
            t,
            "mem-follow",
            self.memory.follow_enabled(),
            "Follow access",
            CheckboxSize::Normal,
            cx.listener(|this, _, _, cx| this.memory_toggle_follow(cx)),
        );
        let memory_header = div()
            .flex()
            .items_center()
            .gap_3()
            .child(follow_checkbox)
            .child(jump_field);
        let memory_widget = MemoryWidget::new(
            row_provider,
            self.memory.row_count(),
            memory::BYTES_PER_ROW,
            self.memory.scroll.clone(),
            self.memory.region_label(),
        )
        .collapsed(self.is_collapsed(MemoryWidget::ID))
        .selection(self.memory.selection())
        .breakpoints(self.memory.breakpoint_ranges())
        .header_extra(memory_header)
        .on_toggle(cx.listener(|this, _, _, cx| this.toggle(MemoryWidget::ID, cx)))
        .on_region_open(
            cx.listener(|this, ev: &ClickEvent, _, cx| this.memory_open_combo(ev.position(), cx)),
        )
        .on_byte_down({
            let this = this.clone();
            move |addr, shift, _, cx| this.update(cx, |d, cx| d.memory_byte_down(addr, shift, cx))
        })
        .on_byte_drag({
            let this = this.clone();
            move |addr, _, cx| this.update(cx, |d, cx| d.memory_byte_drag(addr, cx))
        })
        .on_context({
            let this = this.clone();
            move |addr, pos, _, cx| this.update(cx, |d, cx| d.memory_open_menu(addr, pos, cx))
        });

        let memory_combo = self.memory.combo_open.map(|pos| {
            let options = self.memory.options();
            combobox_menu(
                t,
                pos,
                &options,
                self.memory.region_id(),
                {
                    let this = this.clone();
                    move |id, _, cx| this.update(cx, |d, cx| d.memory_set_region(id, cx))
                },
                cx.listener(|this, _: &MouseDownEvent, _, cx| this.memory_close_combo(cx)),
            )
        });

        let bp_combo = self.breakpoints.combo_open.map(|pos| {
            let options = self.breakpoints.options();
            combobox_menu(
                t,
                pos,
                &options,
                self.breakpoints.draft_kind().id(),
                {
                    let this = this.clone();
                    move |id, _, cx| this.update(cx, |d, cx| d.bp_set_kind(id, cx))
                },
                cx.listener(|this, _: &MouseDownEvent, _, cx| this.bp_close_combo(cx)),
            )
        });

        let memory_menu = self.memory.menu.as_ref().map(|menu| {
            let (bp_start, bp_end) = menu.range;
            let bp_label: SharedString = if bp_end > bp_start {
                format!("${}...{:04X}", menu.address, bp_end & 0xFFFF).into()
            } else {
                format!("${}", menu.address).into()
            };
            let items = vec![
                MenuItem::new(
                    "Copy Address",
                    clipboard_listener(cx, menu.address.clone(), Debugger::memory_close_menu),
                ),
                MenuItem::new(
                    "Copy Hex",
                    clipboard_listener(cx, menu.hex.clone(), Debugger::memory_close_menu),
                ),
                MenuItem::new(
                    "Copy Raw",
                    clipboard_listener(cx, menu.raw.clone(), Debugger::memory_close_menu),
                ),
                MenuItem::new(
                    "Copy ASCII",
                    clipboard_listener(cx, menu.ascii.clone(), Debugger::memory_close_menu),
                ),
                MenuItem::new("Add read breakpoint", {
                    let label = bp_label.clone();
                    cx.listener(move |this, _, _, cx| {
                        this.bp_add_mem(bp_start, bp_end, BreakpointKind::Read, label.clone(), cx)
                    })
                }),
                MenuItem::new("Add write breakpoint", {
                    let label = bp_label.clone();
                    cx.listener(move |this, _, _, cx| {
                        this.bp_add_mem(bp_start, bp_end, BreakpointKind::Write, label.clone(), cx)
                    })
                }),
            ];
            context_menu(
                t,
                menu.position,
                items,
                cx.listener(|this, _: &MouseDownEvent, _, cx| this.memory_close_menu(cx)),
            )
        });

        let log_collapsed = self.is_collapsed(LogWidget::ID);
        let log = LogWidget::new(self.logs.view(), self.logs.scroll.clone())
            .collapsed(log_collapsed)
            .target_label(self.logs.target_label())
            .level_label(self.logs.level_label())
            .message_input(self.log_filter_input.clone())
            .on_toggle(cx.listener(|this, _, _, cx| this.toggle(LogWidget::ID, cx)))
            .on_target_open(
                cx.listener(|this, ev: &ClickEvent, _, cx| this.log_open_target(ev.position(), cx)),
            )
            .on_level_open(
                cx.listener(|this, ev: &ClickEvent, _, cx| this.log_open_level(ev.position(), cx)),
            )
            .on_clear(cx.listener(|this, _, _, cx| this.clear_logs(cx)));
        let log_region = div()
            .flex()
            .flex_col()
            .when(!log_collapsed, |d| d.h(px(180.)).min_h(px(0.)))
            .child(log);

        let log_target_combo = self.logs.target_combo.map(|pos| {
            let options = self.logs.target_options();
            let selected = self.logs.target_id();
            combobox_menu(
                t,
                pos,
                &options,
                &selected,
                {
                    let this = this.clone();
                    move |id, _, cx| this.update(cx, |d, cx| d.log_set_target(id, cx))
                },
                cx.listener(|this, _: &MouseDownEvent, _, cx| this.log_close_target(cx)),
            )
        });

        let log_level_combo = self.logs.level_combo.map(|pos| {
            let options = self.logs.level_options();
            let selected = self.logs.level_id();
            combobox_menu(
                t,
                pos,
                &options,
                &selected,
                {
                    let this = this.clone();
                    move |id, _, cx| this.update(cx, |d, cx| d.log_set_level(id, cx))
                },
                cx.listener(|this, _: &MouseDownEvent, _, cx| this.log_close_level(cx)),
            )
        });

        let manifest = self.target.manifest();
        let mut panels: HashMap<String, AnyElement> = HashMap::new();
        panels.insert(
            "controls".into(),
            div().flex_none().child(controls).into_any_element(),
        );
        panels.insert("breakpoints".into(), breakpoints.into_any_element());
        panels.insert(
            "cartridge".into(),
            div().flex_none().child(cartridge).into_any_element(),
        );
        panels.insert("mmio".into(), mmio.into_any_element());
        panels.insert("memory".into(), memory_widget.into_any_element());
        panels.insert("log".into(), log_region.into_any_element());
        panels.extend(cpu_panels);
        panels.extend(disasm_panels);

        let mut take = |location: DockLocation| -> Vec<AnyElement> {
            manifest
                .in_location(location)
                .filter_map(|panel| panels.remove(panel.id.as_ref()))
                .collect()
        };
        let left_panels = take(DockLocation::Left);
        let center_panels = take(DockLocation::Center);
        let right_panels = take(DockLocation::Right);
        let bottom_panels = take(DockLocation::Bottom);

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
                    .flex_col()
                    .flex_1()
                    .min_h(px(0.))
                    .gap_2()
                    .p_2()
                    .child(
                        div()
                            .flex()
                            .flex_1()
                            .min_h(px(0.))
                            .gap_2()
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .w(px(360.))
                                    .flex_none()
                                    .min_h(px(0.))
                                    .overflow_hidden()
                                    .gap_2()
                                    .children(left_panels),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .flex_1()
                                    .min_w(px(0.))
                                    .min_h(px(0.))
                                    .gap_2()
                                    .children(center_panels),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .w(px(300.))
                                    .flex_none()
                                    .min_h(px(0.))
                                    .gap_2()
                                    .children(right_panels),
                            ),
                    )
                    .children(bottom_panels),
            )
            .child(status_bar(t, &self.status))
            .children(disasm_menus)
            .children(settings_popup)
            .children(bp_combo)
            .children(memory_combo)
            .children(memory_menu)
            .children(log_target_combo)
            .children(log_level_combo)
    }
}
