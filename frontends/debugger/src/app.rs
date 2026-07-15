use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use cirsium::assets::icon;
use cirsium::components::{
    CheckboxSize, MenuItem, Segmented, StatusView, TextInput, TextInputEvent, checkbox,
    combobox_menu, context_menu, field_button, icon_button, popover, status_bar, title_bar,
};
use cirsium::widgets::{
    AudioScopeView, AudioScopeWidget, AudioScopesView, BreakpointKind, BreakpointsWidget,
    CallStackView, CallStackWidget, CartridgeView, CartridgeWidget, ChannelScopesWidget,
    ControlsView, ControlsWidget, CpuWidget, DisassemblyWidget, DmaView, DmaWidget,
    FramebufferView, FramebufferWidget, GridPanel, LogWidget, MemoryWidget, MmioView, MmioWidget,
    PaletteView, PaletteWidget, PpuView, PpuWidget, RowProvider, SpritesView, SpritesWidget,
    StructuredPanel, StructuredView, TablePanel, TableView, TilemapView, TilemapWidget,
    TilesetLayer, TilesetView, TilesetWidget, ViewMode,
};
use cirsium::{
    ACCENT_PRESETS, ChangeTracker, GUEST_COLORS, Highlight, Theme, ThemeMode, set_theme,
};
use debugger_model::{CpuId, CpuView, DebugCore, RunOutcome};
use frontend_model::{
    ComponentManifest, DockLocation, PanelDescriptor, PanelKind, RefreshPolicy, WorkspaceIcon,
};
use gpui::prelude::*;
use gpui::{
    AnyElement, App, ClickEvent, ClipboardItem, Context, Div, Entity, MouseDownEvent, Pixels,
    Point, RenderImage, SharedString, Size, Task, Window, div, px, rgb,
};
use snes::debug::{INSPECTOR_ID as AUDIO_INSPECTOR_ID, VOICES_ID as DSP_VOICES_TABLE_ID};
use web_time::Instant;

use crate::breakpoints::Breakpoints;
use crate::disasm::Disasm;
use crate::logging::{LogBuffer, Logs};
use crate::memory::{self, Memory};

struct ProcessorPane {
    id: CpuId,
    title: SharedString,
    space: Arc<str>,
    cpu_view: CpuView,
    disasm: Disasm,
    call_stack: CallStackView,
    reg_changes: ChangeTracker<Arc<str>>,
}

#[derive(Default)]
struct GridState {
    layer: usize,
    selected: HashMap<usize, usize>,
    hovered: Option<usize>,
    viewport: Size<f32>,
}

impl GridState {
    fn selected(&self) -> Option<usize> {
        self.selected.get(&self.layer).copied()
    }

    fn toggle_selected(&mut self, index: usize) {
        if self.selected.get(&self.layer) == Some(&index) {
            self.selected.remove(&self.layer);
        } else {
            self.selected.insert(self.layer, index);
        }
    }

    fn set_hovered(&mut self, hovered: Option<usize>) -> bool {
        let changed = self.hovered != hovered;
        if changed {
            self.hovered = hovered;
        }
        changed
    }

    fn set_viewport(&mut self, viewport: Size<f32>) -> bool {
        let changed = (self.viewport.width - viewport.width).abs() > 0.5
            || (self.viewport.height - viewport.height).abs() > 0.5;

        if changed {
            self.viewport = viewport;
        }

        changed
    }
}

pub(crate) struct Debugger {
    target: Box<dyn DebugCore>,
    cartridge: CartridgeView,
    mmio_view: MmioView,
    dma_view: DmaView,
    palette_view: PaletteView,
    palette_selected: Option<(usize, usize)>,
    structured_views: HashMap<SharedString, Arc<StructuredView>>,
    table_views: HashMap<SharedString, Arc<TableView>>,
    table_selection: HashMap<SharedString, usize>,
    audio_scopes: AudioScopesView,
    scope: AudioScopeView,
    scope_samples: Vec<f32>,
    audio_samples: Vec<i16>,
    ppu_view: PpuView,
    tileset: Option<TilesetView>,
    tileset_layers: Vec<TilesetLayer>,
    tileset_ui: GridState,
    tilemap: Option<TilemapView>,
    tilemap_ui: GridState,
    sprites: Option<SpritesView>,
    sprites_ui: GridState,
    framebuffer: Option<FramebufferView>,
    expanded: HashMap<&'static str, HashSet<SharedString>>,
    manifest: ComponentManifest,
    status: StatusView,
    panes: Vec<ProcessorPane>,
    memory: Memory,
    breakpoints: Breakpoints,
    logs: Logs,
    show_log: bool,
    jump_input: Entity<TextInput>,
    bp_input: Entity<TextInput>,
    log_filter_input: Entity<TextInput>,
    collapsed: HashSet<SharedString>,
    settings: Option<Point<Pixels>>,
    live_refresh: bool,
    workspace: SharedString,
    running: bool,
    run_task: Option<Task<()>>,
    audio: crate::audio_out::AudioOut,
    accent: usize,
}

const DEFAULT_FRAME_INTERVAL: Duration = Duration::from_millis(16);
const FRAME_PACE_YIELD: Duration = Duration::from_millis(1);

#[derive(Clone, Copy, PartialEq, Eq)]
enum CpuRefresh {
    Highlight,
    Baseline,
}

impl Debugger {
    pub(crate) fn new(
        target: Box<dyn DebugCore>,
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
                    title: desc.title.into(),
                    space: desc.space,
                    cpu_view: CpuView::default(),
                    call_stack: CallStackView::default(),
                    reg_changes: ChangeTracker::default(),
                };
                Self::refresh_pane_view(target.as_ref(), &mut pane, CpuRefresh::Baseline);
                pane
            })
            .collect();
        assert!(
            !panes.is_empty(),
            "a DebugCore must expose at least one processor"
        );

        let jump_input = cx.new(|cx| TextInput::new(cx, "go to...").hex_only(true).max_len(6));
        cx.subscribe(&jump_input, Self::on_jump_event).detach();

        let bp_input = cx.new(|cx| TextInput::new(cx, "address...").hex_only(true).max_len(6));
        cx.subscribe(&bp_input, Self::on_bp_event).detach();

        let log_filter_input = cx.new(|cx| TextInput::new(cx, "filter messages..."));
        cx.subscribe(&log_filter_input, Self::on_log_filter_event)
            .detach();

        let manifest = target.manifest();
        manifest.validate();
        let workspace = manifest
            .workspaces
            .first()
            .map(|w| SharedString::from(w.id.clone()))
            .unwrap_or_default();

        let audio = crate::audio_out::AudioOut::new(target.audio_spec());

        Self {
            cartridge: target.cartridge().clone().into(),
            mmio_view: target.mmio_view().into(),
            dma_view: target.dma_view().into(),
            palette_view: target.palette_view().into(),
            palette_selected: None,
            structured_views: HashMap::new(),
            table_views: HashMap::new(),
            table_selection: HashMap::new(),
            audio_scopes: AudioScopesView::default(),
            scope: AudioScopeView::default(),
            scope_samples: Vec::new(),
            audio_samples: Vec::new(),
            ppu_view: target.ppu_view().into(),
            tileset: target.tileset(0, 0).map(Into::into),
            tileset_layers: target
                .tileset_layers()
                .into_iter()
                .map(Into::into)
                .collect(),
            tileset_ui: GridState::default(),
            tilemap: target.tilemap(0).map(Into::into),
            tilemap_ui: GridState::default(),
            sprites: target.sprites().map(Into::into),
            sprites_ui: GridState::default(),
            framebuffer: target.framebuffer().map(Into::into),
            expanded: HashMap::new(),
            manifest,
            status: target.status("paused").into(),
            memory: Memory::new(target.as_ref()),
            breakpoints: Breakpoints::default(),
            logs: Logs::new(log_buffer),
            show_log: false,
            jump_input,
            bp_input,
            log_filter_input,
            target,
            panes,
            collapsed: HashSet::from([SharedString::from(LogWidget::ID)]),
            settings: None,
            live_refresh: true,
            workspace,
            running: false,
            run_task: None,
            audio,
            accent: 0,
        }
    }

    fn refresh_pane_view(target: &dyn DebugCore, pane: &mut ProcessorPane, mode: CpuRefresh) {
        pane.cpu_view = Self::track_changes(target.cpu_view(&pane.id), &mut pane.reg_changes, mode);
        pane.disasm.recompute(target);
        pane.call_stack = target.call_stack(&pane.id).into();
    }

    fn selected_color(&self) -> usize {
        self.palette_selected.map_or(0, |(palette, color)| {
            let row = self
                .palette_view
                .palettes
                .get(palette)
                .map_or(16, |row| row.len());
            palette * row + color
        })
    }

    fn drop_grid_image(cx: &mut Context<Self>, image: Option<Arc<RenderImage>>) {
        if let Some(image) = image {
            cx.defer(move |cx| cx.drop_image(image, None));
        }
    }

    fn refresh_tileset(&mut self, cx: &mut Context<Self>) {
        Self::drop_grid_image(cx, self.tileset.take().and_then(|view| view.image));
        self.tileset = self
            .target
            .tileset(self.tileset_ui.layer, self.selected_color())
            .map(Into::into);
    }

    fn refresh_tilemap(&mut self, cx: &mut Context<Self>) {
        Self::drop_grid_image(cx, self.tilemap.take().and_then(|view| view.image));
        self.tilemap = self.target.tilemap(self.tilemap_ui.layer).map(Into::into);
    }

    fn refresh_sprites(&mut self, cx: &mut Context<Self>) {
        if self.is_collapsed(SpritesWidget::ID) {
            return;
        }

        Self::drop_grid_image(cx, self.sprites.take().and_then(|view| view.image));
        self.sprites = self.target.sprites().map(Into::into);
    }

    fn refresh_framebuffer(&mut self, cx: &mut Context<Self>) {
        if self.is_collapsed(FramebufferWidget::ID) {
            return;
        }

        Self::drop_grid_image(cx, self.framebuffer.take().and_then(|view| view.image));
        self.framebuffer = self.target.framebuffer().map(Into::into);
    }

    fn should_refresh(&self, panel: &PanelDescriptor, live: bool) -> bool {
        !live || self.live_refresh || panel.refresh == RefreshPolicy::Continuous
    }

    fn wants_refresh(&self, live: bool, kind: impl Fn(&PanelKind) -> bool) -> bool {
        self.manifest
            .in_workspace(&self.workspace)
            .any(|panel| kind(&panel.kind) && self.should_refresh(panel, live))
    }

    fn refresh_panes(&mut self, mode: CpuRefresh, live: bool, cx: &mut Context<Self>) {
        for ix in 0..self.panes.len() {
            let id = self.panes[ix].id.clone();
            let visible = self.wants_refresh(live, |kind| {
                matches!(
                    kind,
                    PanelKind::Registers { cpu } | PanelKind::Disassembly { cpu }
                        if **cpu == *id.as_str()
                )
            });

            if visible {
                Self::refresh_pane_view(self.target.as_ref(), &mut self.panes[ix], mode);
            }
        }

        if self.wants_refresh(live, |kind| matches!(kind, PanelKind::Mmio)) {
            self.mmio_view = self.target.mmio_view().into();
        }

        if self.wants_refresh(live, |kind| matches!(kind, PanelKind::Dma)) {
            self.dma_view = self.target.dma_view().into();
        }

        if self.wants_refresh(live, |kind| matches!(kind, PanelKind::Framebuffer)) {
            self.refresh_framebuffer(cx);
        }

        self.refresh_video_panes(live, cx);
    }

    fn refresh_video_panes(&mut self, live: bool, cx: &mut Context<Self>) {
        if self.wants_refresh(live, |kind| matches!(kind, PanelKind::Palette)) {
            self.palette_view = self.target.palette_view().into();
        }

        if self.wants_refresh(live, |kind| matches!(kind, PanelKind::Ppu)) {
            self.ppu_view = self.target.ppu_view().into();
        }

        if self.wants_refresh(live, |kind| matches!(kind, PanelKind::Tileset)) {
            self.tileset_layers = self
                .target
                .tileset_layers()
                .into_iter()
                .map(Into::into)
                .collect();
            self.refresh_tileset(cx);
        }

        if self.wants_refresh(live, |kind| matches!(kind, PanelKind::Tilemap)) {
            self.refresh_tilemap(cx);
        }

        if self.wants_refresh(live, |kind| matches!(kind, PanelKind::Sprites)) {
            self.refresh_sprites(cx);
        }

        let (tables, structured, scopes, inspector) = self.visible_generic_panels(live);

        for id in tables {
            let view = Arc::new(self.target.table_view(id.as_ref()));
            self.table_views.insert(id, view);
        }

        for id in structured {
            let view = Arc::new(self.target.structured_view(id.as_ref(), None));
            self.structured_views.insert(id, view);
        }

        if scopes {
            self.audio_scopes = self.target.audio_scopes_view();
        }

        if inspector {
            self.refresh_audio_inspector();
        }
    }

    fn visible_generic_panels(
        &self,
        live: bool,
    ) -> (Vec<SharedString>, Vec<SharedString>, bool, bool) {
        let mut tables = Vec::new();
        let mut structured = Vec::new();
        let mut scopes = false;
        let mut inspector = false;

        for panel in self.manifest.in_workspace(&self.workspace) {
            if !self.should_refresh(panel, live) || self.is_collapsed(panel.id.as_ref()) {
                continue;
            }

            match panel.kind {
                PanelKind::Table => tables.push(panel.id.clone().into()),
                PanelKind::Structured => structured.push(panel.id.clone().into()),
                PanelKind::ChannelScopes => scopes = true,
                PanelKind::AudioScope => inspector = true,
                _ => {}
            }
        }

        (tables, structured, scopes, inspector)
    }

    fn refresh_audio_inspector(&mut self) {
        let sel = self
            .table_selection
            .get(DSP_VOICES_TABLE_ID)
            .copied()
            .unwrap_or(0);
        let view = self.target.structured_view(AUDIO_INSPECTOR_ID, Some(sel));
        self.structured_views
            .insert(AUDIO_INSPECTOR_ID.into(), Arc::new(view));
    }

    fn push_scope(buffer: &mut Vec<f32>, samples: &[i16]) {
        const SCOPE_LEN: usize = 512;

        for pair in samples.chunks_exact(2) {
            let mono = (pair[0] as f32 + pair[1] as f32) * 0.5 / 32768.0;
            buffer.push(mono);
        }

        let len = buffer.len();
        if len > SCOPE_LEN {
            buffer.drain(..len - SCOPE_LEN);
        }
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

    fn toggle_in(set: &mut HashSet<SharedString>, id: SharedString) {
        if !set.remove(&id) {
            set.insert(id);
        }
    }

    fn toggle(&mut self, id: impl Into<SharedString>, cx: &mut Context<Self>) {
        let id = id.into();
        Self::toggle_in(&mut self.collapsed, id.clone());

        if !self.collapsed.contains(&id) {
            match id.as_ref() {
                FramebufferWidget::ID => self.refresh_framebuffer(cx),
                SpritesWidget::ID => self.refresh_sprites(cx),
                _ => {}
            }
        }

        cx.notify();
    }

    fn expanded_set(&self, widget: &str) -> HashSet<SharedString> {
        self.expanded.get(widget).cloned().unwrap_or_default()
    }

    fn toggle_expanded(&mut self, widget: &'static str, id: SharedString, cx: &mut Context<Self>) {
        Self::toggle_in(self.expanded.entry(widget).or_default(), id);
        cx.notify();
    }

    fn select_palette_color(&mut self, palette: usize, color: usize, cx: &mut Context<Self>) {
        let selected = Some((palette, color));
        self.palette_selected = (self.palette_selected != selected).then_some((palette, color));
        self.refresh_tileset(cx);
        cx.notify();
    }

    fn select_table_row(&mut self, table_id: SharedString, row: usize, cx: &mut Context<Self>) {
        self.table_selection.insert(table_id.clone(), row);
        if table_id.as_ref() == DSP_VOICES_TABLE_ID {
            self.refresh_audio_inspector();
        }
        cx.notify();
    }

    fn wire_grid(
        &self,
        panel: GridPanel,
        id: &'static str,
        state: &GridState,
        ui: fn(&mut Self) -> &mut GridState,
        refresh: fn(&mut Self, &mut Context<Self>),
        cx: &mut Context<Self>,
    ) -> GridPanel {
        let this = cx.entity();
        panel
            .layer(state.layer)
            .selected(state.selected())
            .hovered(state.hovered)
            .viewport(state.viewport)
            .collapsed(self.is_collapsed(id))
            .on_toggle(cx.listener(move |this, _, _, cx| this.toggle(id, cx)))
            .on_layer_select({
                let this = this.clone();
                move |layer, _, cx| {
                    this.update(cx, |d, cx| {
                        ui(d).layer = layer;
                        refresh(d, cx);
                        cx.notify();
                    })
                }
            })
            .on_select({
                let this = this.clone();
                move |index, _, cx| {
                    this.update(cx, |d, cx| {
                        ui(d).toggle_selected(index);
                        cx.notify();
                    })
                }
            })
            .on_hover({
                let this = this.clone();
                move |hovered, _, cx| {
                    this.update(cx, |d, cx| {
                        if ui(d).set_hovered(hovered) {
                            cx.notify();
                        }
                    })
                }
            })
            .on_viewport({
                let this = this.clone();
                move |viewport, _, cx| {
                    this.update(cx, |d, cx| {
                        if ui(d).set_viewport(viewport) {
                            cx.notify();
                        }
                    })
                }
            })
    }

    fn set_workspace(&mut self, id: SharedString, cx: &mut Context<Self>) {
        self.workspace = id;
        self.refresh_panes(CpuRefresh::Highlight, false, cx);
        cx.notify();
    }

    fn set_accent(&mut self, index: usize, cx: &mut Context<Self>) {
        self.accent = index;
        let mode = Theme::global(cx).mode;
        set_theme(cx, self::themed(mode, index));
        cx.notify();
    }

    fn set_state(&mut self, label: &str) {
        self.status.state = self.target.caption(label).into();
    }

    fn run_frame_interval(&self) -> Duration {
        self.target
            .frame_interval()
            .unwrap_or(DEFAULT_FRAME_INTERVAL)
    }

    fn next_run_wait(next_frame: &mut Instant, interval: Duration) -> Duration {
        *next_frame += interval;
        let now = Instant::now();
        let wait = next_frame.saturating_duration_since(now);
        if wait.is_zero() {
            *next_frame = now;
            FRAME_PACE_YIELD
        } else {
            wait
        }
    }

    fn set_running(&mut self, running: bool, cx: &mut Context<Self>) {
        if self.running == running {
            return;
        }

        self.running = running;
        self.set_state(if running { "running" } else { "paused" });

        if running {
            self.run_task = Some(cx.spawn(async move |this, cx| {
                let mut next_frame = Instant::now();
                loop {
                    let interval = this
                        .update(cx, |this, cx| {
                            if this.run_frame(cx) {
                                Some(this.run_frame_interval())
                            } else {
                                None
                            }
                        })
                        .unwrap_or(None);
                    let Some(interval) = interval else {
                        break;
                    };

                    cx.background_executor()
                        .timer(Self::next_run_wait(&mut next_frame, interval))
                        .await;
                }
            }));
        } else {
            self.run_task = None;
            self.refresh_panes(CpuRefresh::Highlight, false, cx);
        }

        cx.notify();
    }

    fn run_frame(&mut self, cx: &mut Context<Self>) -> bool {
        if !self.running {
            return false;
        }

        let outcome = {
            let _silenced = SilencedPanics::new();
            self.target.debug_run_frame(self.breakpoints.marks())
        };
        self.audio_samples.clear();
        self.target.drain_audio(&mut self.audio_samples);
        self.audio.queue(&self.audio_samples);
        Self::push_scope(&mut self.scope_samples, &self.audio_samples);
        self.scope.samples = self.scope_samples.as_slice().into();

        let keep_going = outcome == RunOutcome::Done;
        self.refresh_panes(CpuRefresh::Highlight, keep_going, cx);
        self.follow_memory();

        if !keep_going {
            self.running = false;
        }

        self.set_state(match outcome {
            RunOutcome::Done => "running",
            RunOutcome::Breakpoint => "breakpoint",
            RunOutcome::Fault => "unimplemented op",
        });

        cx.notify();
        keep_going
    }

    fn step(&mut self, cx: &mut Context<Self>) {
        let ok = {
            let _silenced = SilencedPanics::new();
            self.target.debug_step()
        };

        self.refresh_panes(CpuRefresh::Highlight, false, cx);
        self.set_state(if ok { "paused" } else { "unimplemented op" });
        self.running = false;
        self.run_task = None;
        self.follow_memory();

        cx.notify();
    }

    fn reset(&mut self, cx: &mut Context<Self>) {
        self.target.reset();

        self.refresh_panes(CpuRefresh::Baseline, false, cx);
        self.set_state("reset");
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
        self.set_state("pc set");
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

    fn disasm_hover_edge(&mut self, pane: usize, edge: Option<usize>, cx: &mut Context<Self>) {
        if self.panes[pane].disasm.set_hovered_edge(edge) {
            cx.notify();
        }
    }

    fn disasm_jump(
        &mut self,
        pane: usize,
        from: Point<Pixels>,
        to: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        self.panes[pane].disasm.jump(from, to);
        cx.notify();
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
        let region: Arc<str> = self.memory.region_id().into();
        let note = self.target.annotate(start).into();
        self.breakpoints
            .add_labeled(start, end, kind, cpu, region, label, note);
        self.sync_bp_markers();
        cx.notify();
    }

    /// A processor's id and address space, used to tag breakpoints raised from
    /// that pane (or pane 0 for manually-typed and memory-panel breakpoints).
    fn pane_space(&self, pane: usize) -> (CpuId, Arc<str>) {
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

    fn log_toggle_dedup(&mut self, cx: &mut Context<Self>) {
        self.logs.toggle_dedup();
        cx.notify();
    }

    fn toggle_show_log(&mut self, cx: &mut Context<Self>) {
        self.show_log = !self.show_log;
        cx.notify();
    }

    fn toggle_live_refresh(&mut self, cx: &mut Context<Self>) {
        self.live_refresh = !self.live_refresh;
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

fn accent_at(index: usize) -> u32 {
    let presets = ACCENT_PRESETS.len();
    if index < presets {
        ACCENT_PRESETS[index]
    } else {
        GUEST_COLORS[index - presets].1
    }
}

/// A clickable accent swatch that selects the unified accent `index`.
fn accent_dot(
    t: &Theme,
    index: usize,
    selected: bool,
    cx: &mut Context<Debugger>,
) -> impl IntoElement {
    div()
        .id(("accent", index))
        .size(px(16.))
        .rounded_full()
        .bg(rgb(accent_at(index)))
        .border_2()
        .border_color(if selected {
            t.text_primary
        } else {
            gpui::transparent_black()
        })
        .cursor_pointer()
        .on_click(cx.listener(move |this, _, _, cx| this.set_accent(index, cx)))
}

fn themed(mode: ThemeMode, accent: usize) -> Theme {
    let base = Theme::new(mode);
    if accent == 0 {
        base
    } else {
        base.with_accent(rgb(accent_at(accent)).into())
    }
}

fn workspace_icon(icon: WorkspaceIcon) -> &'static str {
    match icon {
        WorkspaceIcon::Cpu => icon::CPU,
        WorkspaceIcon::Video => icon::MONITOR,
        WorkspaceIcon::Sound => icon::AUDIO,
    }
}

fn panel_column() -> Div {
    div().flex().flex_col().min_w(px(0.)).min_h(px(0.))
}

fn panel_row(left: AnyElement, right: Div) -> AnyElement {
    div()
        .flex()
        .flex_1()
        .min_h(px(0.))
        .gap_2()
        .child(panel_column().flex_1().child(left))
        .child(right)
        .into_any_element()
}

fn half_row(left: AnyElement, right: AnyElement) -> AnyElement {
    self::panel_row(left, panel_column().flex_1().child(right))
}

fn fixed_half_row(left: AnyElement, right: AnyElement) -> AnyElement {
    div()
        .flex()
        .flex_none()
        .gap_2()
        .child(panel_column().flex_1().child(left))
        .child(panel_column().flex_1().child(right))
        .into_any_element()
}

fn with_screen(widget: AnyElement, screen: Option<AnyElement>) -> AnyElement {
    match screen {
        Some(screen) => {
            self::panel_row(widget, panel_column().w(px(300.)).flex_none().child(screen))
        }
        None => widget,
    }
}

type PanicHook = Box<dyn Fn(&std::panic::PanicHookInfo<'_>) + Sync + Send + 'static>;

struct SilencedPanics(Option<PanicHook>);

impl SilencedPanics {
    fn new() -> Self {
        let prev = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {}));
        Self(Some(prev))
    }
}

impl Drop for SilencedPanics {
    fn drop(&mut self) {
        if let Some(prev) = self.0.take() {
            std::panic::set_hook(prev);
        }
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
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::global(cx);
        let t = &*theme;

        self.logs.follow_tail();

        let mut graph_animating = false;
        for pane in &mut self.panes {
            graph_animating |= pane.disasm.graph_animating();
        }
        if graph_animating {
            window.request_animation_frame();
        }

        let this = cx.entity();

        let manifest = &self.manifest;
        let active_workspace = self.workspace.clone();
        let visible: HashSet<&str> = manifest
            .in_workspace(&active_workspace)
            .map(|p| p.id.as_ref())
            .collect();

        let workspace_switcher = (manifest.workspaces.len() > 1).then(|| {
            let mut switcher = Segmented::new("workspace").selected(active_workspace.clone());

            for ws in &manifest.workspaces {
                switcher = switcher.option(
                    SharedString::from(ws.id.clone()),
                    self::workspace_icon(ws.icon),
                    SharedString::from(ws.title.clone()),
                );
            }

            switcher.on_select({
                let this = this.clone();
                move |id, _, cx| {
                    let id = SharedString::from(id.to_string());
                    this.update(cx, |d, cx| d.set_workspace(id, cx));
                }
            })
        });

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
            cx.listener(|this, _, _, cx| {
                let next = Theme::global(cx).mode.toggled();
                set_theme(cx, self::themed(next, this.accent));
                cx.notify();
            }),
        );
        let titlebar_leading = div().flex().items_center().h_full().children(
            workspace_switcher.map(|switcher| div().flex().items_center().child(switcher)),
        );
        let titlebar_trailing = div()
            .flex()
            .items_center()
            .gap_2()
            .h_full()
            .child(settings_button)
            .child(theme_toggle);

        let mut framebuffer = visible
            .contains(FramebufferWidget::ID)
            .then(|| {
                FramebufferWidget::new(self.framebuffer.clone())
                    .collapsed(self.is_collapsed(FramebufferWidget::ID))
                    .on_toggle(cx.listener(|this, _, _, cx| this.toggle(FramebufferWidget::ID, cx)))
            })
            .map(|widget| widget.into_any_element());

        let cartridge = visible.contains(CartridgeWidget::ID).then(|| {
            CartridgeWidget::new(self.cartridge.clone())
                .collapsed(self.is_collapsed(CartridgeWidget::ID))
                .on_toggle(cx.listener(|this, _, _, cx| this.toggle(CartridgeWidget::ID, cx)))
        });

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

        let mmio = MmioWidget::new(self.mmio_view.clone())
            .expanded(self.expanded_set(MmioWidget::ID))
            .collapsed(self.is_collapsed(MmioWidget::ID))
            .on_toggle(cx.listener(|this, _, _, cx| this.toggle(MmioWidget::ID, cx)))
            .on_toggle_register({
                let this = this.clone();
                move |id, _, cx| this.update(cx, |d, cx| d.toggle_expanded(MmioWidget::ID, id, cx))
            });

        let dma = visible.contains(DmaWidget::ID).then(|| {
            DmaWidget::new(self.dma_view.clone())
                .expanded(self.expanded_set(DmaWidget::ID))
                .collapsed(self.is_collapsed(DmaWidget::ID))
                .on_toggle(cx.listener(|this, _, _, cx| this.toggle(DmaWidget::ID, cx)))
                .on_toggle_channel({
                    let this = this.clone();
                    move |id, _, cx| {
                        this.update(cx, |d, cx| d.toggle_expanded(DmaWidget::ID, id, cx))
                    }
                })
        });

        let palette = visible.contains(PaletteWidget::ID).then(|| {
            PaletteWidget::new(self.palette_view.clone())
                .selected(self.palette_selected)
                .collapsed(self.is_collapsed(PaletteWidget::ID))
                .on_toggle(cx.listener(|this, _, _, cx| this.toggle(PaletteWidget::ID, cx)))
                .on_select({
                    let this = this.clone();
                    move |palette, color, _, cx| {
                        this.update(cx, |d, cx| d.select_palette_color(palette, color, cx))
                    }
                })
        });

        let ppu = visible.contains(PpuWidget::ID).then(|| {
            PpuWidget::new(self.ppu_view.clone())
                .expanded(self.expanded_set(PpuWidget::ID))
                .collapsed(self.is_collapsed(PpuWidget::ID))
                .on_toggle(cx.listener(|this, _, _, cx| this.toggle(PpuWidget::ID, cx)))
                .on_toggle_group({
                    let this = this.clone();
                    move |id, _, cx| {
                        this.update(cx, |d, cx| d.toggle_expanded(PpuWidget::ID, id, cx))
                    }
                })
        });

        let tileset = visible.contains(TilesetWidget::ID).then(|| {
            self.wire_grid(
                TilesetWidget::panel(self.tileset.clone()).layers(self.tileset_layers.clone()),
                TilesetWidget::ID,
                &self.tileset_ui,
                |d| &mut d.tileset_ui,
                Self::refresh_tileset,
                cx,
            )
        });

        let tilemap = visible.contains(TilemapWidget::ID).then(|| {
            self.wire_grid(
                TilemapWidget::panel(self.tilemap.clone()).layers(self.tileset_layers.clone()),
                TilemapWidget::ID,
                &self.tilemap_ui,
                |d| &mut d.tilemap_ui,
                Self::refresh_tilemap,
                cx,
            )
        });

        let sprites = visible.contains(SpritesWidget::ID).then(|| {
            self.wire_grid(
                SpritesWidget::panel(self.sprites.clone()),
                SpritesWidget::ID,
                &self.sprites_ui,
                |d| &mut d.sprites_ui,
                Self::refresh_sprites,
                cx,
            )
        });

        let breakpoints =
            visible.contains(BreakpointsWidget::ID).then(|| {
                BreakpointsWidget::new(self.breakpoints.view(), self.breakpoints.draft_kind())
                    .collapsed(self.is_collapsed(BreakpointsWidget::ID))
                    .scrollable(true)
                    .address_input(self.bp_input.clone())
                    .on_toggle(cx.listener(|this, _, _, cx| this.toggle(BreakpointsWidget::ID, cx)))
                    .on_add(cx.listener(|this, _, _, cx| this.bp_add(cx)))
                    .on_kind_open(cx.listener(|this, ev: &ClickEvent, _, cx| {
                        this.bp_open_combo(ev.position(), cx)
                    }))
                    .on_remove({
                        let this = this.clone();
                        move |id, _, cx| this.update(cx, |d, cx| d.bp_remove(id, cx))
                    })
                    .on_toggle_entry({
                        let this = this.clone();
                        move |id, _, cx| this.update(cx, |d, cx| d.bp_toggle(id, cx))
                    })
            });

        let mut cpu_panels: Vec<(String, AnyElement)> = Vec::with_capacity(self.panes.len());
        let mut disasm_panels: Vec<(String, AnyElement)> = Vec::with_capacity(self.panes.len());
        let mut disasm_menus = Vec::new();

        for (pane_ix, pane) in self.panes.iter().enumerate() {
            if visible.contains(pane.id.as_str()) {
                let cpu_id = SharedString::from(pane.id.arc());
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
            }

            let disasm_panel_id = format!("{}.disasm", pane.id.as_str());
            if !visible.contains(disasm_panel_id.as_str()) {
                continue;
            }

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
                    })
                    .hovered_edge(pane.disasm.hovered_edge())
                    .on_edge_hover({
                        let this = this.clone();
                        move |edge, _, cx| {
                            this.update(cx, |d, cx| d.disasm_hover_edge(pane_ix, edge, cx))
                        }
                    })
                    .on_jump({
                        let this = this.clone();
                        move |from, to, _, cx| {
                            this.update(cx, |d, cx| d.disasm_jump(pane_ix, from, to, cx))
                        }
                    });

            let cs_id = SharedString::from(format!("{}-callstack", pane.id.as_str()));
            let cs_title = SharedString::from(format!("{} Call Stack", pane.title));
            let callstack = CallStackWidget::new(pane.call_stack.clone())
                .id(cs_id.clone())
                .title(cs_title)
                .scrollable(true)
                .collapsed(self.is_collapsed(&cs_id))
                .on_toggle({
                    let id = cs_id.clone();
                    cx.listener(move |this, _, _, cx| this.toggle(id.clone(), cx))
                });

            let pane_row = div()
                .flex()
                .flex_1()
                .min_h(px(0.))
                .gap_2()
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .flex_1()
                        .min_w(px(0.))
                        .min_h(px(0.))
                        .child(disassembly),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .w(px(300.))
                        .flex_none()
                        .min_h(px(0.))
                        .child(callstack),
                );

            disasm_panels.push((disasm_panel_id, pane_row.into_any_element()));

            if let Some(menu) = pane.disasm.menu.as_ref() {
                let addr = menu.addr;
                let len = menu.len;

                let mut items = vec![
                    MenuItem::new(
                        "Copy Address",
                        self::clipboard_listener(cx, menu.address.clone(), Debugger::close_menu),
                    ),
                    MenuItem::new(
                        "Copy Bytes",
                        self::clipboard_listener(cx, menu.bytes.clone(), Debugger::close_menu),
                    ),
                    MenuItem::new(
                        "Copy Disassembly",
                        self::clipboard_listener(cx, menu.text.clone(), Debugger::close_menu),
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
                ))
                .child(div().text_xs().text_color(t.text_muted).child("PANELS"))
                .child(checkbox(
                    t,
                    "set-live-refresh",
                    self.live_refresh,
                    "Live refresh while running",
                    CheckboxSize::Small,
                    cx.listener(|this, _, _, cx| this.toggle_live_refresh(cx)),
                ))
                .child(div().text_xs().text_color(t.text_muted).child("LOG"))
                .child(checkbox(
                    t,
                    "set-log-show",
                    self.show_log,
                    "Show log panel",
                    CheckboxSize::Small,
                    cx.listener(|this, _, _, cx| this.toggle_show_log(cx)),
                ))
                .child(checkbox(
                    t,
                    "set-log-dedup",
                    self.logs.dedup(),
                    "Drop repeated logs",
                    CheckboxSize::Small,
                    cx.listener(|this, _, _, cx| this.log_toggle_dedup(cx)),
                ))
                .child(div().text_xs().text_color(t.text_muted).child("ACCENT"))
                .child({
                    let selected = self.accent;
                    (0..ACCENT_PRESETS.len())
                        .fold(div().flex().items_center().gap_2().py_1(), |row, i| {
                            row.child(self::accent_dot(t, i, i == selected, cx))
                        })
                })
                .child(
                    div()
                        .text_xs()
                        .text_color(t.text_muted)
                        .child("SPECIAL GUEST COLORS"),
                )
                .child(GUEST_COLORS.iter().enumerate().fold(
                    div().flex().flex_col().gap(px(3.)),
                    |col, (gi, &(name, _color))| {
                        let index = ACCENT_PRESETS.len() + gi;
                        col.child(
                            div()
                                .flex()
                                .items_center()
                                .gap_2()
                                .child(self::accent_dot(t, index, index == self.accent, cx))
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(t.text_secondary)
                                        .child(name.to_string()),
                                ),
                        )
                    },
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
                    self::clipboard_listener(cx, menu.address.clone(), Debugger::memory_close_menu),
                ),
                MenuItem::new(
                    "Copy Hex",
                    self::clipboard_listener(cx, menu.hex.clone(), Debugger::memory_close_menu),
                ),
                MenuItem::new(
                    "Copy Raw",
                    self::clipboard_listener(cx, menu.raw.clone(), Debugger::memory_close_menu),
                ),
                MenuItem::new(
                    "Copy ASCII",
                    self::clipboard_listener(cx, menu.ascii.clone(), Debugger::memory_close_menu),
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

        let log_region = self.show_log.then(|| {
            let log_collapsed = self.is_collapsed(LogWidget::ID);
            let log = LogWidget::new(self.logs.view(), self.logs.scroll.clone())
                .collapsed(log_collapsed)
                .target_label(self.logs.target_label())
                .level_label(self.logs.level_label())
                .message_input(self.log_filter_input.clone())
                .on_toggle(cx.listener(|this, _, _, cx| this.toggle(LogWidget::ID, cx)))
                .on_target_open(cx.listener(|this, ev: &ClickEvent, _, cx| {
                    this.log_open_target(ev.position(), cx)
                }))
                .on_level_open(cx.listener(|this, ev: &ClickEvent, _, cx| {
                    this.log_open_level(ev.position(), cx)
                }))
                .on_clear(cx.listener(|this, _, _, cx| this.clear_logs(cx)));
            div()
                .flex()
                .flex_col()
                .when(!log_collapsed, |d| d.h(px(180.)).min_h(px(0.)))
                .child(log)
        });

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

        let mut panels: HashMap<String, AnyElement> = HashMap::new();
        panels.insert(
            ControlsWidget::ID.into(),
            div().flex_none().child(controls).into_any_element(),
        );
        if let Some(breakpoints) = breakpoints {
            panels.insert(BreakpointsWidget::ID.into(), breakpoints.into_any_element());
        }
        if let Some(dma) = dma {
            panels.insert(DmaWidget::ID.into(), dma.into_any_element());
        }
        if let Some(palette) = palette {
            panels.insert(PaletteWidget::ID.into(), palette.into_any_element());
        }
        if let Some(ppu) = ppu {
            panels.insert(PpuWidget::ID.into(), ppu.into_any_element());
        }

        let panel_glyph: &str = manifest
            .workspaces
            .iter()
            .find(|w| w.id.as_ref() == active_workspace.as_ref())
            .map_or(icon::AUDIO, |w| self::workspace_icon(w.icon));
        for panel in manifest.in_workspace(&active_workspace) {
            let id = panel.id.as_ref();
            if !visible.contains(id)
                || !matches!(
                    panel.kind,
                    PanelKind::Table
                        | PanelKind::Structured
                        | PanelKind::ChannelScopes
                        | PanelKind::AudioScope
                )
            {
                continue;
            }
            let pid = SharedString::from(panel.id.clone());
            let title = SharedString::from(panel.title.clone());
            let on_toggle = {
                let id = pid.clone();
                cx.listener(move |this, _, _, cx| this.toggle(id.clone(), cx))
            };
            let element = match panel.kind {
                PanelKind::Table => {
                    let view = self.table_views.get(id).cloned().unwrap_or_default();
                    TablePanel::new(pid.clone(), title, view)
                        .selected(self.table_selection.get(id).copied().unwrap_or(0))
                        .collapsed(self.is_collapsed(id))
                        .on_toggle(on_toggle)
                        .on_select({
                            let this = this.clone();
                            let id = pid.clone();
                            move |row, _, cx| {
                                let id = id.clone();
                                this.update(cx, |d, cx| d.select_table_row(id, row, cx))
                            }
                        })
                        .glyph(panel_glyph)
                        .into_any_element()
                }
                PanelKind::Structured => {
                    let view = self.structured_views.get(id).cloned().unwrap_or_default();
                    StructuredPanel::new(pid.clone(), title, view)
                        .collapsed(self.is_collapsed(id))
                        .on_toggle(on_toggle)
                        .glyph(panel_glyph)
                        .into_any_element()
                }
                PanelKind::ChannelScopes => {
                    ChannelScopesWidget::new(pid.clone(), title, self.audio_scopes.clone())
                        .collapsed(self.is_collapsed(id))
                        .on_toggle(on_toggle)
                        .glyph(panel_glyph)
                        .into_any_element()
                }
                PanelKind::AudioScope => {
                    let scope = AudioScopeWidget::new(pid.clone(), title, self.scope.clone())
                        .collapsed(self.is_collapsed(id))
                        .on_toggle(on_toggle)
                        .glyph(panel_glyph);
                    let inspector_view = self
                        .structured_views
                        .get(AUDIO_INSPECTOR_ID)
                        .cloned()
                        .unwrap_or_default();
                    let inspector =
                        StructuredPanel::new(AUDIO_INSPECTOR_ID, "Audio Inspector", inspector_view)
                            .glyph(panel_glyph);
                    self::fixed_half_row(scope.into_any_element(), inspector.into_any_element())
                }
                _ => unreachable!(),
            };
            panels.insert(id.to_string(), element);
        }
        match (tileset, tilemap) {
            (Some(tileset), Some(tilemap)) => {
                panels.insert(
                    TilesetWidget::ID.into(),
                    self::half_row(tileset.into_any_element(), tilemap.into_any_element()),
                );
            }
            (Some(tileset), None) => {
                panels.insert(TilesetWidget::ID.into(), tileset.into_any_element());
            }
            (None, Some(tilemap)) => {
                panels.insert(TilemapWidget::ID.into(), tilemap.into_any_element());
            }
            (None, None) => {}
        }
        if let Some(cartridge) = cartridge {
            panels.insert(
                CartridgeWidget::ID.into(),
                div().flex_none().child(cartridge).into_any_element(),
            );
        }
        panels.insert(MmioWidget::ID.into(), mmio.into_any_element());

        if let Some(sprites) = sprites {
            let element = match framebuffer.take() {
                Some(screen) => self::half_row(sprites.into_any_element(), screen),
                None => sprites.into_any_element(),
            };
            panels.insert(SpritesWidget::ID.into(), element);
        }
        if visible.contains(MemoryWidget::ID) {
            panels.insert(
                MemoryWidget::ID.into(),
                self::with_screen(memory_widget.into_any_element(), framebuffer.take()),
            );
        } else if let Some(screen) = framebuffer.take() {
            panels.insert(FramebufferWidget::ID.into(), screen);
        }
        if let Some(log_region) = log_region {
            panels.insert(LogWidget::ID.into(), log_region.into_any_element());
        }
        panels.extend(cpu_panels);
        panels.extend(disasm_panels);

        let mut take = |location: DockLocation| -> Vec<AnyElement> {
            manifest
                .in_location(&active_workspace, location)
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
            .child(title_bar(
                t,
                "Sly Debugger",
                titlebar_leading,
                titlebar_trailing,
            ))
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
                                    .w(px(300.))
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
