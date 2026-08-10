use std::sync::Arc;

use core_model::Core;
use workspace_model::{
    ComponentManifest, DockLocation, PanelDescriptor, PanelGroupLayout, PanelKind, RefreshPolicy,
    WorkspaceDescriptor, WorkspaceIcon,
};

use crate::model::{
    AudioScopesView, CallStackView, CartridgeView, CpuView, DisasmLine, DmaView, Highlight,
    MemoryRegion, MmioView, PaletteView, PpuView, Sprites, StructuredView, TableView, Tilemap,
    TilemapOverlay, TilemapViewport, Tileset, TilesetLayer, VideoGenerations,
};
use crate::script::{
    EventDescriptor, EventHit, HookHit, SCRIPT_BUILTINS_PANEL, SCRIPT_CONSOLE_PANEL,
    SCRIPT_EDITOR_PANEL, SCRIPT_WORKSPACE,
};

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct CpuId(Arc<str>);

impl CpuId {
    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn arc(&self) -> Arc<str> {
        self.0.clone()
    }
}

impl From<&str> for CpuId {
    fn from(value: &str) -> Self {
        Self(value.into())
    }
}

impl From<Arc<str>> for CpuId {
    fn from(value: Arc<str>) -> Self {
        Self(value)
    }
}

#[derive(Clone)]
pub struct CpuDescriptor {
    pub id: CpuId,
    pub title: Arc<str>,
    pub space: Arc<str>,
    pc_bits: u8,
}

impl CpuDescriptor {
    pub fn new(
        id: impl Into<CpuId>,
        title: impl Into<Arc<str>>,
        space: impl Into<Arc<str>>,
        pc_bits: u8,
    ) -> Self {
        assert!((1..=32).contains(&pc_bits), "PC width must be 1..=32 bits");

        Self {
            id: id.into(),
            title: title.into(),
            space: space.into(),
            pc_bits,
        }
    }

    pub fn pc_bits(&self) -> u8 {
        self.pc_bits
    }

    pub fn pc_mask(&self) -> u32 {
        if self.pc_bits == 32 {
            u32::MAX
        } else {
            (1_u32 << self.pc_bits) - 1
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BreakpointKind {
    Execute,
    Read,
    Write,
}

impl BreakpointKind {
    pub const ALL: [BreakpointKind; 3] = [
        BreakpointKind::Execute,
        BreakpointKind::Read,
        BreakpointKind::Write,
    ];

    pub fn id(self) -> &'static str {
        match self {
            BreakpointKind::Execute => "execute",
            BreakpointKind::Read => "read",
            BreakpointKind::Write => "write",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        match id {
            "execute" => Some(BreakpointKind::Execute),
            "read" => Some(BreakpointKind::Read),
            "write" => Some(BreakpointKind::Write),
            _ => None,
        }
    }

    pub fn tag(self) -> &'static str {
        match self {
            BreakpointKind::Execute => "PC",
            BreakpointKind::Read => "Read",
            BreakpointKind::Write => "Write",
        }
    }
}

#[derive(Clone)]
pub struct BreakpointMark {
    pub start: u32,
    pub end: u32,
    pub kind: BreakpointKind,
    pub cpu: CpuId,
    pub region: Arc<str>,
    pub pc_mask: u32,
}

impl BreakpointMark {
    pub fn covers(&self, kind: BreakpointKind, cpu: &CpuId, addr: u32) -> bool {
        if self.kind != kind {
            return false;
        }

        match kind {
            BreakpointKind::Execute => {
                self.cpu == *cpu
                    && (self.start & self.pc_mask..=self.end & self.pc_mask)
                        .contains(&(addr & self.pc_mask))
            }
            BreakpointKind::Read | BreakpointKind::Write => (self.start..=self.end).contains(&addr),
        }
    }

    pub fn covers_hit(&self, hit: &HookHit) -> bool {
        self.region == hit.region && self.covers(hit.kind, &hit.cpu, hit.addr)
    }
}

pub fn mark_at(marks: &[BreakpointMark], addr: u32, mask: u32) -> Option<BreakpointKind> {
    let addr = addr & mask;
    marks
        .iter()
        .find(|m| (m.start & mask..=m.end & mask).contains(&addr))
        .map(|m| m.kind)
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RunOutcome {
    Done,
    Breakpoint,
}

#[derive(Clone)]
pub struct StepTarget {
    pub id: Arc<str>,
    pub label: Arc<str>,
}

impl StepTarget {
    pub fn new(id: impl Into<Arc<str>>, label: impl Into<Arc<str>>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
        }
    }
}

pub trait DebugCore: Core {
    fn cpus(&self) -> Vec<CpuDescriptor>;
    fn pc(&self, cpu: &CpuId) -> u32;
    fn set_pc(&mut self, cpu: &CpuId, addr: u32);
    fn cpu_view(&self, cpu: &CpuId) -> CpuView;
    fn disassemble(&self, cpu: &CpuId, base: u32, count: usize) -> Vec<DisasmLine>;

    fn disasm_fingerprint(&self, _cpu: &CpuId, _base: u32, _count: usize) -> Option<u64> {
        None
    }

    fn disassemble_one(&self, cpu: &CpuId, addr: u32) -> Option<DisasmLine> {
        self.disassemble(cpu, addr, 1).into_iter().next()
    }

    fn call_stack(&self, _cpu: &CpuId) -> CallStackView {
        CallStackView::default()
    }

    fn manifest(&self) -> ComponentManifest {
        self::standard_manifest(&self.cpus())
    }

    fn debug_step(&mut self);
    fn debug_run_frame(&mut self, breakpoints: &[BreakpointMark]) -> RunOutcome;

    fn step_targets(&self) -> Vec<StepTarget> {
        Vec::new()
    }

    fn run_until(&mut self, _target: &str) -> RunOutcome {
        RunOutcome::Done
    }

    fn cartridge(&self) -> &CartridgeView;

    fn set_cover_art(&mut self, _cover: Arc<str>) {}

    fn mmio_view(&self) -> MmioView;

    fn dma_view(&self) -> DmaView {
        DmaView::default()
    }

    fn palette_view(&self) -> PaletteView {
        PaletteView::default()
    }

    fn ppu_view(&self) -> PpuView {
        PpuView::default()
    }

    fn tileset_layers(&self) -> Vec<TilesetLayer> {
        Vec::new()
    }

    fn tileset(&self, _layer: usize, _color: usize) -> Option<Tileset> {
        None
    }

    fn tilemap(&self, _layer: usize) -> Option<Tilemap> {
        None
    }

    fn tilemap_overlays(&self, _layer: usize) -> Vec<TilemapOverlay> {
        Vec::new()
    }

    fn tilemap_viewport(&self, _layer: usize) -> Option<TilemapViewport> {
        None
    }

    fn sprites(&self) -> Option<Sprites> {
        None
    }

    fn video_generations(&self) -> Option<VideoGenerations> {
        None
    }

    fn structured_view(&self, _id: &str, _selection: Option<usize>) -> StructuredView {
        StructuredView::default()
    }

    fn table_view(&self, _id: &str) -> TableView {
        TableView::default()
    }

    fn scopes_view(&self, _id: &str) -> AudioScopesView {
        AudioScopesView::default()
    }

    fn take_hits(&mut self) -> Vec<HookHit> {
        Vec::new()
    }

    fn events(&self) -> Vec<EventDescriptor> {
        Vec::new()
    }

    fn watch_events(&mut self, _ids: &[Arc<str>]) {}

    fn take_events(&mut self) -> Vec<EventHit> {
        Vec::new()
    }

    fn caption(&self, label: &str) -> String;
    fn memory_regions(&self) -> Vec<MemoryRegion>;
    fn read_memory(&self, region: &str, addr: u32, len: usize) -> Vec<u8>;

    fn write_memory(&mut self, _region: &str, _addr: u32, _bytes: &[u8]) -> usize {
        0
    }

    fn write_register(&mut self, _cpu: &CpuId, _name: &str, _value: u32) -> bool {
        false
    }

    fn format_address(&self, region: &str, addr: u32) -> String;
    fn memory_highlights(&self, region: &str, addr: u32, len: usize) -> Vec<Highlight>;
    fn latest_access(&self) -> Option<(Arc<str>, u32)>;
    fn annotate(&self, _addr: u32) -> String {
        String::new()
    }
}

pub const CPU_WORKSPACE: &str = "cpu";
pub const VIDEO_WORKSPACE: &str = "video";
pub const SOUND_WORKSPACE: &str = "sound";

pub fn standard_manifest(cpus: &[CpuDescriptor]) -> ComponentManifest {
    use DockLocation::{Bottom, Center, Left, Right};
    use RefreshPolicy::{Continuous, OnPause};

    let mut panels = vec![
        PanelDescriptor::new("controls", "Controls", PanelKind::Controls, Left, OnPause),
        PanelDescriptor::new(
            "savestates",
            "Save States",
            PanelKind::Savestates,
            Left,
            OnPause,
        ),
        PanelDescriptor::new(
            "breakpoints",
            "Breakpoints",
            PanelKind::Breakpoints,
            Left,
            OnPause,
        )
        .tag(CPU_WORKSPACE),
        PanelDescriptor::new(
            "cartridge",
            "Cartridge",
            PanelKind::Cartridge,
            Left,
            OnPause,
        )
        .tag(CPU_WORKSPACE),
    ];

    for c in cpus {
        let disassembly_id = format!("{}.disasm", c.id.as_str());
        let call_stack_id = format!("{}.callstack", c.id.as_str());
        panels.push(
            PanelDescriptor::new(
                disassembly_id,
                format!("{} Disassembly", c.title),
                PanelKind::Disassembly { cpu: c.id.arc() },
                Center,
                OnPause,
            )
            .tag(CPU_WORKSPACE),
        );
        panels.push(
            PanelDescriptor::new(
                call_stack_id,
                format!("{} Call Stack", c.title),
                PanelKind::CallStack { cpu: c.id.arc() },
                Center,
                OnPause,
            )
            .tag(CPU_WORKSPACE),
        );
    }
    panels.push(PanelDescriptor::new(
        "memory",
        "Memory",
        PanelKind::Memory,
        Center,
        OnPause,
    ));

    for c in cpus {
        panels.push(
            PanelDescriptor::new(
                c.id.as_str(),
                c.title.as_ref(),
                PanelKind::Registers { cpu: c.id.arc() },
                Right,
                OnPause,
            )
            .tag(CPU_WORKSPACE),
        );
    }

    panels.push(PanelDescriptor::new(
        "mmio",
        "MMIO",
        PanelKind::Mmio,
        Right,
        OnPause,
    ));

    panels.push(
        PanelDescriptor::new("log", "Log", PanelKind::Log, Bottom, Continuous).tag(CPU_WORKSPACE),
    );

    panels.push(
        PanelDescriptor::new(
            SCRIPT_EDITOR_PANEL,
            "Script",
            PanelKind::ScriptEditor,
            Center,
            OnPause,
        )
        .tag(SCRIPT_WORKSPACE),
    );
    panels.push(
        PanelDescriptor::new(
            SCRIPT_BUILTINS_PANEL,
            "Builtins",
            PanelKind::ScriptBuiltins,
            Right,
            OnPause,
        )
        .tag(SCRIPT_WORKSPACE),
    );
    panels.push(
        PanelDescriptor::new(
            SCRIPT_CONSOLE_PANEL,
            "Console",
            PanelKind::ScriptConsole,
            Bottom,
            Continuous,
        )
        .tag(SCRIPT_WORKSPACE),
    );

    let mut manifest = ComponentManifest {
        system: Arc::from(""),
        workspaces: vec![
            WorkspaceDescriptor::new(CPU_WORKSPACE, "CPU", WorkspaceIcon::Cpu),
            WorkspaceDescriptor::new(SCRIPT_WORKSPACE, "Lua", WorkspaceIcon::Script),
        ],
        panels,
        compositions: Vec::new(),
    };
    for c in cpus {
        manifest.compose_in(
            CPU_WORKSPACE,
            format!("{}.disasm", c.id.as_str()),
            format!("{}.callstack", c.id.as_str()),
            PanelGroupLayout::CompanionRail,
        );
    }
    manifest
}
