use std::sync::Arc;

use core_model::Core;
use frontend_model::{ComponentManifest, DockLocation, PanelDescriptor, PanelKind, RefreshPolicy};

use crate::model::{
    CallStackView, CartridgeView, CpuView, DisasmLine, DmaView, Highlight, MemoryRegion, MmioView,
    StatusView,
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
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
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

pub fn mark_at(marks: &[BreakpointMark], addr: u32, mask: u32) -> Option<BreakpointKind> {
    let addr = addr & mask;
    marks
        .iter()
        .find(|m| (m.start & mask..=m.end & mask).contains(&addr))
        .map(|m| m.kind)
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum RunOutcome {
    Done,
    Breakpoint,
    Fault,
}

pub trait DebugCore: Core {
    fn cpus(&self) -> Vec<CpuDescriptor>;
    fn pc(&self, cpu: &CpuId) -> u32;
    fn set_pc(&mut self, cpu: &CpuId, addr: u32);
    fn cpu_view(&self, cpu: &CpuId) -> CpuView;
    fn disassemble(&self, cpu: &CpuId, base: u32, count: usize) -> Vec<DisasmLine>;

    fn call_stack(&self, _cpu: &CpuId) -> CallStackView {
        CallStackView::default()
    }

    fn manifest(&self) -> ComponentManifest {
        self::standard_manifest(&self.cpus())
    }

    fn debug_step(&mut self) -> bool;
    fn debug_run_frame(&mut self, breakpoints: &[BreakpointMark]) -> RunOutcome;

    fn cartridge(&self) -> &CartridgeView;
    fn mmio_view(&self) -> MmioView;

    fn dma_view(&self) -> DmaView {
        DmaView::default()
    }

    fn status(&self, label: &str) -> StatusView;
    fn caption(&self, label: &str) -> String;
    fn memory_regions(&self) -> Vec<MemoryRegion>;
    fn read_memory(&self, region: &str, addr: u32, len: usize) -> Vec<u8>;
    fn format_address(&self, region: &str, addr: u32) -> String;
    fn memory_highlights(&self, region: &str, addr: u32, len: usize) -> Vec<Highlight>;
    fn latest_access(&self) -> Option<(Arc<str>, u32)>;
    fn annotate(&self, _addr: u32) -> String {
        String::new()
    }
}

pub fn standard_manifest(cpus: &[CpuDescriptor]) -> ComponentManifest {
    use DockLocation::{Bottom, Center, Left, Right};
    use RefreshPolicy::{Continuous, OnPause};

    let mut panels = vec![
        PanelDescriptor::new("controls", "Controls", PanelKind::Controls, Left, OnPause),
        PanelDescriptor::new(
            "breakpoints",
            "Breakpoints",
            PanelKind::Breakpoints,
            Left,
            OnPause,
        ),
        PanelDescriptor::new(
            "cartridge",
            "Cartridge",
            PanelKind::Cartridge,
            Left,
            OnPause,
        ),
    ];

    for c in cpus {
        panels.push(PanelDescriptor::new(
            format!("{}.disasm", c.id.as_str()),
            format!("{} Disassembly", c.title),
            PanelKind::Disassembly { cpu: c.id.arc() },
            Center,
            OnPause,
        ));
    }
    panels.push(PanelDescriptor::new(
        "memory",
        "Memory",
        PanelKind::Memory,
        Center,
        OnPause,
    ));

    for c in cpus {
        panels.push(PanelDescriptor::new(
            c.id.as_str(),
            c.title.as_ref(),
            PanelKind::Registers { cpu: c.id.arc() },
            Right,
            OnPause,
        ));
    }

    panels.push(PanelDescriptor::new(
        "mmio",
        "MMIO",
        PanelKind::Mmio,
        Right,
        OnPause,
    ));

    panels.push(PanelDescriptor::new(
        "log",
        "Log",
        PanelKind::Log,
        Bottom,
        Continuous,
    ));

    ComponentManifest {
        system: Arc::from(""),
        panels,
    }
}
