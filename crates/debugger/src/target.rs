use std::sync::Arc;

use cirsium::Highlight;
use cirsium::components::StatusView;
use cirsium::widgets::{CartridgeView, MemoryRegion, MmioView};
use debugger_model::{CpuView, DisasmLine};
use frontend_model::{ComponentManifest, DockLocation, PanelDescriptor, PanelKind, RefreshPolicy};
use gpui::SharedString;

use crate::breakpoints::BreakpointMark;
use crate::processor::{CpuDescriptor, CpuId};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum RunOutcome {
    Done,
    Breakpoint,
    Fault,
}

pub trait DebugTarget {
    fn cpus(&self) -> Vec<CpuDescriptor>;
    fn pc(&self, cpu: &CpuId) -> u32;
    fn set_pc(&mut self, cpu: &CpuId, addr: u32);
    fn cpu_view(&self, cpu: &CpuId) -> CpuView;
    fn disassemble(&self, cpu: &CpuId, base: u32, count: usize) -> Vec<DisasmLine>;

    fn manifest(&self) -> ComponentManifest {
        self::standard_manifest(&self.cpus())
    }

    fn step(&mut self) -> bool;
    fn run_frame(&mut self, breakpoints: &[BreakpointMark]) -> RunOutcome;
    fn reset(&mut self);

    fn cartridge(&self) -> &CartridgeView;
    fn mmio_view(&self) -> MmioView;
    fn status(&self, label: &str) -> StatusView;
    fn caption(&self, label: &str) -> SharedString;
    fn memory_regions(&self) -> Vec<MemoryRegion>;
    fn read_memory(&self, region: &str, addr: u32, len: usize) -> Vec<u8>;
    fn format_address(&self, region: &str, addr: u32) -> SharedString;
    fn memory_highlights(&self, region: &str, addr: u32, len: usize) -> Vec<Highlight>;
    fn latest_access(&self) -> Option<(SharedString, u32)>;
    fn annotate(&self, _addr: u32) -> SharedString {
        SharedString::default()
    }
}

pub fn standard_manifest(cpus: &[CpuDescriptor]) -> ComponentManifest {
    use DockLocation::{Bottom, Center, Left, Right};
    use RefreshPolicy::{Continuous, OnPause};

    let cpu_id = |c: &CpuDescriptor| -> Arc<str> { Arc::from(c.id.as_str()) };

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
            PanelKind::Disassembly { cpu: cpu_id(c) },
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
            PanelKind::Registers { cpu: cpu_id(c) },
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
