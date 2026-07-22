use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use cirsium::assets::icon;
use cirsium::components::{
    CheckboxSize, MenuItem, PanelChrome, Segmented, StatusView, TextInput, TextInputEvent,
    checkbox, combobox_menu, context_menu, field_button, icon_button, popover, status_bar,
    title_bar,
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
    ACCENT_PRESETS, ChangeTracker, GUEST_COLORS, Highlight, Theme, ThemeMode, accent_at, set_theme,
};
use core_model::{Button, ExecutionState};
use debugger_model::{CpuId, CpuView, DebugCore, RunOutcome, VideoGenerations};
use gpui::prelude::*;
use gpui::{
    AnyElement, App, ClickEvent, ClipboardItem, Context, Entity, FocusHandle, KeyDownEvent,
    KeyUpEvent, MouseDownEvent, Pixels, Point, RenderImage, SharedString, Size, Task, Window, div,
    px, rgb,
};
use web_time::Instant;
use workspace_model::{PanelDescriptor, PanelKind, RefreshPolicy, WorkspaceIcon};

use crate::breakpoints::Breakpoints;
use crate::disasm::Disasm;
use crate::logging::{LogBuffer, Logs};
use crate::memory::{self, Memory};

mod actions;
mod input;
mod panels;
mod refresh;
mod render;
mod runtime;

use panels::{DebuggerPanelFactory, PanelFrames};

struct ProcessorPane {
    id: CpuId,
    title: SharedString,
    space: Arc<str>,
    cpu_view: Arc<cirsium::widgets::CpuView>,
    disasm: Disasm,
    call_stack: Arc<CallStackView>,
    reg_changes: ChangeTracker<Arc<str>>,
}

#[derive(Default)]
struct GridState {
    layer: usize,
    selected: HashMap<usize, usize>,
    hovered: Option<usize>,
    viewport: Size<f32>,
    show_window_overlays: bool,
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
    cartridge: Arc<CartridgeView>,
    mmio_view: Arc<MmioView>,
    dma_view: Arc<DmaView>,
    palette_view: Arc<PaletteView>,
    palette_selected: Option<(usize, usize)>,
    structured_views: HashMap<SharedString, Arc<StructuredView>>,
    table_views: HashMap<SharedString, Arc<TableView>>,
    table_selection: HashMap<SharedString, usize>,
    scopes_views: HashMap<SharedString, AudioScopesView>,
    scope_samples: Vec<f32>,
    scope_view: AudioScopeView,
    audio_samples: Vec<i16>,
    ppu_view: Arc<PpuView>,
    tileset: Option<TilesetView>,
    tileset_layers: Vec<TilesetLayer>,
    tileset_ui: GridState,
    tilemap: Option<TilemapView>,
    tilemap_ui: GridState,
    sprites: Option<SpritesView>,
    sprites_ui: GridState,
    framebuffer: Option<FramebufferView>,
    framebuffer_staged: Vec<u8>,
    framebuffer_dirty: bool,
    palette_key: Option<u64>,
    tileset_key: Option<(u64, usize, usize)>,
    tilemap_key: Option<(u64, usize)>,
    tilemap_overlay_key: Option<(u64, usize)>,
    sprites_key: Option<u64>,
    expanded: HashMap<&'static str, HashSet<SharedString>>,
    registry: cirsium::PanelRegistry,
    panel_frames: PanelFrames,
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
    execution: ExecutionState,
    game_focus: FocusHandle,
    pressed_buttons: Vec<Button>,
    run_task: Option<Task<()>>,
    audio: crate::audio_out::AudioOut,
    stats: crate::stats::RunStats,
    perf: Vec<SharedString>,
    emu_clock: Instant,
    accent: usize,
    #[cfg(target_family = "wasm")]
    web_tick_armed: bool,
}

const DEFAULT_FRAME_INTERVAL: Duration = Duration::from_millis(16);
const MAX_CATCHUP_FRAMES: u32 = 3;
#[cfg(not(target_family = "wasm"))]
const FRAME_PACE_YIELD: Duration = Duration::from_millis(1);

#[derive(Clone, Copy, PartialEq, Eq)]
enum CpuRefresh {
    Highlight,
    Baseline,
}

#[derive(Default)]
struct GenericRefresh {
    tables: Vec<SharedString>,
    structured: Vec<(SharedString, Option<usize>)>,
    scopes: Vec<SharedString>,
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
                    cpu_view: Arc::default(),
                    call_stack: Arc::default(),
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
        let workspace = manifest
            .workspaces
            .first()
            .map(|w| SharedString::from(w.id.clone()))
            .unwrap_or_default();

        let audio = crate::audio_out::AudioOut::new(target.audio_spec());
        let panel_frames = PanelFrames::default();
        let mut panel_factory = DebuggerPanelFactory::new(panel_frames.clone());
        let registry = cirsium::PanelRegistry::new(manifest, &mut panel_factory, cx)
            .expect("invalid component manifest");
        let mut status: StatusView = target.session_info().into();
        status.state = target.caption("paused").into();

        Self {
            cartridge: Arc::new(target.cartridge().clone().into()),
            mmio_view: Arc::new(target.mmio_view().into()),
            dma_view: Arc::new(target.dma_view().into()),
            palette_view: Arc::new(target.palette_view().into()),
            palette_selected: None,
            structured_views: HashMap::new(),
            table_views: HashMap::new(),
            table_selection: HashMap::new(),
            scopes_views: HashMap::new(),
            scope_samples: Vec::new(),
            scope_view: AudioScopeView::default(),
            audio_samples: Vec::new(),
            ppu_view: Arc::new(target.ppu_view().into()),
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
            framebuffer: target.software_frame().map(Into::into),
            framebuffer_staged: Vec::new(),
            framebuffer_dirty: false,
            palette_key: None,
            tileset_key: None,
            tilemap_key: None,
            tilemap_overlay_key: None,
            sprites_key: None,
            expanded: HashMap::new(),
            registry,
            panel_frames,
            status,
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
            execution: ExecutionState::Paused,
            game_focus: cx.focus_handle(),
            pressed_buttons: Vec::new(),
            run_task: None,
            audio,
            stats: crate::stats::RunStats::default(),
            perf: Vec::new(),
            emu_clock: Instant::now(),
            accent: 0,
            #[cfg(target_family = "wasm")]
            web_tick_armed: false,
        }
    }
}
