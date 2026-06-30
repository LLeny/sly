use cirsium::Highlight;
use cirsium::components::StatusView;
use cirsium::widgets::{CartridgeView, MemoryRegion};
use debugger_model::{CpuView, DisasmLine};
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

    fn step(&mut self) -> bool;
    fn run_frame(&mut self, breakpoints: &[BreakpointMark]) -> RunOutcome;
    fn reset(&mut self);

    fn cartridge(&self) -> &CartridgeView;
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
