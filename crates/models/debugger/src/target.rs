use std::sync::Arc;

use core_model::Core;
use workspace_model::{
    ComponentManifest, DockLocation, PanelDescriptor, PanelGroupLayout, PanelKind, RefreshPolicy,
    WorkspaceDescriptor, WorkspaceIcon,
};

use crate::{AccessTracking, AccessWidth, DebugMemoryError};

use crate::model::{
    AudioScopesView, CallStackView, CartridgeView, CpuView, DisasmLine, Highlight, MemoryRegion,
    MmioView, StructuredView, TableView,
};

use crate::script::{
    EventDescriptor, EventHit, HookHit, SCRIPT_BUILTINS_PANEL, SCRIPT_CONSOLE_PANEL,
    SCRIPT_EDITOR_PANEL, SCRIPT_WORKSPACE,
};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
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

#[derive(Clone, Debug)]
pub struct DecodeMode {
    pub id: Arc<str>,
    pub label: Arc<str>,
}

impl DecodeMode {
    pub fn new(id: impl Into<Arc<str>>, label: impl Into<Arc<str>>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct DisassemblyRequest {
    pub base: u32,
    pub count: usize,
    /// None uses the current CPU mode.
    pub mode: Option<Arc<str>>,
}

impl DisassemblyRequest {
    pub fn new(base: u32, count: usize) -> Self {
        Self {
            base,
            count,
            mode: None,
        }
    }
}

#[derive(Clone)]
pub struct CpuDescriptor {
    pub id: CpuId,
    pub title: Arc<str>,
    pub space: Arc<str>,
    pub decode_modes: Vec<DecodeMode>,
}

impl CpuDescriptor {
    pub fn new(
        id: impl Into<CpuId>,
        title: impl Into<Arc<str>>,
        space: impl Into<Arc<str>>,
    ) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            space: space.into(),
            decode_modes: Vec::new(),
        }
    }

    pub fn decode_modes(mut self, modes: Vec<DecodeMode>) -> Self {
        self.decode_modes = modes;
        self
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
}

impl BreakpointMark {
    pub fn covers(
        &self,
        kind: BreakpointKind,
        cpu: &CpuId,
        region: &str,
        addr: u32,
        width: u32,
    ) -> bool {
        if self.kind != kind || self.region.as_ref() != region {
            return false;
        }

        if kind == BreakpointKind::Execute {
            self.cpu == *cpu && (self.start..=self.end).contains(&addr)
        } else {
            let last = u64::from(addr) + u64::from(width.saturating_sub(1));
            u64::from(self.start) <= last && u64::from(self.end) >= u64::from(addr)
        }
    }

    pub fn covers_hit(&self, hit: &HookHit) -> bool {
        self.covers(hit.kind, &hit.cpu, &hit.region, hit.addr, hit.width.bytes())
    }
}

pub fn mark_at(
    marks: &[BreakpointMark],
    cpu: &CpuId,
    region: &str,
    addr: u32,
    kind: Option<BreakpointKind>,
) -> Option<BreakpointKind> {
    marks
        .iter()
        .find(|m| kind.is_none_or(|k| k == m.kind) && m.covers(m.kind, cpu, region, addr, 1))
        .map(|m| m.kind)
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RunOutcome {
    FrameComplete,
    Breakpoint,
    Event,
    TargetReached,
    BudgetExhausted,
    Unsupported,
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

    fn disassemble(&self, cpu: &CpuId, request: &DisassemblyRequest) -> Vec<DisasmLine>;

    fn disasm_fingerprint(&self, _cpu: &CpuId, _request: &DisassemblyRequest) -> Option<u64> {
        None
    }

    fn disassemble_one(&self, cpu: &CpuId, request: &DisassemblyRequest) -> Option<DisasmLine> {
        let mut request = request.clone();
        request.count = 1;
        self.disassemble(cpu, &request).into_iter().next()
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
        RunOutcome::Unsupported
    }

    fn cartridge(&self) -> &CartridgeView;

    fn set_cover_art(&mut self, _cover: Arc<str>) {}

    fn mmio_view(&self) -> MmioView;

    fn video(&self) -> Option<&dyn crate::VideoDebug> {
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

    fn peek_memory(&self, region: &str, addr: u32, len: usize) -> Vec<u8>;

    fn read_bus(
        &mut self,
        _region: &str,
        _addr: u32,
        _width: AccessWidth,
    ) -> Result<u32, DebugMemoryError> {
        Err(DebugMemoryError::Unsupported)
    }

    fn write_bus(
        &mut self,
        _region: &str,
        _addr: u32,
        _width: AccessWidth,
        _value: u32,
    ) -> Result<(), DebugMemoryError> {
        Err(DebugMemoryError::Unsupported)
    }

    fn track_accesses(&mut self, _tracking: AccessTracking) {}

    fn patch_memory(&mut self, _region: &str, _addr: u32, _bytes: &[u8]) -> usize {
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

pub mod panel {
    pub const CONTROLS: &str = "controls";
    pub const SAVESTATES: &str = "savestates";
    pub const BREAKPOINTS: &str = "breakpoints";
    pub const CARTRIDGE: &str = "cartridge";
    pub const MEMORY: &str = "memory";
    pub const MMIO: &str = "mmio";
    pub const LOG: &str = "log";
    pub const FRAMEBUFFER: &str = "framebuffer";
    pub const PALETTE: &str = "palette";
    pub const PPU: &str = "ppu";
    pub const DMA: &str = "dma";
    pub const TILESET: &str = "tileset";
    pub const TILEMAP: &str = "tilemap";
    pub const SPRITES: &str = "sprites";
    pub const RASTER: &str = "raster";
    pub const AUDIO_SCOPE: &str = "audio_scope";
}

pub fn standard_manifest(cpus: &[CpuDescriptor]) -> ComponentManifest {
    use DockLocation::{Bottom, Center, Left, Right};
    use RefreshPolicy::{Continuous, OnPause};

    let mut panels = vec![
        PanelDescriptor::new(
            panel::CONTROLS,
            "Controls",
            PanelKind::Controls,
            Left,
            OnPause,
        ),
        PanelDescriptor::new(
            panel::SAVESTATES,
            "Save States",
            PanelKind::Savestates,
            Left,
            OnPause,
        ),
        PanelDescriptor::new(
            panel::BREAKPOINTS,
            "Breakpoints",
            PanelKind::Breakpoints,
            Left,
            OnPause,
        )
        .tag(CPU_WORKSPACE),
        PanelDescriptor::new(
            panel::CARTRIDGE,
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
        panel::MEMORY,
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
        panel::MMIO,
        "MMIO",
        PanelKind::Mmio,
        Right,
        OnPause,
    ));

    panels.push(
        PanelDescriptor::new(panel::LOG, "Log", PanelKind::Log, Bottom, Continuous)
            .tag(CPU_WORKSPACE),
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
